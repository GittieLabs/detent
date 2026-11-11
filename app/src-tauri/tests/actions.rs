//! Actions: argument checks, the recording executor, volume arithmetic, and
//! AppleScript through `osascript` with harmless scripts. Real effects (an app
//! opening, the volume moving, Automation prompts) are `acceptance.md` M2.

use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use detent_lib::actions::{
    applescript, next_volume, ActionError, Done, Executor, RecordingExecutor, ScriptError,
};
use detent_lib::store::Action;

fn invalid_actions() -> Vec<(Action, &'static str)> {
    vec![
        (
            Action::OpenApp {
                bundle_id: "Mail".into(),
            },
            "bundle_id",
        ),
        (Action::VolumeUp { step: 0.0 }, "step"),
        (Action::VolumeDown { step: 1.5 }, "step"),
        (Action::VolumeUp { step: f32::NAN }, "step"),
        (Action::VolumeSet { level: -0.1 }, "level"),
        (Action::VolumeSet { level: f32::NAN }, "level"),
        (
            Action::AppleScript {
                source: " \n".into(),
                timeout_ms: 1000,
            },
            "source",
        ),
        (
            Action::AppleScript {
                source: "beep".into(),
                timeout_ms: 99,
            },
            "timeout_ms",
        ),
        (
            Action::SendShortcut {
                keys: "cmd+banana".into(),
            },
            "keys",
        ),
        (Action::SendShortcut { keys: "".into() }, "keys"),
    ]
}

#[test]
fn argument_checks_name_the_bad_field() {
    for (action, field) in invalid_actions() {
        let err = action.check_args().expect_err(&format!("{action:?}"));
        assert_eq!(err.field, field, "{action:?}");
    }
    for ok in [
        Action::OpenApp {
            bundle_id: "com.apple.mail".into(),
        },
        Action::VolumeUp { step: 1.0 },
        Action::VolumeSet { level: 0.0 },
        Action::VolumeSet { level: 1.0 },
        Action::MuteToggle,
        Action::AppleScript {
            source: "beep".into(),
            timeout_ms: 100,
        },
    ] {
        assert_eq!(ok.check_args(), Ok(()), "{ok:?}");
    }
}

#[test]
fn recording_executor_records_in_order_and_runs_nothing() {
    let rec = RecordingExecutor::default();
    let a = Action::OpenApp {
        bundle_id: "com.apple.mail".into(),
    };
    let b = Action::VolumeUp { step: 0.25 };
    assert_eq!(rec.run(&a), Ok(Done::Recorded));
    assert_eq!(rec.run(&b), Ok(Done::Recorded));
    assert_eq!(rec.calls(), vec![a, b]);
}

#[test]
fn recording_executor_refuses_bad_arguments_without_recording() {
    let rec = RecordingExecutor::default();
    for (action, field) in invalid_actions() {
        match rec.run(&action) {
            Err(ActionError::Invalid(e)) => assert_eq!(e.field, field),
            other => panic!("{action:?} gave {other:?}"),
        }
    }
    assert!(rec.calls().is_empty());
}

#[test]
fn recording_executor_is_shared_across_threads() {
    let rec = Arc::new(RecordingExecutor::default());
    let exec: Arc<dyn Executor> = rec.clone();
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let exec = exec.clone();
            thread::spawn(move || exec.run(&Action::MuteToggle))
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), Ok(Done::Recorded));
    }
    assert_eq!(rec.calls().len(), 4);
}

#[test]
fn volume_steps_clamp_to_the_range() {
    let up = Action::VolumeUp { step: 0.25 };
    let down = Action::VolumeDown { step: 0.25 };
    assert_eq!(next_volume(0.5, &up), Some(0.75));
    assert_eq!(next_volume(0.9, &up), Some(1.0));
    assert_eq!(next_volume(0.5, &down), Some(0.25));
    assert_eq!(next_volume(0.1, &down), Some(0.0));
    assert_eq!(
        next_volume(0.3, &Action::VolumeSet { level: 0.8 }),
        Some(0.8)
    );
    assert_eq!(next_volume(0.3, &Action::MuteToggle), None);
}

#[test]
fn results_read_well_in_the_log() {
    assert_eq!(
        Done::Opened("com.apple.mail".into()).to_string(),
        "opened com.apple.mail"
    );
    assert_eq!(
        Done::Volume {
            from: 0.5,
            to: 0.5625
        }
        .to_string(),
        "volume 50% -> 56%"
    );
    assert_eq!(
        ScriptError::TimedOut { timeout_ms: 300 }.to_string(),
        "AppleScript timed out after 300 ms and was stopped"
    );
}

// AppleScript through osascript. These scripts touch no app, so they raise no
// Automation prompt and run the same on CI.

