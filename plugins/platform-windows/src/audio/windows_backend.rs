// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]

use super::{
    model::{AudioFailure, AudioReading},
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

const CALLBACK_CAPACITY: usize = 32;
const EVENT_CONTEXT: GUID = GUID::from_u128(0x6477b610_3d25_4c79_9551_71e4a813a79c);

#[derive(Debug, Clone, Copy)]
enum AdapterEvent {
    VolumeChanged,
    DefaultEndpointChanged,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Maintenance {
    changed: bool,
    rebind: bool,
}

#[derive(Clone)]
struct CallbackQueue {
    sender: SyncSender<AdapterEvent>,
    overflow: Arc<AtomicBool>,
}

impl CallbackQueue {
    fn push(&self, event: AdapterEvent) {
        if self.sender.try_send(event).is_err() {
            self.overflow.store(true, Ordering::Release);
        }
    }
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    queue: CallbackQueue,
}

#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(
        &self,
        _notification: *mut AUDIO_VOLUME_NOTIFICATION_DATA,
    ) -> windows::core::Result<()> {
        self.queue.push(AdapterEvent::VolumeChanged);
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceCallback {
    queue: CallbackQueue,
}

#[allow(non_snake_case)]
impl IMMNotificationClient_Impl for DeviceCallback_Impl {
    fn OnDeviceStateChanged(
        &self,
        _device_id: &PCWSTR,
        _state: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDeviceAdded(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDeviceRemoved(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
        self.queue.push(AdapterEvent::DefaultEndpointChanged);
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _device_id: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flow == eRender && role == eMultimedia {
            self.queue.push(AdapterEvent::DefaultEndpointChanged);
        }
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        _device_id: &PCWSTR,
        _key: &PROPERTYKEY,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

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
        let mut backend = Self {
            binding: None,
            enumerator: Some(enumerator),
            device_callback: Some(device_callback),
            queue: Some(queue.clone()),
            events,
            overflow,
            _apartment: Some(apartment),
            initialization_failure: None,
        };
        let _ = backend.bind_default(queue);
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

    fn ensure_binding(&mut self) -> Result<&EndpointBinding, AudioFailure> {
        if self.binding.is_none() {
            let queue = self.queue.clone().ok_or_else(|| {
                self.initialization_failure
                    .unwrap_or(AudioFailure::OsApiUnavailable)
            })?;
            self.bind_default(queue)?;
        }
        self.binding.as_ref().ok_or(AudioFailure::NoDefaultEndpoint)
    }

    fn invalidate_on_rebind(&mut self, failure: AudioFailure) -> AudioFailure {
        if failure == AudioFailure::TransientRebind {
            self.clear_binding();
        }
        failure
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
            (Err(failure), _) | (_, Err(failure)) => Err(self.invalidate_on_rebind(failure)),
        }
    }

    fn set_volume(&mut self, scalar: f64) -> Result<(), AudioFailure> {
        let volume = self.ensure_binding()?.volume.clone();
        unsafe { volume.SetMasterVolumeLevelScalar(scalar as f32, &EVENT_CONTEXT) }
            .map_err(map_windows_error)
            .map_err(|failure| self.invalidate_on_rebind(failure))
    }

    fn set_mute(&mut self, muted: bool) -> Result<(), AudioFailure> {
        let volume = self.ensure_binding()?.volume.clone();
        unsafe { volume.SetMute(muted, &EVENT_CONTEXT) }
            .map_err(map_windows_error)
            .map_err(|failure| self.invalidate_on_rebind(failure))
    }

    fn maintenance_due(&mut self) -> bool {
        let maintenance = drain_events(&self.events, &self.overflow);
        if maintenance.rebind {
            self.clear_binding();
        }
        maintenance.changed
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

fn drain_events(events: &Receiver<AdapterEvent>, overflow: &AtomicBool) -> Maintenance {
    let mut maintenance = Maintenance {
        changed: overflow.swap(false, Ordering::AcqRel),
        rebind: false,
    };
    loop {
        match events.try_recv() {
            Ok(AdapterEvent::VolumeChanged) => maintenance.changed = true,
            Ok(AdapterEvent::DefaultEndpointChanged) => {
                maintenance.changed = true;
                maintenance.rebind = true;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return maintenance,
        }
    }
}

#[cfg(test)]
mod tests;
