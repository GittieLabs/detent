//! Transport: moves vendor reports to and from one device. Knows report IDs
//! and nothing about message contents; the devices module decodes.
//!
//! [`HidTransport`] does the framing over any [`RawHid`] link: every write is
//! report ID 6 plus the 63 byte report, every read has its ID checked and
//! stripped, and reads with another ID (Consumer Control's 4, anything else)
//! are dropped (D-036). [`UsbHid`] runs it over hidapi; [`MockTransport`] runs
//! the same code over a scripted link, so tests exercise the real framing
//! (D-061). Bluetooth later is another [`DeviceTransport`].

mod mock;
mod usb;

use std::fmt;
use std::time::{Duration, Instant};

use detent_protocol::{REPORT_LEN, VENDOR_REPORT_ID};

pub use mock::{MockBus, MockHid, MockTransport};
pub use usb::{Channel, Hid, UsbHid};

/// Why the link stopped working. Any error means the device is gone or
/// unusable: the caller closes it and waits for it to reappear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// A read or write failed; on macOS this is what an unplug looks like.
    Disconnected(String),
    /// The OS accepted fewer bytes than a whole report.
    ShortWrite { wrote: usize, expected: usize },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected(why) => write!(f, "device disconnected: {why}"),
            Self::ShortWrite { wrote, expected } => {
                write!(f, "wrote {wrote} of {expected} bytes")
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// One open device. Owned by one thread at a time.
pub trait DeviceTransport: Send {
    /// The next vendor report (the bytes after the report ID, normally 63),
    /// waiting up to `timeout`. `Ok(None)` when none arrived in time. Reports
    /// of the wrong length pass through for the codec to reject and count.
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError>;

    /// Sends one encoded app to device report.
    fn write(&mut self, report: &[u8; REPORT_LEN]) -> Result<(), TransportError>;
}

/// The list of attached vendor channels and a way to open them: hidapi
/// ([`Hid`]) for real, [`MockBus`] in tests.
pub trait Bus: Send {
    /// Vendor channel entries attached now.
    fn scan(&mut self) -> Result<Vec<Channel>, TransportError>;
    /// Opens one, non exclusive.
    fn open_channel(
        &mut self,
        channel: &Channel,
    ) -> Result<Box<dyn DeviceTransport>, TransportError>;
}

/// A raw HID link: reads and writes whole HID reports, report ID first, the
/// way hidapi does for a device with numbered reports.
pub trait RawHid: Send {
    /// Waits up to `timeout_ms` for one report; `Ok(0)` on timeout.
    fn read_timeout(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, String>;
    /// Writes one report, returning the bytes the OS accepted.
    fn write(&mut self, data: &[u8]) -> Result<usize, String>;
}

/// Report framing over a raw link. Once the link fails it stays failed.
#[derive(Debug)]
pub struct HidTransport<R> {
    raw: R,
    failed: Option<String>,
}

impl<R: RawHid> HidTransport<R> {
    pub fn from_raw(raw: R) -> Self {
        Self { raw, failed: None }
    }

    fn fail(&mut self, why: String) -> TransportError {
        self.failed = Some(why.clone());
        TransportError::Disconnected(why)
    }
}

impl<R: RawHid> DeviceTransport for HidTransport<R> {
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        if let Some(why) = &self.failed {
            return Err(TransportError::Disconnected(why.clone()));
        }
        let deadline = Instant::now() + timeout;
        // Room for a longer report than ours, so one shows up as the wrong
        // length instead of being cut to fit.
        let mut buf = [0u8; 2 * (1 + REPORT_LEN)];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let ms = i32::try_from(left.as_millis()).unwrap_or(i32::MAX);
            let n = match self.raw.read_timeout(&mut buf, ms) {
                Ok(n) => n,
                Err(why) => return Err(self.fail(why)),
            };
            match buf[..n].split_first() {
                // Timed out.
                None => return Ok(None),
                Some((&VENDOR_REPORT_ID, report)) => return Ok(Some(report.to_vec())),
                // Another collection's report: drop it and keep waiting.
                Some(_) if left.is_zero() => return Ok(None),
                Some(_) => {}
            }
        }
    }

    fn write(&mut self, report: &[u8; REPORT_LEN]) -> Result<(), TransportError> {
        if let Some(why) = &self.failed {
            return Err(TransportError::Disconnected(why.clone()));
        }
        let mut out = [0u8; 1 + REPORT_LEN];
        out[0] = VENDOR_REPORT_ID;
        out[1..].copy_from_slice(report);
        match self.raw.write(&out) {
            Ok(n) if n == out.len() => Ok(()),
            Ok(n) => Err(TransportError::ShortWrite {
                wrote: n,
                expected: out.len(),
            }),
            Err(why) => Err(self.fail(why)),
        }
    }
}
