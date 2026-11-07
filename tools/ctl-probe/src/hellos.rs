//! `--count-hellos N`: every connection must produce a HELLO, N times over.
//! M1 exit criterion: 20 unplug and replug cycles, a HELLO every time.

use std::time::{Duration, Instant};

/// How long a connection may go without a HELLO before it counts as a miss.
pub const HELLO_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, PartialEq, Eq)]
pub enum Progress {
    /// Keep going.
    Waiting,
    /// This connection's HELLO arrived; `count` of `target` done.
    Counted { count: u32, target: u32 },
    /// All `target` connections produced a HELLO.
    Done,
    /// A connection ended or timed out without a HELLO.
    Missed { connection: u32, why: &'static str },
}

pub struct HelloCounter {
    target: u32,
    count: u32,
    connections: u32,
    /// Set while connected and still waiting for this connection's HELLO.
    waiting_since: Option<Instant>,
    connected: bool,
}

impl HelloCounter {
    pub fn new(target: u32) -> Self {
        Self {
            target,
            count: 0,
            connections: 0,
            waiting_since: None,
            connected: false,
        }
    }

    pub fn on_connect(&mut self, now: Instant) {
        self.connections += 1;
        self.connected = true;
        self.waiting_since = Some(now);
    }

    /// Only the first HELLO of a connection counts.
    pub fn on_hello(&mut self) -> Progress {
        if self.waiting_since.take().is_none() {
            return Progress::Waiting;
        }
        self.count += 1;
        if self.count >= self.target {
            Progress::Done
        } else {
            Progress::Counted {
                count: self.count,
                target: self.target,
            }
        }
    }

    pub fn on_disconnect(&mut self) -> Progress {
        let missed = self.connected && self.waiting_since.is_some();
        self.connected = false;
        self.waiting_since = None;
        if missed {
            Progress::Missed {
                connection: self.connections,
                why: "disconnected before a HELLO",
            }
        } else {
            Progress::Waiting
        }
    }

    pub fn tick(&mut self, now: Instant) -> Progress {
        match self.waiting_since {
            Some(since) if now.duration_since(since) > HELLO_TIMEOUT => {
                self.waiting_since = None;
                Progress::Missed {
                    connection: self.connections,
                    why: "no HELLO within 3 s of connecting",
                }
            }
            _ => Progress::Waiting,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_one_hello_per_connection_until_done() {
        let t = Instant::now();
        let mut c = HelloCounter::new(2);
        c.on_connect(t);
        assert_eq!(
            c.on_hello(),
            Progress::Counted {
                count: 1,
                target: 2
            }
        );
        // A second HELLO on the same connection (REQUEST_HELLO reply) doesn't count.
        assert_eq!(c.on_hello(), Progress::Waiting);
        assert_eq!(c.on_disconnect(), Progress::Waiting);
        c.on_connect(t);
        assert_eq!(c.on_hello(), Progress::Done);
    }

    #[test]
    fn disconnect_without_hello_is_a_miss() {
        let mut c = HelloCounter::new(5);
        c.on_connect(Instant::now());
        assert_eq!(
            c.on_disconnect(),
            Progress::Missed {
                connection: 1,
                why: "disconnected before a HELLO"
            }
        );
    }

    #[test]
    fn timeout_without_hello_is_a_miss() {
        let t = Instant::now();
        let mut c = HelloCounter::new(5);
        c.on_connect(t);
        assert_eq!(c.tick(t + Duration::from_secs(2)), Progress::Waiting);
        assert_eq!(
            c.tick(t + Duration::from_secs(4)),
            Progress::Missed {
                connection: 1,
                why: "no HELLO within 3 s of connecting"
            }
        );
    }

    #[test]
    fn hello_in_time_stops_the_clock() {
        let t = Instant::now();
        let mut c = HelloCounter::new(5);
        c.on_connect(t);
        assert_eq!(
            c.on_hello(),
            Progress::Counted {
                count: 1,
                target: 5
            }
        );
        assert_eq!(c.tick(t + Duration::from_secs(10)), Progress::Waiting);
    }

    #[test]
    fn hello_while_disconnected_or_before_connect_is_ignored() {
        let mut c = HelloCounter::new(1);
        assert_eq!(c.on_hello(), Progress::Waiting);
        assert_eq!(c.on_disconnect(), Progress::Waiting);
    }
}
