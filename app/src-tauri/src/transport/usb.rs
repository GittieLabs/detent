//! USB HID through hidapi. Opens only vendor channel entries (usage page
//! 0xFF00, usage 1) and always non exclusive: hidapi's macOS default is
//! exclusive, which swallows the device's volume keys (D-036).

use std::ffi::CString;

use detent_protocol::{USAGE, USAGE_PAGE};
use hidapi::{HidApi, HidDevice};

use super::{Bus, DeviceTransport, HidTransport, RawHid, TransportError};

/// [`HidTransport`] over a real HID device.
pub type UsbHid = HidTransport<HidDevice>;

impl RawHid for HidDevice {
    fn read_timeout(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, String> {
        HidDevice::read_timeout(self, buf, timeout_ms).map_err(|e| e.to_string())
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, String> {
        HidDevice::write(self, data).map_err(|e| e.to_string())
    }
}

/// An attached vendor channel. Which ones to open (the allowlist, the ignore
/// list, unknown units) is the devices module's call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub path: CString,
    pub vid: u16,
    pub pid: u16,
    /// The unit's serial, if it reports one.
    pub serial: Option<String>,
    pub product: Option<String>,
}

/// The HID subsystem: lists attached vendor channels and opens them.
pub struct Hid {
    api: HidApi,
}

impl std::fmt::Debug for Hid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Hid")
    }
}

fn hid_err(e: hidapi::HidError) -> TransportError {
    TransportError::Disconnected(e.to_string())
}

impl Hid {
    pub fn new() -> Result<Self, TransportError> {
        let api = HidApi::new().map_err(hid_err)?;
        api.set_open_exclusive(false);
        Ok(Self { api })
    }

    /// Vendor channel entries attached now, of any VID and PID.
    pub fn channels(&mut self) -> Result<Vec<Channel>, TransportError> {
        self.api.refresh_devices().map_err(hid_err)?;
        Ok(self
            .api
            .device_list()
            .filter(|d| d.usage_page() == USAGE_PAGE && d.usage() == USAGE)
            .map(|d| Channel {
                path: d.path().to_owned(),
                vid: d.vendor_id(),
                pid: d.product_id(),
                serial: d.serial_number().map(str::to_owned),
                product: d.product_string().map(str::to_owned),
            })
            .collect())
    }

    /// Opens a channel non exclusive.
    pub fn open(&self, channel: &Channel) -> Result<UsbHid, TransportError> {
        let device = self.api.open_path(&channel.path).map_err(hid_err)?;
        Ok(HidTransport::from_raw(device))
    }
}

impl Bus for Hid {
    fn scan(&mut self) -> Result<Vec<Channel>, TransportError> {
        self.channels()
    }

    fn open_channel(
        &mut self,
        channel: &Channel,
    ) -> Result<Box<dyn DeviceTransport>, TransportError> {
        Ok(Box::new(self.open(channel)?))
    }
}
