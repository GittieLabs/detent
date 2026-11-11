//! The whole pipeline: MockBus -> devices -> router (fake frontmost app) ->
//! a probe executor, on the engine's real threads with real time.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use detent_lib::actions::{ActionError, Done, Executor, Outcome};
use detent_lib::devices::{DeviceEvent, Lost, Options};
use detent_lib::engine::{recovery_delay, Engine, Record, Sink};
use detent_lib::store::{self, Action, Config, Trigger};
use detent_lib::transport::{MockBus, MockHid};
use detent_protocol::codec::{ButtonAction, DeviceMessage};
use detent_protocol::ids::{PID, VID};

const SERIAL: &str = "E072A1FC4EDC";
const WAIT: Duration = Duration::from_secs(2);
/// A frontmost app that makes the rig's frontmost lookup panic.
const PANIC_APP: &str = "!panic";

fn config() -> Config {
    store::parse(&format!(
        r#"{{ "version": 1,
          "devices": [{{ "name": "desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "{SERIAL}",
            "menu": [
              {{ "label": "Mail", "action": {{ "type": "open_app", "bundle_id": "com.apple.mail" }} }},
              {{ "label": "Louder", "action": {{ "type": "volume_up", "step": 0.125 }} }}
            ] }}],
          "mappings": [
            {{ "device": "{SERIAL}", "control": 1, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "open_app", "bundle_id": "com.apple.mail" }} }},
            {{ "device": "{SERIAL}", "control": 1, "trigger": "press", "scope": {{ "app": "com.apple.Safari" }},
               "action": {{ "type": "mute_toggle" }} }},
            {{ "device": "{SERIAL}", "control": 2, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "applescript", "source": "slow" }} }},
            {{ "device": "{SERIAL}", "control": 3, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "open_app", "bundle_id": "com.example.panic" }} }},
            {{ "device": "{SERIAL}", "control": 5, "trigger": "clockwise", "scope": "everywhere",
               "action": {{ "type": "volume_up", "step": 0.125 }} }}
          ] }}"#
    ))
    .expect("test config parses")
}

fn mail() -> Action {
    Action::OpenApp {
        bundle_id: "com.apple.mail".into(),
    }
}

fn press(control: u8) -> DeviceMessage {
    DeviceMessage::Button {
        control,
        action: ButtonAction::Down,
    }
}

/// Reports each action it's given, with the time it was called. Blocks on
/// AppleScript actions until released, to stand in for a slow script.
struct ProbeExecutor {
    calls: Mutex<Sender<(Action, Instant)>>,
    release: Mutex<Receiver<()>>,
}

impl Executor for ProbeExecutor {
    fn run(&self, action: &Action) -> Outcome {
        let _ = self
            .calls
            .lock()
            .unwrap()
            .send((action.clone(), Instant::now()));
        if matches!(action, Action::OpenApp { bundle_id } if bundle_id == "com.example.panic") {
            panic!("probe executor told to panic");
        }
        if matches!(action, Action::AppleScript { .. }) {
            let _ = self
                .release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10));
        }
        // Volume and mute answer like the real executor, so the status line
        // has something to show; everything else is just recorded.
        match action {
            Action::VolumeUp { step } => Ok(Done::Volume {
                from: 0.5,
                to: 0.5 + step,
            }),
            Action::MuteToggle => Ok(Done::Muted(true)),
            _ => Ok(Done::Recorded),
        }
    }
}

struct ChannelSink(Mutex<Sender<Record>>);

impl Sink for ChannelSink {
    fn record(&self, r: &Record) {
        let _ = self.0.lock().unwrap().send(r.clone());
    }
}

struct Rig {
    hid: MockHid,
    bus: MockBus,
    frontmost: Arc<Mutex<Option<String>>>,
    calls: Receiver<(Action, Instant)>,
    records: Receiver<Record>,
    release: Sender<()>,
    engine: Option<detent_lib::engine::EngineHandle>,
}

impl Rig {
    fn start() -> Self {
        Self::start_with(Options::default().status_for)
    }

