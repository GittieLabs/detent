# The config file

Detent keeps everything in `~/Library/Application Support/Detent/config.json`. The app writes it whenever you change something, atomically and in one tidy form. You can edit it by hand while Detent is quit; JSON has no comments, and unknown fields are errors, so the notes are here. Every error names the field it's about, for example `mappings[2].action.level: must be from 0 to 1`.

If the file can't be read, Detent never overwrites it: Devices explains the problem and offers **Start fresh**, which moves the broken file aside as `config.json.bad-<time>`.

## Top level

| Field | Required | Meaning |
| --- | --- | --- |
| `version` | yes | `2`. An older version is upgraded on load and the original kept as `config.json.v<N>.bak`; a newer one is refused and left alone. |
| `devices` | no | Paired devices. Detent opens these and nothing else. |
| `ignored` | no | Devices Detent never opens or offers to pair (`vid`, `pid`, `serial`; matched by `serial`). |
| `mappings` | no | What each control does. |
| `paused` | no | `true` while Pause is on. |

## `devices[]`

| Field | Meaning |
| --- | --- |
| `name` | Your name for it. |
| `vid`, `pid` | USB IDs when it was paired, as `"0x303A"` or a number. For your reference only: Detent knows a device by its serial, so reflashing it with other IDs keeps its mappings. |
| `serial` | The device's USB serial (shown by Setup on its OLED). This is how Detent recognizes it. |
| `menu` | Optional: up to 8 `{ "label": "Mail", "action": { ... } }` for the [device menu](device-menu.md). |

## `mappings[]`

| Field | Meaning |
| --- | --- |
| `device` | The `serial` of an entry in `devices`. |
| `control` | The control's ID as the device reports it (Debug shows them). |
| `trigger` | `press` or `long_press` for buttons; `clockwise` or `counterclockwise` for encoders. |
| `scope` | `"everywhere"`, or `{ "app": "com.apple.Safari" }`. |
| `action` | One of the actions below. |

Two mappings with the same device, control, trigger and scope are an error.

## Actions

| `type` | Fields |
| --- | --- |
| `open_app` | `bundle_id`. Find one with `osascript -e 'id of app "Mail"'`. |
| `volume_up`, `volume_down` | `step`, optional, more than 0 up to 1, default `0.0625`. |
| `volume_set` | `level`, 0 to 1. |
| `mute_toggle` | none |
| `send_shortcut` | `keys`: `cmd+shift+e`, or up to four combos separated by spaces. Modifiers `cmd`, `option`, `control`, `shift`; keys are letters, digits, `f1` to `f12`, `space`, `return`, `escape`, `tab`, `delete`, arrows and punctuation names (`minus`, `comma`), on US key positions. |
| `open_dashboard` | `url` (optional, http or https; defaults to this site), `display` (optional, a display's name; the main one otherwise), `mode` (`fullscreen` or `window`), `close_without_device` (default `true`). |
| `applescript` | `source`, and `timeout_ms` (optional, 100 to 60000, default 5000). |

## Example

```json
{
  "version": 2,
  "devices": [
    { "name": "Desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC" }
  ],
  "mappings": [
    { "device": "E072A1FC4EDC", "control": 1, "trigger": "press", "scope": "everywhere",
      "action": { "type": "open_app", "bundle_id": "com.apple.mail" } },
    { "device": "E072A1FC4EDC", "control": 1, "trigger": "press", "scope": { "app": "com.apple.Safari" },
      "action": { "type": "mute_toggle" } }
  ]
}
```
