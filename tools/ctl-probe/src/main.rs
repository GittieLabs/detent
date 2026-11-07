//! ctl-probe: opens a Detent device and prints what it sends.
//!
//! M1 spike commands (raw, no codec yet):
//!   ctl-probe list                       every HID entry with Detent's VID
//!   ctl-probe dump [--secs N] [--exclusive]
//!                                        print raw vendor reports
//!   ctl-probe send <E|U|W> [--exclusive] [--report-id N]
//!                                        send a spike command, print any reply
//!
//! The probe only ever opens the vendor collection (usage page 0xFF00), never a
//! keyboard or Consumer Control entry.
//!
//! It opens non exclusive by default: an exclusive open swallows the device's
//! Consumer Control reports and stops the volume keys (D-036). `--exclusive`
//! exists only to demonstrate that.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use detent_protocol::{ids, REPORT_LEN, USAGE, USAGE_PAGE, VENDOR_REPORT_ID};
use hidapi::{DeviceInfo, HidApi, HidDevice};

#[derive(Debug, PartialEq, Eq)]
enum Command {
    List,
    Dump {
        secs: u64,
        exclusive: bool,
    },
    Send {
        cmd: u8,
        exclusive: bool,
        report_id: u8,
    },
}

const USAGE_TEXT: &str = "usage:
  ctl-probe list
  ctl-probe dump [--secs N] [--exclusive]
  ctl-probe send <E|U|W> [--exclusive] [--report-id N]";

fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut it = args.iter().map(String::as_str);
    let sub = it.next().ok_or("missing command")?;
    let mut secs = 10;
    let mut exclusive = false;
    let mut report_id = VENDOR_REPORT_ID;
    let mut cmd = None;
    while let Some(arg) = it.next() {
        match arg {
            "--exclusive" => exclusive = true,
            "--secs" => {
                let v = it.next().ok_or("--secs needs a value")?;
                secs = v.parse().map_err(|_| format!("bad --secs value: {v}"))?;
            }
            "--report-id" => {
                let v = it.next().ok_or("--report-id needs a value")?;
                report_id = v
                    .parse()
                    .map_err(|_| format!("bad --report-id value: {v}"))?;
            }
            c if sub == "send" && cmd.is_none() && matches!(c, "E" | "U" | "W") => {
                cmd = Some(c.as_bytes()[0]);
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    match sub {
        "list" => Ok(Command::List),
        "dump" => Ok(Command::Dump { secs, exclusive }),
        "send" => Ok(Command::Send {
            cmd: cmd.ok_or("send needs E, U, or W")?,
            exclusive,
            report_id,
        }),
        other => Err(format!("unknown command: {other}")),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Hex of `bytes` with trailing zero bytes collapsed, so 64 byte reports stay
/// readable.
fn hex_trimmed(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let zeros = bytes.len() - end;
    match (end, zeros) {
        (0, 0) => String::new(),
        (_, 0) => hex(&bytes[..end]),
        (0, z) => format!("00 x{z}"),
        (_, z) => format!("{} ... 00 x{z}", hex(&bytes[..end])),
    }
}

fn is_vendor_channel(d: &DeviceInfo) -> bool {
    d.vendor_id() == ids::VID
        && d.product_id() == ids::PID
        && d.usage_page() == USAGE_PAGE
        && d.usage() == USAGE
}

fn list(api: &HidApi) {
    let mut found = 0;
    for d in api.device_list().filter(|d| d.vendor_id() == ids::VID) {
        found += 1;
        println!(
            "vid=0x{:04X} pid=0x{:04X} usage_page=0x{:04X} usage=0x{:02X} interface={} serial={} product={} path={}",
            d.vendor_id(),
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.interface_number(),
            d.serial_number().unwrap_or("?"),
            d.product_string().unwrap_or("?"),
            d.path().to_string_lossy(),
        );
    }
    if found == 0 {
        println!("no HID devices with vid=0x{:04X}", ids::VID);
    }
}

fn open_vendor(api: &HidApi, exclusive: bool) -> Result<HidDevice, String> {
    api.set_open_exclusive(exclusive);
    let info = api
        .device_list()
        .find(|d| is_vendor_channel(d))
        .ok_or("no Detent vendor channel found (vid/pid/usage page 0xFF00)")?;
    let dev = info
        .open_device(api)
        .map_err(|e| format!("open failed: {e}"))?;
    println!(
        "opened {} ({})",
        info.path().to_string_lossy(),
        if exclusive { "exclusive" } else { "shared" }
    );
    Ok(dev)
}

fn dump(dev: &HidDevice, secs: u64) -> Result<(), String> {
    let start = Instant::now();
    let mut buf = [0u8; 65];
    while start.elapsed() < Duration::from_secs(secs) {
        let n = dev
            .read_timeout(&mut buf, 250)
            .map_err(|e| format!("read failed: {e}"))?;
        if n > 0 {
            println!(
                "{:>7.3}s len={n:<2} {}",
                start.elapsed().as_secs_f64(),
                hex_trimmed(&buf[..n])
            );
        }
    }
    Ok(())
}

fn send(dev: &HidDevice, cmd: u8, report_id: u8) -> Result<(), String> {
    // hidapi write: first byte is the report ID, then the report.
    let mut out = vec![0u8; 1 + REPORT_LEN];
    out[0] = report_id;
    out[1] = cmd;
    out[2..6].copy_from_slice(b"ping");
    let n = dev.write(&out).map_err(|e| format!("write failed: {e}"))?;
    println!("wrote {n} bytes: {}", hex_trimmed(&out));
    // Print what comes back for a moment (echo replies, heartbeats).
    dump(dev, 2)
}

fn run(cmd: Command) -> Result<(), String> {
    let api = HidApi::new().map_err(|e| format!("hidapi init failed: {e}"))?;
    match cmd {
        Command::List => {
            list(&api);
            Ok(())
        }
        Command::Dump { secs, exclusive } => dump(&open_vendor(&api, exclusive)?, secs),
        Command::Send {
            cmd,
            exclusive,
            report_id,
        } => send(&open_vendor(&api, exclusive)?, cmd, report_id),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = match parse_args(&args) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("ctl-probe: {err}\n{USAGE_TEXT}");
            return ExitCode::from(2);
        }
    };
    match run(cmd) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("ctl-probe: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn parses_each_command() {
        assert_eq!(parse_args(&args("list")), Ok(Command::List));
        assert_eq!(
            parse_args(&args("dump --secs 3 --exclusive")),
            Ok(Command::Dump {
                secs: 3,
                exclusive: true
            })
        );
        assert_eq!(
            parse_args(&args("send U")),
            Ok(Command::Send {
                cmd: b'U',
                exclusive: false,
                report_id: VENDOR_REPORT_ID
            })
        );
        assert_eq!(
            parse_args(&args("send E --report-id 0")),
            Ok(Command::Send {
                cmd: b'E',
                exclusive: false,
                report_id: 0
            })
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&args("send")).is_err());
        assert!(parse_args(&args("send X")).is_err());
        assert!(parse_args(&args("dump --secs")).is_err());
        assert!(parse_args(&args("dump --secs abc")).is_err());
        assert!(parse_args(&args("frobnicate")).is_err());
    }

    #[test]
    fn hex_collapses_trailing_zeros() {
        assert_eq!(
            hex_trimmed(&[0x06, 0x44, 0x01, 0, 0, 0]),
            "06 44 01 ... 00 x3"
        );
        assert_eq!(hex_trimmed(&[0, 0]), "00 x2");
        assert_eq!(hex_trimmed(&[0xab]), "ab");
        assert_eq!(hex_trimmed(&[]), "");
    }
}
