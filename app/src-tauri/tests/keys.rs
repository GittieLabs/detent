//! Shortcut names (D-082): parsed into combos on US ANSI key codes, written
//! back in one canonical form.

use detent_lib::keys::{self, Combo, KeyError, Mods};

fn combo(mods: Mods, key: &str) -> Combo {
    Combo {
        mods,
        key: key.into(),
        code: keys::code_of(key).expect("a known key"),
    }
}

const NONE: Mods = Mods {
    cmd: false,
    shift: false,
    option: false,
    control: false,
};

#[test]
fn a_combo_parses_with_modifiers_in_any_order_and_case() {
    let want = vec![combo(
        Mods {
            cmd: true,
            shift: true,
            ..NONE
        },
        "e",
    )];
    assert_eq!(keys::parse("cmd+shift+e"), Ok(want.clone()));
    assert_eq!(keys::parse("Shift+CMD+E"), Ok(want.clone()));
    assert_eq!(keys::parse("command + shift + e"), Ok(want.clone()));
    assert_eq!(keys::parse("⌘⇧E"), Ok(want));
}

#[test]
fn aliases_and_named_keys() {
    let parsed = keys::parse("opt+f5").unwrap();
    assert!(parsed[0].mods.option);
    assert_eq!(parsed[0].key, "f5");
    assert_eq!(keys::parse("alt+ctrl+left").unwrap()[0].key, "left");
    assert_eq!(keys::parse("control+enter").unwrap()[0].key, "return");
    assert_eq!(keys::parse("esc").unwrap()[0].key, "escape");
    assert_eq!(keys::parse("backspace").unwrap()[0].key, "delete");
    assert_eq!(keys::parse("e").unwrap(), vec![combo(NONE, "e")]);
}

#[test]
fn a_short_sequence_is_up_to_four_combos() {
    let seq = keys::parse("cmd+k cmd+s").unwrap();
    assert_eq!(seq.len(), 2);
    assert_eq!(seq[1].key, "s");
    assert!(keys::parse("a b c d").is_ok());
    assert_eq!(keys::parse("a b c d e"), Err(KeyError::TooLong));
}

#[test]
fn bad_shortcuts_say_what_is_wrong() {
    assert_eq!(keys::parse(""), Err(KeyError::Empty));
    assert_eq!(keys::parse("   "), Err(KeyError::Empty));
    assert_eq!(
        keys::parse("cmd+shift"),
        Err(KeyError::NoKey("cmd+shift".into()))
    );
    assert_eq!(
        keys::parse("cmd+e+f"),
        Err(KeyError::TwoKeys("cmd+e+f".into()))
    );
    assert_eq!(
        keys::parse("cmd+banana"),
        Err(KeyError::Unknown("banana".into()))
    );
    assert_eq!(
        keys::parse("cmd+cmd+e"),
        Err(KeyError::Repeated("cmd+cmd+e".into()))
    );
    assert_eq!(
        KeyError::Unknown("banana".into()).to_string(),
        "\"banana\" isn't a key name (try a letter, a digit, f1 to f12, space, return, escape, tab, delete, or an arrow)"
    );
}

#[test]
fn shortcuts_are_written_back_in_one_form() {
    assert_eq!(
        keys::canonical(&keys::parse("Shift+CMD+E").unwrap()),
        "cmd+shift+e"
    );
    assert_eq!(
        keys::canonical(&keys::parse("ctrl+opt+shift+cmd+f12  ⌘K").unwrap()),
        "cmd+option+control+shift+f12 cmd+k"
    );
    // Canonical text parses back to the same thing.
    let text = "cmd+option+left";
    assert_eq!(keys::canonical(&keys::parse(text).unwrap()), text);
}

#[test]
fn the_key_table_covers_what_shortcuts_use() {
    for c in 'a'..='z' {
        assert!(keys::code_of(&c.to_string()).is_some(), "{c}");
    }
    for d in 0..=9 {
        assert!(keys::code_of(&d.to_string()).is_some(), "{d}");
    }
    for f in 1..=12 {
        assert!(keys::code_of(&format!("f{f}")).is_some(), "f{f}");
    }
    for name in [
        "space",
        "return",
        "tab",
        "escape",
        "delete",
        "forwarddelete",
        "left",
        "right",
        "up",
        "down",
        "home",
        "end",
        "pageup",
        "pagedown",
        "minus",
        "equal",
        "leftbracket",
        "rightbracket",
        "backslash",
        "semicolon",
        "quote",
        "comma",
        "period",
        "slash",
        "grave",
    ] {
        assert!(keys::code_of(name).is_some(), "{name}");
    }
    // Spot checks against the ANSI virtual key codes (HIToolbox Events.h).
    assert_eq!(keys::code_of("a"), Some(0x00));
    assert_eq!(keys::code_of("e"), Some(0x0E));
    assert_eq!(keys::code_of("4"), Some(0x15));
    assert_eq!(keys::code_of("return"), Some(0x24));
    assert_eq!(keys::code_of("space"), Some(0x31));
    assert_eq!(keys::code_of("f5"), Some(0x60));
    assert_eq!(keys::code_of("left"), Some(0x7B));
    // Codes are unique.
    let mut codes: Vec<u16> = keys::names()
        .iter()
        .filter_map(|n| keys::code_of(n))
        .collect();
    let n = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), n, "two names share a code");
}

/// The shortcut field (`ui/keycodes.json`, KeyboardEvent.code to key name)
/// only produces names this module knows, and covers every one of them.
#[test]
fn the_ui_shortcut_field_uses_the_same_key_names() {
    let table: std::collections::BTreeMap<String, String> =
        serde_json::from_str(include_str!("../../ui/keycodes.json")).unwrap();
    for (code, name) in &table {
        assert!(keys::code_of(name).is_some(), "{code} -> {name}");
        assert_eq!(
            keys::canonical(&keys::parse(name).unwrap()),
            *name,
            "{name} isn't canonical"
        );
    }
    for name in keys::names() {
        assert!(
            table.values().any(|n| n == name),
            "the UI can't record {name}"
        );
    }
}
