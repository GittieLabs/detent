# Test and Debug

## Test

The Test screen shows the device live. Pressing a control lights it and lists what fired: the control, the app in front, the mapping that applied and what happened.

**Dry run** is on while Test is showing: presses show what would run and nothing runs. Turn it off to run actions for real; it comes back on next time you open Test.

## Debug

Tick **Debug** on the Test screen, or start Detent with `--debug`:

```
open -a Detent --args --debug
```

Debug adds:

- **A live event log**: every report the device sends (with its raw bytes and what they decode to, heartbeats included), every mapping resolved and every action's result. It keeps the latest 500.
- **Last HELLO**: the device's firmware, model and controls as it described itself.
- **Permissions**: Accessibility and how Automation works.
- **Export logs**: writes `Detent logs <date>.zip` to your Desktop and shows it in Finder.

The export holds Detent's log (and the previous one), your config and a few facts (Detent and macOS versions, each device's serial, firmware and controls). Device names, menu labels, AppleScript text, dashboard URLs and display names are replaced, so it's made to attach to a GitHub issue. Words that are no longer in your config, such as a device's old name, can still be in older log lines: have a look before you share it.

## The log

Detent logs to `~/Library/Logs/Detent/detent.log`. Past 1 MB it moves to `detent.log.1` and a new one starts.
