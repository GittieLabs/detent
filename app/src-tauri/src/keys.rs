//! Shortcut names (D-082): `cmd+shift+e`, `f5`, or a short sequence such as
//! `cmd+k cmd+s`, parsed into combos on US ANSI virtual key codes. Pure;
//! `macos::post_combo` sends them.

use std::fmt;

/// The most combos one Send shortcut may hold.
pub const MAX_COMBOS: usize = 4;

/// Modifier keys held for a combo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub cmd: bool,
    pub shift: bool,
    pub option: bool,
    pub control: bool,
}

/// One key with its modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combo {
    pub mods: Mods,
    /// Canonical key name (`e`, `f5`, `left`).
    pub key: String,
    /// US ANSI virtual key code (HIToolbox `Events.h`).
    pub code: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    Empty,
    TooLong,
    /// Only modifiers.
    NoKey(String),
    /// More than one non modifier key in a combo.
    TwoKeys(String),
    Unknown(String),
    /// A modifier named twice.
    Repeated(String),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "no keys given"),
            Self::TooLong => write!(f, "at most {MAX_COMBOS} key combos in a row"),
            Self::NoKey(c) => write!(f, "{c:?} has modifiers but no key"),
            Self::TwoKeys(c) => write!(f, "{c:?} has two keys; use one key plus modifiers"),
            Self::Unknown(k) => write!(
                f,
                "{k:?} isn't a key name (try a letter, a digit, f1 to f12, space, return, \
                 escape, tab, delete, or an arrow)"
            ),
            Self::Repeated(c) => write!(f, "{c:?} names a modifier twice"),
        }
    }
}

impl std::error::Error for KeyError {}

/// Canonical names and their US ANSI key codes.
const KEYS: &[(&str, u16)] = &[
    ("a", 0x00),
    ("s", 0x01),
    ("d", 0x02),
    ("f", 0x03),
    ("h", 0x04),
    ("g", 0x05),
    ("z", 0x06),
    ("x", 0x07),
    ("c", 0x08),
    ("v", 0x09),
    ("b", 0x0B),
    ("q", 0x0C),
    ("w", 0x0D),
    ("e", 0x0E),
    ("r", 0x0F),
    ("y", 0x10),
    ("t", 0x11),
    ("1", 0x12),
    ("2", 0x13),
    ("3", 0x14),
    ("4", 0x15),
    ("6", 0x16),
    ("5", 0x17),
    ("equal", 0x18),
    ("9", 0x19),
    ("7", 0x1A),
    ("minus", 0x1B),
    ("8", 0x1C),
    ("0", 0x1D),
    ("rightbracket", 0x1E),
    ("o", 0x1F),
    ("u", 0x20),
    ("leftbracket", 0x21),
    ("i", 0x22),
    ("p", 0x23),
    ("return", 0x24),
    ("l", 0x25),
    ("j", 0x26),
    ("quote", 0x27),
    ("k", 0x28),
    ("semicolon", 0x29),
    ("backslash", 0x2A),
    ("comma", 0x2B),
    ("slash", 0x2C),
    ("n", 0x2D),
    ("m", 0x2E),
    ("period", 0x2F),
    ("tab", 0x30),
    ("space", 0x31),
    ("grave", 0x32),
    ("delete", 0x33),
    ("escape", 0x35),
    ("f5", 0x60),
    ("f6", 0x61),
    ("f7", 0x62),
    ("f3", 0x63),
    ("f8", 0x64),
    ("f9", 0x65),
    ("f11", 0x67),
    ("f10", 0x6D),
    ("f12", 0x6F),
    ("home", 0x73),
    ("pageup", 0x74),
    ("forwarddelete", 0x75),
    ("f4", 0x76),
    ("end", 0x77),
    ("f2", 0x78),
    ("pagedown", 0x79),
    ("f1", 0x7A),
    ("left", 0x7B),
    ("right", 0x7C),
    ("down", 0x7D),
    ("up", 0x7E),
];

/// Other spellings of keys.
const KEY_ALIASES: &[(&str, &str)] = &[
    ("enter", "return"),
    ("esc", "escape"),
    ("backspace", "delete"),
    ("del", "forwarddelete"),
    ("-", "minus"),
    ("=", "equal"),
    ("[", "leftbracket"),
    ("]", "rightbracket"),
    ("\\", "backslash"),
    (";", "semicolon"),
    ("'", "quote"),
    (",", "comma"),
    (".", "period"),
    ("/", "slash"),
    ("`", "grave"),
];

/// Every canonical key name.
pub fn names() -> Vec<&'static str> {
    KEYS.iter().map(|(n, _)| *n).collect()
}