    /// Like `start`, with the OLED status line cleared after `status_for`.
    fn start_with(status_for: Duration) -> Self {
        let bus = MockBus::default();
        let hid = bus.attach(VID, PID, Some(SERIAL));
        let (calls_tx, calls) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let (records_tx, records) = mpsc::channel();
        let frontmost = Arc::new(Mutex::new(None::<String>));
        let front = frontmost.clone();
        let engine = Engine::new(
            bus.clone(),
            &config(),
            Arc::new(ProbeExecutor {
                calls: Mutex::new(calls_tx),
                release: Mutex::new(release_rx),
            }),
            Arc::new(move || {
                let app = front.lock().unwrap().clone();
                // Stands in for a bug anywhere in the engine's turn.
                if app.as_deref() == Some(PANIC_APP) {
                    panic!("frontmost told to panic");
                }
                app
            }),
            Arc::new(ChannelSink(Mutex::new(records_tx))),
            Options {
                read_wait: Duration::from_millis(2),
                status_for,
                ..Options::default()
            },
        )
        .spawn();
        let rig = Self {
            hid,
            bus,
            frontmost,
            calls,
            records,
            release,
            engine: Some(engine),
        };
        rig.expect(|r| matches!(r, Record::Device(DeviceEvent::Connected { .. })));
        rig
    }

    /// Waits for a record matching `f`, skipping others.
    fn expect(&self, f: impl Fn(&Record) -> bool) -> Record {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.records.recv_timeout(left) {
                Ok(r) if f(&r) => return r,
                Ok(_) => {}
                Err(_) => panic!("no matching record within {WAIT:?}"),
            }
        }
    }

    fn call(&self) -> (Action, Instant) {
        self.calls.recv_timeout(WAIT).expect("an action ran")
    }

    fn no_call(&self) {
        assert!(self.calls.recv_timeout(Duration::from_millis(100)).is_err());
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.release.send(());
        if let Some(e) = self.engine.take() {
            e.stop();
        }
    }
}

#[test]
fn a_press_runs_its_mapping_and_is_logged_end_to_end() {
    let r = Rig::start();
    r.hid.push_device(0, &press(1));
    assert_eq!(r.call().0, mail());
    match r.expect(|x| matches!(x, Record::Resolved { .. })) {
        Record::Resolved {
            id,
            control,
            trigger,
            frontmost,
            action,
            count,
            paused,
            dry_run,
        } => {
            assert!(!paused && !dry_run);
            assert_eq!(id.serial, SERIAL);
            assert_eq!((control, trigger, count), (1, Trigger::Press, 1));
            assert_eq!(frontmost, None);
            assert_eq!(action, Some(mail()));
        }
        _ => unreachable!(),
    }
    match r.expect(|x| matches!(x, Record::Ran { .. })) {
        Record::Ran {
            action, outcome, ..
        } => {
            assert_eq!(action, mail());
            assert_eq!(outcome, Ok(Done::Recorded));
        }
        _ => unreachable!(),
    }
}

#[test]
fn dispatch_is_well_under_100_ms() {
    let r = Rig::start();
    let mut worst = Duration::ZERO;
    for i in 0..20u8 {
        let pushed = Instant::now();
        r.hid.push_device(i, &press(1));
        let (_, called) = r.call();
        worst = worst.max(called - pushed);
    }
    assert!(worst < Duration::from_millis(50), "worst {worst:?}");
    match r.expect(|x| matches!(x, Record::Ran { .. })) {
        Record::Ran { waited, .. } => assert!(waited < Duration::from_millis(50), "{waited:?}"),
        _ => unreachable!(),
    }
}

#[test]
fn the_frontmost_app_picks_the_app_scoped_mapping() {
    let r = Rig::start();
    *r.frontmost.lock().unwrap() = Some("com.apple.Safari".into());
    r.hid.push_device(0, &press(1));
    assert_eq!(r.call().0, Action::MuteToggle);
    *r.frontmost.lock().unwrap() = Some("com.apple.Terminal".into());
    r.hid.push_device(1, &press(1));
    assert_eq!(r.call().0, mail());
}

#[test]
fn an_encoder_turn_runs_once_per_detent() {
    let r = Rig::start();
    r.hid.push_device(
        0,
        &DeviceMessage::Encoder {
            control: 5,
            delta: 3,
        },
    );
    for _ in 0..3 {
        assert_eq!(r.call().0, Action::VolumeUp { step: 0.125 });
    }
    r.no_call();
}

