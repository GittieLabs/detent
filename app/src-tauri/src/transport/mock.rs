//! A scripted raw HID link for tests. The test keeps a [`MockHid`] handle to
//! push reads, unplug, and inspect writes while the transport is in use,
//! possibly on another thread.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use detent_protocol::codec::{encode_device, DeviceMessage};
use detent_protocol::VENDOR_REPORT_ID;

use super::{HidTransport, RawHid};

/// [`HidTransport`] over a scripted link: the real framing, fake hardware.
pub type MockTransport = HidTransport<MockHid>;

impl MockTransport {
    /// A transport and the handle that drives it.
    pub fn new() -> (MockTransport, MockHid) {
        let hid = MockHid::default();
        (HidTransport::from_raw(hid.clone()), hid)
    }
}

#[derive(Debug, Default)]
struct State {
    reads: VecDeque<Vec<u8>>,
    writes: Vec<Vec<u8>>,
    unplugged: bool,
    write_limit: Option<usize>,
}

/// The fake device side. Clones share one link.
#[derive(Debug, Clone, Default)]
pub struct MockHid {
    shared: Arc<(Mutex<State>, Condvar)>,
}

impl MockHid {
    fn state(&self) -> MutexGuard<'_, State> {
        self.shared.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn change(&self, f: impl FnOnce(&mut State)) {
        f(&mut self.state());
        self.shared.1.notify_all();
    }

    /// Queues one raw HID read, report ID included, exactly as given.
    pub fn push_raw(&self, bytes: Vec<u8>) {
        self.change(|s| s.reads.push_back(bytes));
    }

    /// Queues a device message as the board sends it: ID 6 plus 63 bytes.
    pub fn push_device(&self, seq: u8, msg: &DeviceMessage) {
        let report = encode_device(seq, msg).expect("test message encodes");
        let mut raw = vec![VENDOR_REPORT_ID];
        raw.extend_from_slice(&report);
        self.push_raw(raw);
    }

    /// The device goes away: every read and write fails from now on, and a
    /// read that's waiting wakes up with the error.
    pub fn unplug(&self) {
        self.change(|s| s.unplugged = true);
    }

    /// Makes the OS accept at most `n` bytes per write.
    pub fn limit_writes_to(&self, n: usize) {
        self.change(|s| s.write_limit = Some(n));
    }

    /// Every write so far, report ID included.
    pub fn writes(&self) -> Vec<Vec<u8>> {
        self.state().writes.clone()
    }
}

const UNPLUGGED: &str = "mock device unplugged";

impl RawHid for MockHid {
    fn read_timeout(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, String> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(0) as u64);
        let (lock, wake) = &*self.shared;
        let mut s = lock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if s.unplugged {
                return Err(UNPLUGGED.into());
            }
            if let Some(read) = s.reads.pop_front() {
                let n = read.len().min(buf.len());
                buf[..n].copy_from_slice(&read[..n]);
                return Ok(n);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(0);
            }
            s = wake
                .wait_timeout(s, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, String> {
        let mut s = self.state();
        if s.unplugged {
            return Err(UNPLUGGED.into());
        }
        let n = s.write_limit.map_or(data.len(), |l| l.min(data.len()));
        s.writes.push(data[..n].to_vec());
        Ok(n)
    }
}
