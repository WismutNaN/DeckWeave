// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]

use super::{
    model::{AudioCommandError, AudioFailure, AudioReading},
    service::AudioBackend,
};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError},
};
use windows::{
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, EDataFlow, ERole,
            Endpoints::{
                IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                IAudioEndpointVolumeCallback_Impl,
            },
            IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
            MMDeviceEnumerator, eMultimedia, eRender,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
            CoUninitialize,
        },
    },
    core::{GUID, PCWSTR, implement},
};

mod recovery;
use recovery::Recovery;
use std::time::Instant;

const CALLBACK_CAPACITY: usize = 32;
const EVENT_CONTEXT: GUID = GUID::from_u128(0x6477b610_3d25_4c79_9551_71e4a813a79c);

mod callbacks;
use callbacks::*;

struct Apartment;

impl Apartment {
    fn initialize() -> Result<Self, AudioFailure> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(map_windows_error)?;
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct EndpointBinding {
    endpoint_ref: String,
    volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

pub(super) struct NativeAudioBackend {
    binding: Option<EndpointBinding>,
    enumerator: Option<IMMDeviceEnumerator>,
    device_callback: Option<IMMNotificationClient>,
    queue: Option<CallbackQueue>,
    events: Receiver<AdapterEvent>,
    overflow: Arc<AtomicBool>,
    _apartment: Option<Apartment>,
    initialization_failure: Option<AudioFailure>,
    recovery: Recovery,
}

impl NativeAudioBackend {
    pub fn new() -> Self {
        match Self::initialize() {
            Ok(backend) => backend,
            Err(failure) => {
                let (_sender, events) = mpsc::sync_channel(1);
                Self {
                    binding: None,
                    enumerator: None,
                    device_callback: None,
                    queue: None,
                    events,
                    overflow: Arc::new(AtomicBool::new(false)),
                    _apartment: None,
                    initialization_failure: Some(failure),
                    recovery: {
                        let mut r = Recovery::new();
                        r.failed(Instant::now());
                        r
                    },
                }
            }
        }
    }

    fn initialize() -> Result<Self, AudioFailure> {
        let apartment = Apartment::initialize()?;
        let overflow = Arc::new(AtomicBool::new(false));
        let (sender, events) = mpsc::sync_channel(CALLBACK_CAPACITY);
        let queue = CallbackQueue {
            sender,
            overflow: Arc::clone(&overflow),
        };
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(map_windows_error)?;
        let device_callback: IMMNotificationClient = DeviceCallback {
            queue: queue.clone(),
        }
        .into();
        unsafe { enumerator.RegisterEndpointNotificationCallback(&device_callback) }
            .map_err(map_windows_error)?;
        let backend = Self {
            binding: None,
            enumerator: Some(enumerator),
            device_callback: Some(device_callback),
            queue: Some(queue.clone()),
            events,
            overflow,
            _apartment: Some(apartment),
            initialization_failure: None,
            recovery: Recovery::new(),
        };
        Ok(backend)
    }

    fn bind_default(&mut self, queue: CallbackQueue) -> Result<(), AudioFailure> {
        self.clear_binding();
        let enumerator = self
            .enumerator
            .as_ref()
            .ok_or(AudioFailure::OsApiUnavailable)?;
        let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }
            .map_err(map_windows_error)?;
        let raw_id = unsafe { device.GetId() }.map_err(map_windows_error)?;
        let id_result = unsafe { raw_id.to_string() }.map_err(|_| AudioFailure::OsApiUnavailable);
        unsafe { CoTaskMemFree(Some(raw_id.0.cast())) };
        let endpoint_ref = format!("endpoint-{:x}", Sha256::digest(id_result?.as_bytes()));
        let volume: IAudioEndpointVolume =
            unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(map_windows_error)?;
        let callback: IAudioEndpointVolumeCallback = VolumeCallback { queue }.into();
        unsafe { volume.RegisterControlChangeNotify(&callback) }.map_err(map_windows_error)?;
        self.binding = Some(EndpointBinding {
            endpoint_ref,
            volume,
            callback,
        });
        Ok(())
    }

    fn clear_binding(&mut self) {
        if let Some(binding) = self.binding.take() {
            let _ = unsafe {
                binding
                    .volume
                    .UnregisterControlChangeNotify(&binding.callback)
            };
        }
    }

    fn process_events(&mut self) -> Maintenance {
        let maintenance = drain_events(&self.events, &self.overflow);
        if maintenance.rebind {
            self.clear_binding();
        }
        maintenance
    }

    fn ensure_binding(&mut self) -> Result<&EndpointBinding, AudioFailure> {
        self.process_events();
        if self.binding.is_none() {
            if !self.recovery.due(Instant::now()) {
                return Err(self
                    .initialization_failure
                    .unwrap_or(AudioFailure::TransientRebind));
            }
            if self.enumerator.is_none() {
                match Self::initialize() {
                    Ok(mut replacement) => {
                        std::mem::swap(&mut replacement.recovery, &mut self.recovery);
                        *self = replacement;
                    }
                    Err(failure) => {
                        self.record_failure(failure);
                        return Err(failure);
                    }
                }
            }
            {
                let queue = self.queue.clone().ok_or(AudioFailure::OsApiUnavailable)?;
                if let Err(failure) = self.bind_default(queue) {
                    self.record_failure(failure);
                    return Err(failure);
                }
                self.recovery.recovered();
                self.initialization_failure = None;
            }
        }
        self.binding.as_ref().ok_or(
            self.initialization_failure
                .unwrap_or(AudioFailure::NoDefaultEndpoint),
        )
    }

    fn record_failure(&mut self, failure: AudioFailure) -> AudioFailure {
        self.clear_binding();
        // A stale COM enumerator after an audio-service failure must be recreated too.
        if failure == AudioFailure::OsApiUnavailable {
            if let (Some(enumerator), Some(callback)) = (&self.enumerator, &self.device_callback) {
                let _ = unsafe { enumerator.UnregisterEndpointNotificationCallback(callback) };
            }
            self.device_callback = None;
            self.enumerator = None;
            self.queue = None;
            self._apartment = None;
        }
        self.initialization_failure = Some(failure);
        self.recovery.failed(Instant::now());
        failure
    }

    fn command_volume(
        &mut self,
        endpoint: &str,
    ) -> Result<IAudioEndpointVolume, AudioCommandError> {
        let binding = self
            .ensure_binding()
            .map_err(AudioCommandError::before_effect)?;
        if binding.endpoint_ref != endpoint {
            return Err(AudioCommandError::before_effect(
                AudioFailure::TransientRebind,
            ));
        }
        Ok(binding.volume.clone())
    }
}

impl AudioBackend for NativeAudioBackend {
    fn read(&mut self) -> Result<AudioReading, AudioFailure> {
        let (endpoint_ref, volume) = {
            let binding = self.ensure_binding()?;
            (binding.endpoint_ref.clone(), binding.volume.clone())
        };
        let scalar = unsafe { volume.GetMasterVolumeLevelScalar() }.map_err(map_windows_error);
        let muted = unsafe { volume.GetMute() }.map_err(map_windows_error);
        match (scalar, muted) {
            (Ok(scalar), Ok(muted)) => Ok(AudioReading {
                endpoint_ref,
                scalar: f64::from(scalar),
                muted: muted.as_bool(),
            }),
            (Err(failure), _) | (_, Err(failure)) => Err(self.record_failure(failure)),
        }
    }