#[test]
fn unmapped_input_and_releases_run_nothing_but_are_logged() {
    let r = Rig::start();
    r.hid.push_device(0, &press(9));
    let rec = r.expect(|x| matches!(x, Record::Resolved { .. }));
    assert!(matches!(rec, Record::Resolved { action: None, .. }));
    r.hid.push_device(
        1,
        &DeviceMessage::Button {
            control: 1,
            action: ButtonAction::Up,
        },
    );
    r.expect(|x| matches!(x, Record::Device(DeviceEvent::Input { .. })));
    r.no_call();
}

#[test]
fn a_slow_action_does_not_stall_input() {
    let r = Rig::start();
    r.hid.push_device(0, &press(2)); // the blocking script
    assert!(matches!(r.call().0, Action::AppleScript { .. }));
    // The engine thread still reads and resolves while the worker is busy.
    r.hid.push_device(1, &press(1));
    let rec = r.expect(|x| matches!(x, Record::Resolved { control: 1, .. }));
    assert!(matches!(
        rec,
        Record::Resolved {
            action: Some(_),
            ..
        }
    ));
    r.no_call();
    // Released, the queued action runs, in order.
    r.release.send(()).unwrap();
    assert_eq!(r.call().0, mail());
}

#[test]
fn unplug_and_replug_are_logged_and_presses_work_again() {
    let mut r = Rig::start();
    r.bus.detach(SERIAL);
    r.expect(|x| matches!(x, Record::Device(DeviceEvent::Disconnected { .. })));
    r.hid = r.bus.attach(VID, PID, Some(SERIAL));
    r.expect(|x| matches!(x, Record::Device(DeviceEvent::Connected { .. })));
    r.hid.push_device(0, &press(1));
    assert_eq!(r.call().0, mail());
}

#[test]
fn stop_returns_promptly() {
    let mut r = Rig::start();
    let started = Instant::now();
    r.engine.take().unwrap().stop();
    assert!(started.elapsed() < Duration::from_millis(500));
}

#[test]
fn records_read_well_in_the_log() {
    let r = Rig::start();
    r.hid.push_device(0, &press(1));
    let resolved = r
        .expect(|x| matches!(x, Record::Resolved { .. }))
        .to_string();
    assert!(
        resolved.contains("button 1 press") && resolved.contains("open_app com.apple.mail"),
        "{resolved}"
    );
    let ran = r.expect(|x| matches!(x, Record::Ran { .. })).to_string();
    assert!(ran.contains("recorded (dry run)"), "{ran}");
}

#[test]
fn the_log_file_appends_and_starts_over_past_1_mb() {
    use detent_lib::engine::FileLog;
    let dir = std::env::temp_dir().join(format!("detent-log-test-{}", std::process::id()));
    let path = dir.join("nested/detent.log");
    let _ = std::fs::remove_dir_all(&dir);

    FileLog::open(&path).unwrap().line("first");
    FileLog::open(&path).unwrap().line("second");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 2, "{text}");
    assert!(text.lines().last().unwrap().ends_with(" second"));

    std::fs::write(&path, vec![b'x'; 1024 * 1024 + 1]).unwrap();
    FileLog::open(&path).unwrap().line("fresh");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.ends_with(" fresh\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_sleep_shows_as_the_wall_clock_outrunning_the_monotonic_one() {
    use detent_lib::engine::slept;
    let s = Duration::from_secs;
    let ms = Duration::from_millis;
    // Awake: both clocks move together, give or take scheduling jitter.
    assert!(!slept(ms(20), ms(20)));
    assert!(!slept(ms(20), ms(900)));
    // A wall clock step backwards (NTP) is not a sleep.
    assert!(!slept(ms(20), Duration::ZERO));
    // Asleep for a minute: the monotonic clock barely moved.
    assert!(slept(ms(20), s(60)));
    assert!(slept(ms(20), s(3)));
}

// Live reload (M3).

#[test]
fn a_reloaded_mapping_applies_to_the_next_press_without_reconnecting() {
    let r = Rig::start();
    let mut config = config();
    config.mappings[0].action = Action::MuteToggle; // button 1 everywhere
    r.engine.as_ref().unwrap().reload(config);
    r.expect(|x| matches!(x, Record::Reloaded { .. }));
    r.hid.push_device(0, &press(1));
    assert_eq!(r.call().0, Action::MuteToggle);
    assert_eq!(r.bus.opens(), 1, "the unit was not reopened");
}

