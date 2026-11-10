//! Manual check for the macOS bindings (`docs/acceptance.md` M2 step 1).
//!
//!   just macos-check open com.apple.mail   open an app by bundle ID
//!   just macos-check frontmost             print the frontmost app for 10 s
//!   just macos-check volume                step the volume up, then restore it
//!   just macos-check mute                  toggle mute, then restore it

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    use detent_lib::macos;
    use std::thread::sleep;
    use std::time::Duration;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let result: Result<(), String> = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["open", id] => macos::open_app(id)
            .map(|()| println!("asked macOS to open {id}"))
            .map_err(|e| e.to_string()),
        ["frontmost"] => {
            println!("switch apps; printing the frontmost app every second for 10 s");
            for _ in 0..10 {
                println!(
                    "  {}",
                    macos::frontmost_bundle_id().unwrap_or_else(|| "(none)".into())
                );
                sleep(Duration::from_secs(1));
            }
            Ok(())
        }
        ["volume"] => (|| {
            let v0 = macos::volume()?;
            println!("volume {:.0}%; stepping up for 2 s", v0 * 100.0);
            macos::set_volume(v0 + 0.125)?;
            println!("volume {:.0}%", macos::volume()? * 100.0);
            sleep(Duration::from_secs(2));
            macos::set_volume(v0)?;
            println!("restored to {:.0}%", macos::volume()? * 100.0);
            Ok::<(), macos::AudioError>(())
        })()
        .map_err(|e| e.to_string()),
        ["mute"] => (|| {
            let m0 = macos::muted()?;
            println!("muted: {m0}; toggling for 2 s");
            macos::set_muted(!m0)?;
            println!("muted: {}", macos::muted()?);
            sleep(Duration::from_secs(2));
            macos::set_muted(m0)?;
            println!("restored, muted: {}", macos::muted()?);
            Ok::<(), macos::AudioError>(())
        })()
        .map_err(|e| e.to_string()),
        _ => Err("usage: macos_check open <bundle-id> | frontmost | volume | mute".into()),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("macos_check: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macos_check runs on macOS only");
}
