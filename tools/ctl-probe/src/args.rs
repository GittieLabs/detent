//! Command line parsing. No dependencies; the surface is small (D-041).

use detent_protocol::codec::InjectEvent;

use crate::inject;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Print decoded events; reopen after unplug. With `count_hellos`, exit 0
    /// once that many connections have each produced a HELLO.
    Watch {
        raw: bool,
        count_hellos: Option<u32>,
        /// Sent as TEST_INJECT after each connect (debug firmware only).
        inject: Option<Vec<InjectEvent>>,
        /// Exit 0 after this many seconds.
        exit_after: Option<u64>,
    },
    /// List every HID entry with Detent's VID.
    List,
    /// Print undecoded reports for `secs` seconds.
    Raw { secs: u64 },
}

pub const USAGE: &str = "usage:
  ctl-probe [--raw] [--count-hellos N]   print decoded events; reopen after unplug
            [--inject SCRIPT]            after connecting, send TEST_INJECT (debug firmware)
            [--exit-after SECS]          exit 0 after SECS (not with --count-hellos)
  SCRIPT: \"B1:down B1:up@120 E5:+2@50\"; B<id>:<down|up|long>, E<id>:<+n|-n>, @ms waits after the previous event
  ctl-probe list                         list HID entries with Detent's VID
  ctl-probe raw [--secs N]               print undecoded reports (default 10 s)";

pub fn parse(args: &[String]) -> Result<Command, String> {
    let mut it = args.iter().map(String::as_str).peekable();
    let sub = match it.peek() {
        Some(&s) if !s.starts_with("--") => {
            it.next();
            s
        }
        _ => "watch",
    };
    let mut raw = false;
    let mut count_hellos = None;
    let mut inject = None;
    let mut exit_after = None;
    let mut secs = 10;
    while let Some(arg) = it.next() {
        match (sub, arg) {
            ("watch", "--raw") => raw = true,
            ("watch", "--count-hellos") => {
                let v = it.next().ok_or("--count-hellos needs a value")?;
                let n: u32 = v
                    .parse()
                    .map_err(|_| format!("bad --count-hellos value: {v}"))?;
                if n == 0 {
                    return Err("--count-hellos must be at least 1".into());
                }
                count_hellos = Some(n);
            }
            ("watch", "--inject") => {
                let v = it.next().ok_or("--inject needs a script")?;
                inject = Some(inject::parse(v)?);
            }
            ("watch", "--exit-after") => {
                let v = it.next().ok_or("--exit-after needs a value")?;
                exit_after = Some(
                    v.parse()
                        .map_err(|_| format!("bad --exit-after value: {v}"))?,
                );
            }
            ("raw", "--secs") => {
                let v = it.next().ok_or("--secs needs a value")?;
                secs = v.parse().map_err(|_| format!("bad --secs value: {v}"))?;
            }
            (_, other) => return Err(format!("unexpected argument: {other}")),
        }
    }
    match sub {
        "watch" if count_hellos.is_some() && exit_after.is_some() => {
            Err("--exit-after can't be combined with --count-hellos".into())
        }
        "watch" => Ok(Command::Watch {
            raw,
            count_hellos,
            inject,
            exit_after,
        }),
        "list" => Ok(Command::List),
        "raw" => Ok(Command::Raw { secs }),
        other => Err(format!("unknown command: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Command, String> {
        let args: Vec<String> = s.split_whitespace().map(String::from).collect();
        parse(&args)
    }

    fn watch(raw: bool, count_hellos: Option<u32>) -> Command {
        Command::Watch {
            raw,
            count_hellos,
            inject: None,
            exit_after: None,
        }
    }

    #[test]
    fn watch_is_the_default() {
        assert_eq!(p(""), Ok(watch(false, None)));
        assert_eq!(p("--raw"), Ok(watch(true, None)));
        assert_eq!(p("--count-hellos 20 --raw"), Ok(watch(true, Some(20))));
    }

    #[test]
    fn inject_and_exit_after() {
        // The script is one argument, as the shell passes a quoted string.
        let args: Vec<String> = ["--inject", "B1:down B1:up@120", "--exit-after", "3"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let Ok(Command::Watch {
            inject: Some(evs),
            exit_after: Some(3),
            ..
        }) = parse(&args)
        else {
            panic!("{:?}", parse(&args));
        };
        assert_eq!(evs.len(), 2);
        assert!(p("--inject B1:press").is_err());
        assert!(p("--exit-after x").is_err());
        assert!(p("--count-hellos 2 --exit-after 3").is_err());
    }

    #[test]
    fn subcommands() {
        assert_eq!(p("list"), Ok(Command::List));
        assert_eq!(p("raw"), Ok(Command::Raw { secs: 10 }));
        assert_eq!(p("raw --secs 3"), Ok(Command::Raw { secs: 3 }));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            "--count-hellos",
            "--count-hellos 0",
            "--count-hellos x",
            "--secs 3",
            "raw --raw",
            "list --raw",
            "raw --secs",
            "send E",
            "--bogus",
        ] {
            assert!(p(bad).is_err(), "{bad}");
        }
    }
}