#[test]
fn reloading_without_the_unit_disconnects_it() {
    let r = Rig::start();
    let mut config = config();
    config.devices.clear();
    config.mappings.clear();
    r.engine.as_ref().unwrap().reload(config);
    r.expect(|x| matches!(x, Record::Device(DeviceEvent::Disconnected { .. })));
    r.hid.push_device(0, &press(1));
    r.no_call();
}

#[test]
fn a_candidate_press_is_reported_but_runs_nothing() {
    let r = Rig::start();
    let other = r.bus.attach(VID, PID, Some("AABBCCDDEEFF"));
    r.engine.as_ref().unwrap().identify(true);
    r.expect(|x| matches!(x, Record::Device(DeviceEvent::CandidateFound { .. })));
    other.push_device(0, &press(1));
    r.expect(|x| matches!(x, Record::Device(DeviceEvent::CandidatePressed(_))));
    r.no_call();
    r.engine.as_ref().unwrap().identify(false);
}

#[test]
fn a_paused_press_is_logged_but_runs_nothing_until_unpaused() {
    let r = Rig::start();
    let engine = r.engine.as_ref().unwrap();
    let mut paused = config();
    paused.paused = true;
    engine.reload(paused);
    r.expect(|x| matches!(x, Record::Reloaded { .. }));
    r.hid.push_device(0, &press(1));
    let rec = r.expect(|x| matches!(x, Record::Resolved { .. }));
    assert!(
        matches!(&rec, Record::Resolved { action: Some(a), paused: true, .. } if *a == mail()),
        "{rec:?}"
    );
    assert!(rec.to_string().ends_with("(paused, not run)"), "{rec}");
    r.no_call();

    engine.reload(config());
    r.expect(|x| matches!(x, Record::Reloaded { .. }));
    r.hid.push_device(1, &press(1));
    assert_eq!(r.call().0, mail());
}

#[test]
fn stopping_tells_the_unit_it_is_no_longer_paired() {
    let mut r = Rig::start();
    r.engine.take().unwrap().stop();
    let last = r.hid.writes().last().cloned().expect("something was sent");
    assert_eq!(
        detent_protocol::codec::decode_app(&last[1..]).unwrap().msg,
        detent_protocol::codec::AppMessage::HelloAck { paired: false }
    );
}

#[test]
fn dry_run_resolves_and_records_but_never_calls_the_executor() {
    let r = Rig::start();
    let engine = r.engine.as_ref().unwrap();
    engine.set_dry_run(true);
    let on = r.expect(|x| matches!(x, Record::DryRun { .. }));
    assert_eq!(on, Record::DryRun { on: true });
    assert_eq!(on.to_string(), "dry run on: actions are logged, not run");

    r.hid.push_device(0, &press(1));
    let rec = r.expect(|x| matches!(x, Record::Resolved { .. }));
    assert!(
        matches!(&rec, Record::Resolved { action: Some(a), dry_run: true, .. } if *a == mail()),
        "{rec:?}"
    );
    match r.expect(|x| matches!(x, Record::Ran { .. })) {
        Record::Ran {
            action, outcome, ..
        } => {
            assert_eq!(action, mail());
            assert_eq!(outcome, Ok(Done::Recorded));
        }
        _ => unreachable!(),
    }
    r.no_call();

    engine.set_dry_run(false);
    assert_eq!(
        r.expect(|x| matches!(x, Record::DryRun { .. })),
        Record::DryRun { on: false }
    );
    r.hid.push_device(1, &press(1));
    assert_eq!(r.call().0, mail());
}

#[test]
fn asking_for_the_dry_run_state_it_has_records_nothing() {
    let r = Rig::start();
    let engine = r.engine.as_ref().unwrap();
    engine.set_dry_run(false); // already off
    engine.set_dry_run(true);
    engine.set_dry_run(true); // already on
    engine.set_dry_run(false);
    let mut seen = Vec::new();
    while seen.len() < 2 {
        seen.push(r.expect(|x| matches!(x, Record::DryRun { .. })));
    }
    assert_eq!(
        seen,
        [Record::DryRun { on: true }, Record::DryRun { on: false }]
    );
    let more = r
        .records
        .recv_timeout(Duration::from_millis(100))
        .into_iter()
        .filter(|x| matches!(x, Record::DryRun { .. }))
        .count();
    assert_eq!(more, 0, "no repeats");
}

// Device menu items (M6): MENU_SELECT runs the picked item's action.