    fn set_volume(&mut self, endpoint: &str, scalar: f64) -> Result<(), AudioCommandError> {
        let volume = self.command_volume(endpoint)?;
        unsafe { volume.SetMasterVolumeLevelScalar(scalar as f32, &EVENT_CONTEXT) }
            .map_err(map_windows_error)
            .map_err(|failure| AudioCommandError::after_effect(self.record_failure(failure)))
    }

    fn set_mute(&mut self, endpoint: &str, muted: bool) -> Result<(), AudioCommandError> {
        let volume = self.command_volume(endpoint)?;
        unsafe { volume.SetMute(muted, &EVENT_CONTEXT) }
            .map_err(map_windows_error)
            .map_err(|failure| AudioCommandError::after_effect(self.record_failure(failure)))
    }

    fn maintenance_due(&mut self) -> bool {
        let maintenance = self.process_events();
        if self.binding.is_none() {
            self.recovery.due(Instant::now())
        } else {
            maintenance.changed
        }
    }
}

impl Drop for NativeAudioBackend {
    fn drop(&mut self) {
        self.clear_binding();
        if let (Some(enumerator), Some(callback)) = (&self.enumerator, &self.device_callback) {
            let _ = unsafe { enumerator.UnregisterEndpointNotificationCallback(callback) };
        }
    }
}

fn map_windows_error(error: windows::core::Error) -> AudioFailure {
    classify_hresult(error.code().0)
}

fn classify_hresult(code: i32) -> AudioFailure {
    match code as u32 {
        0x8007_0490 => AudioFailure::NoDefaultEndpoint,
        0x8007_0005 => AudioFailure::PermissionDenied,
        0x8889_0004 => AudioFailure::TransientRebind,
        _ => AudioFailure::OsApiUnavailable,
    }
}

#[cfg(test)]
mod tests;
