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
    assert!(header.contains("#define DETENT_PROTOCOL_VERSION 1\n"));
    assert!(header.contains("#define DETENT_USB_VID 0x303A"));
    assert!(header.contains("#define DETENT_USB_PID 0x8FDE"));
    assert!(header.contains("#define DETENT_REPORT_LEN 63\n"));
    assert!(header.contains("#define DETENT_VENDOR_REPORT_ID 6\n"));
    assert!(header.contains("#define DETENT_CONSUMER_REPORT_ID 4\n"));
}