/// The key code for a canonical name or alias, ignoring case.
pub fn code_of(name: &str) -> Option<u16> {
    canonical_key(name).and_then(|n| KEYS.iter().find(|(k, _)| *k == n).map(|(_, c)| *c))
}

fn canonical_key(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    let lower = KEY_ALIASES
        .iter()
        .find(|(a, _)| *a == lower)
        .map_or(lower.as_str(), |(_, n)| *n);
    KEYS.iter().find(|(k, _)| *k == lower).map(|(k, _)| *k)
}

/// Which modifier a word names, if any.
fn modifier(word: &str) -> Option<fn(&mut Mods) -> &mut bool> {
    match word.to_lowercase().as_str() {
        "cmd" | "command" => Some(|m| &mut m.cmd),
        "shift" => Some(|m| &mut m.shift),
        "option" | "opt" | "alt" => Some(|m| &mut m.option),
        "control" | "ctrl" => Some(|m| &mut m.control),
        _ => None,
    }
}

/// Splits text into combo strings. `⌘⇧E` style symbols become words, and
/// spaces around `+` don't split a combo.
fn combos_of(text: &str) -> Vec<String> {
    let mut spelled = String::new();
    for ch in text.chars() {
        match ch {
            '⌘' => spelled.push_str("cmd+"),
            '⇧' => spelled.push_str("shift+"),
            '⌥' => spelled.push_str("option+"),
            '⌃' => spelled.push_str("control+"),
            _ => spelled.push(ch),
        }
    }
    let mut out: Vec<String> = Vec::new();
    let mut joining = false;
    for word in spelled.split_whitespace() {
        let starts_plus = word.starts_with('+');
        match out.last_mut() {
            Some(last) if joining || starts_plus => last.push_str(word),
            _ => out.push(word.to_owned()),
        }
        joining = word.ends_with('+');
    }
    out
}

fn parse_combo(text: &str) -> Result<Combo, KeyError> {
    let mut mods = Mods::default();
    let mut key: Option<&'static str> = None;
    for part in text.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(flag) = modifier(part) {
            let held = flag(&mut mods);
            if *held {
                return Err(KeyError::Repeated(text.to_owned()));
            }
            *held = true;
        } else {
            let name = canonical_key(part).ok_or_else(|| KeyError::Unknown(part.to_owned()))?;
            if key.is_some() {
                return Err(KeyError::TwoKeys(text.to_owned()));
            }
            key = Some(name);
        }
    }
    let key = key.ok_or_else(|| KeyError::NoKey(text.to_owned()))?;
    Ok(Combo {
        mods,
        key: key.to_owned(),
        code: code_of(key).expect("canonical names are in the table"),
    })
}

/// Parses a shortcut: one combo, or up to [`MAX_COMBOS`] separated by spaces.
pub fn parse(text: &str) -> Result<Vec<Combo>, KeyError> {
    let combos = combos_of(text);
    if combos.is_empty() {
        return Err(KeyError::Empty);
    }
    if combos.len() > MAX_COMBOS {
        return Err(KeyError::TooLong);
    }
    combos.iter().map(|c| parse_combo(c)).collect()
}

/// One form for a parsed shortcut: modifiers in the order cmd, option,
/// control, shift, then the key; combos separated by a space.
pub fn canonical(combos: &[Combo]) -> String {
    combos
        .iter()
        .map(|c| {
            let mut parts: Vec<&str> = Vec::new();
            for (on, name) in [
                (c.mods.cmd, "cmd"),
                (c.mods.option, "option"),
                (c.mods.control, "control"),
                (c.mods.shift, "shift"),
            ] {
                if on {
                    parts.push(name);
                }
            }
            parts.push(&c.key);
            parts.join("+")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Why a shortcut wasn't sent. Checked before the first key, so a refused
/// sequence sends nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Detent doesn't hold the Accessibility grant.
    NotTrusted,
    /// A password field (or similar) has Secure Input on.
    SecureInput,
    /// macOS couldn't create a key event.
    NoEvent,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTrusted => write!(f, "Detent needs Accessibility to send shortcuts"),
            Self::SecureInput => write!(
                f,
                "Secure Input is on (a password field has focus), so macOS blocks sent keys"
            ),
            Self::NoEvent => write!(f, "macOS couldn't create the key event"),
        }
    }
}

impl std::error::Error for Refusal {}

/// Whether to refuse, from the grant and Secure Input: the grant first.
pub fn refusal(trusted: bool, secure_input: bool) -> Option<Refusal> {
    if !trusted {
        Some(Refusal::NotTrusted)
    } else if secure_input {
        Some(Refusal::SecureInput)
    } else {
        None
    }
}