#[test]
fn applescript_returns_its_result() {
    let out = applescript::run("set x to 6\nreturn x * 7", Duration::from_secs(5));
    assert_eq!(out, Ok("42".to_owned()));
}

#[test]
fn applescript_source_with_quotes_and_unicode_arrives_intact() {
    let out = applescript::run(r#"return "say \"hi\" — ok""#, Duration::from_secs(5));
    assert_eq!(out, Ok(r#"say "hi" — ok"#.to_owned()));
}

#[test]
fn applescript_errors_carry_the_message() {
    match applescript::run(r#"error "boom" number 42"#, Duration::from_secs(5)) {
        Err(ScriptError::Failed { message, .. }) => {
            assert!(message.contains("boom"), "{message}")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn applescript_syntax_errors_are_failures() {
    assert!(matches!(
        applescript::run("this is not applescript (", Duration::from_secs(5)),
        Err(ScriptError::Failed { .. })
    ));
}

#[test]
fn large_output_does_not_deadlock() {
    // Far more than a pipe buffer holds, so the reader must drain as it runs.
    let out = applescript::run(
        r#"do shell script "head -c 300000 /dev/zero | tr '\\0' x""#,
        Duration::from_secs(10),
    )
    .expect("runs");
    assert_eq!(out.len(), 300_000);
}

#[test]
fn a_hung_script_times_out_and_its_children_are_stopped() {
    // A marker no other process will have, to look for leftovers.
    let marker = "31.4159265";
    let started = Instant::now();
    let out = applescript::run(
        &format!(r#"do shell script "sleep {marker}""#),
        Duration::from_millis(300),
    );
    assert_eq!(out, Err(ScriptError::TimedOut { timeout_ms: 300 }));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    thread::sleep(Duration::from_millis(200));
    let left = Command::new("/usr/bin/pgrep")
        .args(["-f", &format!("sleep {marker}")])
        .output()
        .expect("pgrep runs");
    assert!(
        left.stdout.is_empty(),
        "left running: {}",
        String::from_utf8_lossy(&left.stdout)
    );
}

#[cfg(target_os = "macos")]
#[test]
fn system_executor_checks_arguments_before_touching_anything() {
    use detent_lib::actions::SystemExecutor;
    // Each of these would open an app, move the volume, or run a script if the
    // check came second. None of them may.
    for (action, field) in invalid_actions() {
        match SystemExecutor.run(&action) {
            Err(ActionError::Invalid(e)) => assert_eq!(e.field, field),
            other => panic!("{action:?} gave {other:?}"),
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn system_executor_reports_a_missing_app() {
    use detent_lib::actions::SystemExecutor;
    let action = Action::OpenApp {
        bundle_id: "com.gittielabs.detent.no-such-app".into(),
    };
    assert!(matches!(
        SystemExecutor.run(&action),
        Err(ActionError::OpenApp(_))
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn system_executor_runs_applescript() {
    use detent_lib::actions::SystemExecutor;
    let action = Action::AppleScript {
        source: "return 1 + 1".into(),
        timeout_ms: 5000,
    };
    assert_eq!(SystemExecutor.run(&action), Ok(Done::Script("2".into())));
}

// Send shortcut (M5).

#[test]
fn a_shortcut_is_recorded_in_dry_run_and_never_sent() {
    let rec = RecordingExecutor::default();
    let a = Action::SendShortcut {
        keys: "cmd+shift+e".into(),
    };
    assert_eq!(rec.run(&a), Ok(Done::Recorded));
    assert_eq!(rec.calls(), vec![a]);
}

#[test]
fn shortcut_results_read_well() {
    assert_eq!(
        Done::Sent("cmd+shift+e".into()).to_string(),
        "sent cmd+shift+e"
    );
    assert_eq!(
        ActionError::Keys(detent_lib::keys::Refusal::SecureInput).to_string(),
        "Secure Input is on (a password field has focus), so macOS blocks sent keys"
    );
}

#[test]
fn shortcuts_are_refused_before_any_key_without_the_grant_or_with_secure_input() {
    use detent_lib::keys::{refusal, Refusal};
    assert_eq!(refusal(false, false), Some(Refusal::NotTrusted));
    assert_eq!(
        refusal(false, true),
        Some(Refusal::NotTrusted),
        "the grant first"
    );
    assert_eq!(refusal(true, true), Some(Refusal::SecureInput));
    assert_eq!(refusal(true, false), None);
    assert_eq!(
        Refusal::NotTrusted.to_string(),
        "Detent needs Accessibility to send shortcuts"
    );
}

#[test]
fn a_dashboard_window_failure_says_so() {
    assert_eq!(
        ActionError::Window("no answer from the main thread".into()).to_string(),
        "the dashboard window failed: no answer from the main thread"
    );
}
