//! The committed C header must match what the crate generates. If this fails,
//! run `scripts/gen-header.sh` and commit the header with the crate change.

use std::path::PathBuf;

fn committed_header() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../firmware/include/detent_protocol.h");
    std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{} unreadable ({err}); run scripts/gen-header.sh",
            path.display()
        )
    })
}

#[test]
fn committed_header_matches_the_crate() {
    assert_eq!(
        committed_header(),
        detent_protocol::header::c_header(),
        "firmware/include/detent_protocol.h is stale; run scripts/gen-header.sh"
    );
}

#[test]
fn header_carries_every_message_type() {
    let header = detent_protocol::header::c_header();
    for (id, name) in detent_protocol::msg::ALL {
        let line = format!("#define DETENT_MSG_{name} 0x{id:02X}");
        assert!(header.contains(&line), "missing: {line}");
    }
}

#[test]
fn header_carries_version_and_usb_ids() {
    let header = detent_protocol::header::c_header();
    assert!(header.contains("#define DETENT_PROTOCOL_VERSION 3\n"));
    // Each board's IDs and model name under its board flag (D-112, D-113).
    for m in detent_protocol::ids::MODELS {
        let block = format!(
            "defined(DETENT_BOARD_{})\n#define DETENT_USB_VID 0x{:04X}\n#define DETENT_USB_PID 0x{:04X}\n#define DETENT_MODEL_NAME \"{}\"\n",
            m.board, m.vid, m.pid, m.name
        );
        assert!(header.contains(&block), "missing: {block}");
    }
    assert!(header.contains("#define DETENT_REPORT_LEN 63\n"));
    assert!(header.contains("#define DETENT_VENDOR_REPORT_ID 6\n"));
    assert!(header.contains("#define DETENT_CONSUMER_REPORT_ID 4\n"));
}

#[test]
fn header_carries_codec_limits_and_enum_values() {
    let header = detent_protocol::header::c_header();
    for line in [
        "#define DETENT_MAX_CONTROLS 27\n",
        "#define DETENT_KIND_ENCODER_WITH_PUSH 3\n",
        "#define DETENT_MAX_LABEL_LEN 16\n",
        "#define DETENT_MAX_STATUS_LEN 20\n",
        "#define DETENT_MAX_INJECT_EVENTS 11\n",
        "#define DETENT_ENCODER_MODE_VOLUME 0\n",
        "#define DETENT_ENCODER_MODE_MENU 1\n",
        "#define DETENT_ENCODER_MODE_MAPPED 2\n",
        "#define DETENT_KIND_BUTTON 1\n",
        "#define DETENT_KIND_ENCODER 2\n",
        "#define DETENT_ACTION_DOWN 1\n",
        "#define DETENT_ACTION_UP 2\n",
        "#define DETENT_ACTION_LONG_PRESS 3\n",
    ] {
        assert!(header.contains(line), "missing: {line}");
    }
}
