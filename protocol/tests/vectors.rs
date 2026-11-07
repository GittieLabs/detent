//! The committed golden vectors must match what the codec generates, and every
//! case must be what it claims (packs and parses decode back, rejects reject).

use std::path::PathBuf;

use detent_protocol::codec::{decode_app, decode_device, encode_app, encode_device};
use detent_protocol::vectors::{c_vectors, pack_cases, parse_cases, reject_cases};

#[test]
fn committed_vectors_match_the_codec() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../firmware/test/vectors.h");
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{} unreadable ({err}); run scripts/gen-header.sh",
            path.display()
        )
    });
    assert_eq!(
        committed,
        c_vectors(),
        "firmware/test/vectors.h is stale; run scripts/gen-header.sh"
    );
}

#[test]
fn every_case_is_what_it_claims() {
    for (name, m, seq) in pack_cases() {
        let r = encode_device(seq, &m).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(decode_device(&r).map(|f| f.msg), Ok(m), "{name}");
    }
    for (name, m, seq) in parse_cases() {
        let r = encode_app(seq, &m).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(decode_app(&r).map(|f| f.msg), Ok(m), "{name}");
    }
    let rejects = reject_cases();
    assert!(rejects.len() >= 20);
    for (name, r) in rejects {
        assert!(decode_app(&r).is_err(), "{name} decoded");
    }
}