#[test]
fn picking_a_device_menu_item_runs_its_action() {
    let r = Rig::start();
    r.hid.push_device(0, &DeviceMessage::MenuSelect { item: 2 });
    let rec = r.expect(|x| matches!(x, Record::MenuPicked { .. }));
    assert_eq!(
        rec.to_string(),
        format!("{SERIAL} menu \"Louder\" -> volume_up 13%")
    );
    assert_eq!(r.call().0, Action::VolumeUp { step: 0.125 });
}

#[test]
fn an_unknown_menu_item_runs_nothing() {
    let r = Rig::start();
    r.hid.push_device(0, &DeviceMessage::MenuSelect { item: 9 });
    let rec = r.expect(|x| matches!(x, Record::MenuPicked { .. }));
    assert!(rec.to_string().ends_with("-> no such item"), "{rec}");
    r.no_call();
}

#[test]
fn a_menu_item_obeys_dry_run() {
    let r = Rig::start();
    let engine = r.engine.as_ref().unwrap();
    engine.set_dry_run(true);
    r.expect(|x| matches!(x, Record::DryRun { .. }));
    r.hid.push_device(0, &DeviceMessage::MenuSelect { item: 1 });
    r.expect(|x| matches!(x, Record::MenuPicked { .. }));
    r.no_call();
}

// The device's status line after an action (M6, D-087).

#[test]
fn an_action_result_becomes_a_short_status_line() {
    use detent_lib::actions::ActionError;
    use detent_lib::engine::status_of;
    use detent_lib::keys::Refusal;
    assert_eq!(
        status_of(&Ok(Done::Volume {
            from: 0.31,
            to: 0.375
        })),
        Some("Vol 38%".into())
    );
    assert_eq!(status_of(&Ok(Done::Muted(true))), Some("Muted".into()));
    assert_eq!(status_of(&Ok(Done::Muted(false))), Some("Unmuted".into()));
    assert_eq!(
        status_of(&Ok(Done::Opened("com.apple.mail".into()))),
        Some("Opened Mail".into())
    );
    assert_eq!(
        status_of(&Ok(Done::Opened("com.electron.plasticity".into()))),
        Some("Opened Plasticity".into())
    );
    assert_eq!(
        status_of(&Ok(Done::Sent("cmd+shift+e".into()))),
        Some("Sent cmd+shift+e".into())
    );
    assert_eq!(
        status_of(&Ok(Done::Script(String::new()))),
        Some("Script done".into())
    );
    assert_eq!(
        status_of(&Ok(Done::Script("42".into()))),
        Some("Script done".into())
    );
    assert_eq!(
        status_of(&Err(ActionError::Keys(Refusal::SecureInput))),
        Some("Failed".into())
    );
    assert_eq!(
        status_of(&Ok(Done::Recorded)),
        None,
        "a dry run shows nothing"
    );
    // Always at most 20 printable ASCII characters (the protocol's STATUS).
    let long = status_of(&Ok(Done::Sent("cmd+option+control+shift+f12".into()))).unwrap();
    assert_eq!(long.len(), 20);
    assert!(detent_protocol::codec::StatusText::new(&long).is_ok());
    let odd = status_of(&Ok(Done::Opened("com.example.Café".into()))).unwrap();
    assert!(
        detent_protocol::codec::StatusText::new(&odd).is_ok(),
        "{odd}"
    );
}

fn statuses(hid: &MockHid) -> Vec<String> {
    hid.writes()
        .iter()
        .filter_map(
            |w| match detent_protocol::codec::decode_app(&w[1..]).ok()?.msg {
                detent_protocol::codec::AppMessage::Status { text } => {
                    Some(text.as_str().to_owned())
                }
                _ => None,
            },
        )
        .collect()
}

/// Waits until the device has been sent `want` statuses, in order.
fn wait_for_statuses(hid: &MockHid, want: &[&str]) {
    let deadline = Instant::now() + WAIT;
    while statuses(hid) != want && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(statuses(hid), want);
}

#[test]
fn a_status_goes_to_the_device_after_an_action_and_clears() {
    let r = Rig::start_with(Duration::from_millis(150));
    r.hid.push_device(
        0,
        &DeviceMessage::Encoder {
            control: 5,
            delta: 1,
        },
    );
    r.call();
    wait_for_statuses(&r.hid, &["Vol 63%"]);
    // Cleared once `status_for` passes with no newer status.
    wait_for_statuses(&r.hid, &["Vol 63%", ""]);
}

