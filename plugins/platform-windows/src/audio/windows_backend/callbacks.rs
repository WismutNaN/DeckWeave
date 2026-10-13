// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
#[derive(Debug, Clone, Copy)]
pub(super) enum AdapterEvent {
    VolumeChanged,
    DefaultEndpointChanged,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Maintenance {
    pub(super) changed: bool,
    pub(super) rebind: bool,
}

#[derive(Clone)]
pub(super) struct CallbackQueue {
    pub(super) sender: SyncSender<AdapterEvent>,
    pub(super) overflow: Arc<AtomicBool>,
}

impl CallbackQueue {
    pub(super) fn push(&self, event: AdapterEvent) {
        if self.sender.try_send(event).is_err() {
            self.overflow.store(true, Ordering::Release);
        }
    }
}

#[implement(IAudioEndpointVolumeCallback)]
pub(super) struct VolumeCallback {
    pub(super) queue: CallbackQueue,
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
pub(super) struct DeviceCallback {
    pub(super) queue: CallbackQueue,
}

#[allow(non_snake_case)]
impl IMMNotificationClient_Impl for DeviceCallback_Impl {
    fn OnDeviceStateChanged(
        &self,
        _device_id: &PCWSTR,
        _state: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        self.queue.push(AdapterEvent::DefaultEndpointChanged);
        Ok(())
    }

    fn OnDeviceAdded(&self, _device_id: &PCWSTR) -> windows::core::Result<()> {
        self.queue.push(AdapterEvent::DefaultEndpointChanged);
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

pub(super) fn drain_events(events: &Receiver<AdapterEvent>, overflow: &AtomicBool) -> Maintenance {
    // A full queue may have dropped the only default-endpoint notification.
    let lost_event = overflow.swap(false, Ordering::AcqRel);
    let mut maintenance = Maintenance {
        changed: lost_event,
        rebind: lost_event,
    };
    for _ in 0..CALLBACK_CAPACITY {
        match events.try_recv() {
            Ok(AdapterEvent::VolumeChanged) => maintenance.changed = true,
            Ok(AdapterEvent::DefaultEndpointChanged) => {
                maintenance.changed = true;
                maintenance.rebind = true;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return maintenance,
        }
    }
    maintenance
}
