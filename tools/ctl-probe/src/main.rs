//! ctl-probe: opens a Detent device and prints what it sends.
//!
//!   ctl-probe [--raw] [--count-hellos N]   decoded events; reopens after unplug
//!   ctl-probe list                         HID entries with Detent's VID
//!   ctl-probe raw [--secs N]               undecoded reports
//!
//! It only ever opens the vendor collection (usage page 0xFF00), never a
//! keyboard or the Consumer Control entry, and always non exclusive: an
//! exclusive open swallows the device's volume keys (D-036). It uses hidapi
//! directly; the app's DeviceTransport arrives in M2 (D-034).

mod args;
mod format;
mod hellos;

use std::ffi::CString;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant};

use detent_protocol::codec::{encode_app, AppMessage, DeviceMessage};
use detent_protocol::{ids, REPORT_LEN, USAGE, USAGE_PAGE, VENDOR_REPORT_ID};
use hidapi::{DeviceInfo, HidApi, HidDevice};

use args::Command;
use format::{classify, hex_trimmed, line, Event};
use hellos::{HelloCounter, Progress};

/// How often to look for the device while it's unplugged.
const RECONNECT_POLL: Duration = Duration::from_millis(250);
/// Read timeout, which is also how often the HELLO counter's clock ticks.
const READ_TIMEOUT_MS: i32 = 100;

fn is_vendor_channel(d: &DeviceInfo) -> bool {
    d.vendor_id() == ids::VID
        && d.product_id() == ids::PID
        && d.usage_page() == USAGE_PAGE
        && d.usage() == USAGE
}

/// The vendor channel's path and serial, if a device is attached.
fn find_channel(api: &mut HidApi) -> Option<(CString, String)> {
    api.refresh_devices().ok()?;
    api.device_list().find(|d| is_vendor_channel(d)).map(|d| {
        (
            d.path().to_owned(),
            d.serial_number().unwrap_or("?").to_owned(),
        )
    })
}

fn list(api: &HidApi) {
    let mut found = 0;
    for d in api.device_list().filter(|d| d.vendor_id() == ids::VID) {
        found += 1;
        println!(
            "vid=0x{:04X} pid=0x{:04X} usage_page=0x{:04X} usage=0x{:02X} interface={} serial={} product={}",
            d.vendor_id(),
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.interface_number(),
            d.serial_number().unwrap_or("?"),
            d.product_string().unwrap_or("?"),
        );
    }
    if found == 0 {
        println!("no HID devices with vid=0x{:04X}", ids::VID);
    }
}

fn open_once(api: &mut HidApi) -> Result<HidDevice, String> {
    let (path, _) = find_channel(api).ok_or("no Detent vendor channel found")?;
    api.open_path(&path)
        .map_err(|e| format!("open failed: {e}"))
}

fn raw(api: &mut HidApi, secs: u64) -> Result<ExitCode, String> {
    let dev = open_once(api)?;
    let start = Instant::now();
    let mut buf = [0u8; 1 + REPORT_LEN];
    while start.elapsed() < Duration::from_secs(secs) {
        let n = dev
            .read_timeout(&mut buf, READ_TIMEOUT_MS)
            .map_err(|e| format!("read failed: {e}"))?;
        if n > 0 {
            println!(
                "{:>8.3}s  len={n:<2} {}",
                start.elapsed().as_secs_f64(),
                hex_trimmed(&buf[..n])
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Sends REQUEST_HELLO: report ID, then the 63 byte report.
fn request_hello(dev: &HidDevice, seq: u8) -> Result<(), String> {
    let report = encode_app(seq, &AppMessage::RequestHello).map_err(|e| e.to_string())?;
    let mut out = [0u8; 1 + REPORT_LEN];
    out[0] = VENDOR_REPORT_ID;
    out[1..].copy_from_slice(&report);
    dev.write(&out).map(|_| ()).map_err(|e| e.to_string())
}

/// Prints progress; returns the exit code once counting is finished.
fn on_progress(p: Progress) -> Option<ExitCode> {
    match p {
        Progress::Waiting => None,
        Progress::Counted { count, target } => {
            println!("HELLO {count}/{target}. Unplug and replug the device.");
            None
        }
        Progress::Done => {
            println!("Done: every connection produced a HELLO.");
            Some(ExitCode::SUCCESS)
        }
        Progress::Missed { connection, why } => {
            eprintln!("ctl-probe: connection {connection}: {why}");
            Some(ExitCode::FAILURE)
        }
    }
}

fn watch(api: &mut HidApi, raw: bool, count_hellos: Option<u32>) -> Result<ExitCode, String> {
    let start = Instant::now();
    let stamp = || format!("{:>8.3}s", start.elapsed().as_secs_f64());
    let mut counter = count_hellos.map(HelloCounter::new);
    let mut seq_out: u8 = 0;
    let mut buf = [0u8; 1 + REPORT_LEN];

    if let Some(n) = count_hellos {
        println!("Counting HELLOs over {n} connections. Plug the device in.");
    }
    loop {
        // Wait for the device and open it.
        let mut announced = false;
        let (dev, serial) = loop {
            if let Some((path, serial)) = find_channel(api) {
                match api.open_path(&path) {
                    Ok(dev) => break (dev, serial),
                    Err(e) if !announced => {
                        eprintln!("{}  open failed, retrying: {e}", stamp());
                        announced = true;
                    }
                    Err(_) => {}
                }
            } else if !announced {
                println!("{}  waiting for device", stamp());
                announced = true;
            }
            sleep(RECONNECT_POLL);
        };
        println!("{}  connected, serial {serial}", stamp());
        if let Some(c) = counter.as_mut() {
            c.on_connect(Instant::now());
        }
        match request_hello(&dev, seq_out) {
            Ok(()) => seq_out = seq_out.wrapping_add(1),
            Err(e) => eprintln!("{}  REQUEST_HELLO failed: {e}", stamp()),
        }

        // Read until the device goes away.
        loop {
            match dev.read_timeout(&mut buf, READ_TIMEOUT_MS) {
                Ok(n) => {
                    let read = &buf[..n];
                    if let Some(l) = line(start.elapsed(), read, raw) {
                        println!("{l}");
                    }
                    if let (Some(c), Event::Message(f)) = (counter.as_mut(), classify(read)) {
                        if matches!(f.msg, DeviceMessage::Hello(_)) {
                            if let Some(code) = on_progress(c.on_hello()) {
                                return Ok(code);
                            }
                        }
                    }
                }
                Err(_) => {
                    println!("{}  disconnected", stamp());
                    if let Some(code) = counter
                        .as_mut()
                        .and_then(|c| on_progress(c.on_disconnect()))
                    {
                        return Ok(code);
                    }
                    break;
                }
            }
            if let Some(code) = counter
                .as_mut()
                .and_then(|c| on_progress(c.tick(Instant::now())))
            {
                return Ok(code);
            }
        }
    }
}

fn run(cmd: Command) -> Result<ExitCode, String> {
    let mut api = HidApi::new().map_err(|e| format!("hidapi init failed: {e}"))?;
    api.set_open_exclusive(false);
    match cmd {
        Command::List => {
            list(&api);
            Ok(ExitCode::SUCCESS)
        }
        Command::Raw { secs } => raw(&mut api, secs),
        Command::Watch { raw, count_hellos } => watch(&mut api, raw, count_hellos),
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cmd = match args::parse(&argv) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("ctl-probe: {err}\n{}", args::USAGE);
            return ExitCode::from(2);
        }
    };
    match run(cmd) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("ctl-probe: {err}");
            ExitCode::FAILURE
        }
    }
}
