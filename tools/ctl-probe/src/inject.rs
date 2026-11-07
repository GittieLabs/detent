//! `--inject` scripts: whitespace separated events, sent as one TEST_INJECT.
//!
//!   B<id>:<down|up|long>[@<ms>]   button event
//!   E<id>:<+n|-n>[@<ms>]          encoder event, n nonzero
//!
//! `@<ms>` is the wait before that event, after the previous one (default 0).
//! Example: "B1:down B1:up@120 E5:+2@50". Debug firmware only.

use detent_protocol::codec::{ButtonAction, InjectEvent, Injected, MAX_INJECT_EVENTS};

pub fn parse(script: &str) -> Result<Vec<InjectEvent>, String> {
    let events = script
        .split_whitespace()
        .map(parse_event)
        .collect::<Result<Vec<_>, _>>()?;
    match events.len() {
        0 => Err("inject script is empty".into()),
        n if n > MAX_INJECT_EVENTS => Err(format!(
            "inject script has {n} events, max {MAX_INJECT_EVENTS}"
        )),
        _ => Ok(events),
    }
}

fn parse_event(token: &str) -> Result<InjectEvent, String> {
    let bad = || {
        format!(
            "bad inject event {token:?}; expected B<id>:<down|up|long>[@ms] or E<id>:<+n|-n>[@ms]"
        )
    };
    let (body, delay_ms) = match token.split_once('@') {
        Some((b, d)) => (b, d.parse::<u16>().map_err(|_| bad())?),
        None => (token, 0),
    };
    let (target, value) = body.split_once(':').ok_or_else(bad)?;
    let mut chars = target.chars();
    let kind = chars.next().ok_or_else(bad)?.to_ascii_uppercase();
    let control: u8 = chars.as_str().parse().map_err(|_| bad())?;
    let event = match kind {
        'B' => Injected::Button {
            control,
            action: match value {
                "down" => ButtonAction::Down,
                "up" => ButtonAction::Up,
                "long" => ButtonAction::LongPress,
                _ => return Err(bad()),
            },
        },
        'E' => {
            let delta: i8 = value.parse().map_err(|_| bad())?;
            if delta == 0 || !(value.starts_with('+') || value.starts_with('-')) {
                return Err(bad());
            }
            Injected::Encoder { control, delta }
        }
        _ => return Err(bad()),
    };
    Ok(InjectEvent { event, delay_ms })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(control: u8, action: ButtonAction, delay_ms: u16) -> InjectEvent {
        InjectEvent {
            event: Injected::Button { control, action },
            delay_ms,
        }
    }

    #[test]
    fn parses_buttons_and_encoders() {
        assert_eq!(
            parse("B1:down B1:up@120 b2:long E5:+2@50 e5:-3").unwrap(),
            vec![
                button(1, ButtonAction::Down, 0),
                button(1, ButtonAction::Up, 120),
                button(2, ButtonAction::LongPress, 0),
                InjectEvent {
                    event: Injected::Encoder {
                        control: 5,
                        delta: 2
                    },
                    delay_ms: 50
                },
                InjectEvent {
                    event: Injected::Encoder {
                        control: 5,
                        delta: -3
                    },
                    delay_ms: 0
                },
            ]
        );
    }

    #[test]
    fn enforces_one_to_eleven_events() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
        let eleven = ["B1:down"; 11].join(" ");
        assert_eq!(parse(&eleven).unwrap().len(), 11);
        let twelve = ["B1:down"; 12].join(" ");
        assert!(parse(&twelve).unwrap_err().contains("max 11"));
    }

    #[test]
    fn rejects_bad_tokens() {
        for bad in [
            "B1",
            "B1:press",
            "X1:down",
            "B:down",
            "B256:down",
            "Bx:down",
            "E5:0",
            "E5:+0",
            "E5:2",
            "E5:+200",
            "B1:down@",
            "B1:down@70000",
            "B1:down@-5",
            ":down",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
