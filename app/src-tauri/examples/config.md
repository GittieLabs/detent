# Config file reference (schema v1)

Detent reads `~/Library/Application Support/Detent/config.json`. In M2 you write it by hand; start from `config.example.json` next to this file. JSON has no comments, and unknown fields are errors, so the notes live here. Every error names the field it's about, for example `mappings[2].action.level: must be from 0 to 1`.

## Top level

| Field | Required | Meaning |
| --- | --- | --- |
| `version` | yes | Always `1`. Other versions are refused with a clear message. |
| `devices` | no | Paired units. Detent opens these and nothing else. |
| `ignored` | no | Units Detent never opens or offers to pair (`vid`, `pid`, `serial`). |
| `mappings` | no | What each control does. |

## `devices[]`

| Field | Meaning |
| --- | --- |
| `name` | Your name for it. |
| `vid`, `pid` | USB IDs, as `"0x303A"` (any case) or a number. Detent boards are `0x303A` / `0x8FDE` until the Espressif PID lands. |
| `serial` | The USB serial, 12 hex chars on Detent boards (shown on the OLED's Setup screen and by `just probe list`). Must be unique. |

## `mappings[]`

| Field | Meaning |
| --- | --- |
| `device` | The `serial` of an entry in `devices`. |
| `control` | The control ID from HELLO (`just probe` shows it). On the first board: buttons `1` to `4`, encoder `5`, encoder push switch `6`. |
| `trigger` | `press` or `long_press` for buttons (fires on press, never on release); `clockwise` or `counterclockwise` for the encoder (runs once per detent). The encoder and its switch only reach the app in Mapped mode. |
| `scope` | `"everywhere"`, or `{ "app": "com.apple.Safari" }` for only while that app is frontmost. An app mapping wins over an everywhere one for the same control and trigger. |
| `action` | One of the actions below. |

Two mappings with the same device, control, trigger, and scope are an error: Detent couldn't choose.

## Actions

| `type` | Fields | Does |
| --- | --- | --- |
| `open_app` | `bundle_id` | Opens the app or brings it forward. Find an ID with `osascript -e 'id of app "Mail"'`. |
| `volume_up`, `volume_down` | `step` (optional, more than 0 up to 1, default `0.0625`, one key press) | Changes the output volume. |
| `volume_set` | `level` (0 to 1) | Sets the output volume. |
| `mute_toggle` | none | Toggles mute. |
| `applescript` | `source`, `timeout_ms` (optional, 100 to 60000, default 5000) | Runs AppleScript. The first run against an app shows macOS's Automation prompt naming Detent. |

Open dashboard is deferred until the app's dashboard is defined (D-055).