#[test]
fn a_newer_status_restarts_the_clock() {
    let r = Rig::start_with(Duration::from_millis(300));
    r.hid.push_device(
        0,
        &DeviceMessage::Encoder {
            control: 5,
            delta: 1,
        },
    );
    r.call();
    wait_for_statuses(&r.hid, &["Vol 63%"]);
    std::thread::sleep(Duration::from_millis(150));
    r.hid.push_device(
        1,
        &DeviceMessage::Encoder {
            control: 5,
            delta: 1,
        },
    );
    r.call();
    wait_for_statuses(&r.hid, &["Vol 63%", "Vol 63%"]);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        statuses(&r.hid).len(),
        2,
        "not cleared under the newer one's 300 ms"
    );
    wait_for_statuses(&r.hid, &["Vol 63%", "Vol 63%", ""]);
}

#[test]
fn a_dry_run_sends_no_status() {
    let r = Rig::start_with(Duration::from_millis(100));
    r.engine.as_ref().unwrap().set_dry_run(true);
    r.expect(|x| matches!(x, Record::DryRun { .. }));
    r.hid.push_device(
        0,
        &DeviceMessage::Encoder {
            control: 5,
            delta: 1,
        },
    );
    r.expect(|x| matches!(x, Record::Ran { .. }));
    std::thread::sleep(Duration::from_millis(250));
    assert!(statuses(&r.hid).is_empty());
}

#[test]
fn the_dashboard_says_shown_or_hidden_on_the_status_line() {
    use detent_lib::engine::status_of;
    assert_eq!(
        status_of(&Ok(Done::Dashboard { shown: true })),
        Some("Dashboard on".into())
    );
    assert_eq!(
        status_of(&Ok(Done::Dashboard { shown: false })),
        Some("Dashboard off".into())
    );
}

#[test]
fn a_panicking_action_is_reported_and_the_next_one_runs() {
    let r = Rig::start();
    r.hid.push_device(0, &press(3));
    let rec = r.expect(|x| matches!(x, Record::Recovered { .. }));
    let Record::Recovered { thread, message } = rec else {
        unreachable!()
    };
    assert_eq!(thread, "actions");
    assert_eq!(message, "probe executor told to panic");
    let ran = r.expect(|x| matches!(x, Record::Ran { .. }));
    let Record::Ran { outcome, .. } = ran else {
        unreachable!()
    };
    assert!(
        matches!(outcome, Err(ActionError::Panicked(_))),
        "{outcome:?}"
    );
    let _ = r.call(); // the panicking call itself
    r.hid.push_device(0, &press(1));
    assert_eq!(r.call().0, mail());
}

#[test]
fn a_panic_in_the_engine_is_recovered_and_input_works_again() {
    let r = Rig::start();
    *r.frontmost.lock().unwrap() = Some(PANIC_APP.into());
    r.hid.push_device(0, &press(1));
    let rec = r.expect(|x| matches!(x, Record::Recovered { .. }));
    let Record::Recovered { thread, message } = rec else {
        unreachable!()
    };
    assert_eq!(thread, "engine");
    assert_eq!(message, "frontmost told to panic");
    // Every unit is closed and reopened, so none is left half read.
    r.expect(|x| {
        matches!(
            x,
            Record::Device(DeviceEvent::Disconnected {
                why: Lost::Recovered,
                ..
            })
        )
    });
    *r.frontmost.lock().unwrap() = None;
    r.expect(|x| matches!(x, Record::Device(DeviceEvent::Connected { .. })));
    r.hid.push_device(0, &press(1));
    assert_eq!(r.call().0, mail());
}

#[test]
fn recovery_waits_longer_after_each_panic_in_a_row_up_to_a_cap() {
    assert_eq!(recovery_delay(1), Duration::from_millis(250));
    assert_eq!(recovery_delay(2), Duration::from_millis(500));
    assert_eq!(recovery_delay(3), Duration::from_secs(1));
    assert_eq!(recovery_delay(8), Duration::from_secs(30));
    assert_eq!(recovery_delay(200), Duration::from_secs(30));
}

#[test]
fn a_recovery_reads_well_in_the_log() {
    let line = Record::Recovered {
        thread: "engine".into(),
        message: "boom".into(),
    }
    .to_string();
    assert_eq!(line, "recovered from a panic in the engine thread: boom");
}
