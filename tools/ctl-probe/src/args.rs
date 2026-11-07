//! Command line parsing. No dependencies; the surface is small.

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Print decoded events; reopen after unplug. With `count_hellos`, exit 0
    /// once that many connections have each produced a HELLO.
    Watch {
        raw: bool,
        count_hellos: Option<u32>,
    },
    /// List every HID entry with Detent's VID.
    List,
    /// Print undecoded reports for `secs` seconds.
    Raw { secs: u64 },
}

pub const USAGE: &str = "usage:
  ctl-probe [--raw] [--count-hellos N]   print decoded events; reopen after unplug
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
            ("raw", "--secs") => {
                let v = it.next().ok_or("--secs needs a value")?;
                secs = v.parse().map_err(|_| format!("bad --secs value: {v}"))?;
            }
            (_, other) => return Err(format!("unexpected argument: {other}")),
        }
    }
    match sub {
        "watch" => Ok(Command::Watch { raw, count_hellos }),
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

    #[test]
    fn watch_is_the_default() {
        assert_eq!(
            p(""),
            Ok(Command::Watch {
                raw: false,
                count_hellos: None
            })
        );
        assert_eq!(
            p("--raw"),
            Ok(Command::Watch {
                raw: true,
                count_hellos: None
            })
        );
        assert_eq!(
            p("--count-hellos 20 --raw"),
            Ok(Command::Watch {
                raw: true,
                count_hellos: Some(20)
            })
        );
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
