//! The panic hook writes each panic to Detent's log with where it happened
//! and a backtrace. Its own test binary: a panic hook is process wide.

use std::sync::Arc;

use detent_lib::engine::{install_panic_hook, FileLog};

#[test]
fn a_panic_on_any_thread_lands_in_the_log() {
    let dir = std::env::temp_dir().join(format!("detent-panic-hook-{}", std::process::id()));
    let path = dir.join("detent.log");
    let _ = std::fs::remove_dir_all(&dir);
    let log = Arc::new(FileLog::open(&path).expect("log opens"));
    install_panic_hook(log);

    let joined = std::thread::Builder::new()
        .name("detent-test-panicker".into())
        .spawn(|| panic!("the hook should see this"))
        .unwrap()
        .join();
    assert!(joined.is_err());

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("panic in thread detent-test-panicker: the hook should see this"),
        "{text}"
    );
    assert!(text.contains("tests/panic_hook.rs"), "{text}");
    assert!(text.contains("backtrace:"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
