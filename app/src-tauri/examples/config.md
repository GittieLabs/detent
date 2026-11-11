# Config file reference (schema v1)

Detent reads `~/Library/Application Support/Detent/config.json`. In M2 you write it by hand; start from `config.example.json` next to this file. From M3 the app also writes it (pairing, renaming, ignoring): it checks the whole config first, writes it atomically, and saves it in one canonical form (two space indents, USB IDs in hex, defaults written out), so your own formatting is replaced on the first save (D-069). The running app picks up its own saves without a relaunch; an edit you make by hand still needs a relaunch. JSON has no comments, and unknown fields are errors, so the notes live here. Every error names the field it's about, for example `mappings[2].action.level: must be from 0 to 1`.

## Top level

| Field | Required | Meaning |
| --- | --- | --- |
| `version` | yes | Always `1` today. An older version is migrated on load and the old file kept as `config.json.v<N>.bak`; a newer one is refused with a clear message and left untouched. |
| `devices` | no | Paired units. Detent opens these and nothing else. |
| `ignored` | no | Units Detent never opens or offers to pair (`vid`, `pid`, `serial`). |
| `mappings` | no | What each control does. |
| `paused` | no | `true` while the tray's Pause is on: presses are logged but run nothing. Default `false`. Detent writes it; there's no need to edit it by hand. |

## `devices[]`

| Field | Meaning |
| --- | --- |
| `name` | Your name for it. |
| `vid`, `pid` | USB IDs, as `"0x303A"` (any case) or a number. Detent boards are `0x303A` / `0x8FDE` until the Espressif PID lands. |
| `serial` | The USB serial, 12 hex chars on Detent boards (shown on the OLED's Setup screen and by `just probe list`). Must be unique. |
| `menu` | Optional. The unit's own OLED menu items, after the device's (Volume, Mapped, Setup, Device info): an ordered list of up to 8 `{ "label": "Mail", "action": { ... } }`. Labels are 1 to 16 printable ASCII characters; the action is any from the list below. Picking one on the device runs its action (D-086). |

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
| `send_shortcut` | `keys`: a combo such as `cmd+shift+e`, or up to 4 separated by spaces (`cmd+k cmd+s`). Modifiers `cmd`, `option`, `control`, `shift`; keys are letters, digits, `f1` to `f12`, `space`, `return`, `escape`, `tab`, `delete`, arrows, and punctuation names (`minus`, `comma`...), on US key positions (D-082) | Sends the keys to the frontmost app. Needs Accessibility; refused, sending nothing, while Secure Input is on (a password field has focus). |
| `open_dashboard` | `url` (optional, http or https, default `https://gittielabs.github.io/detent/`, the documentation site; configs saved before it keep the repo page they were written with), `display` (optional, a display's name as macOS shows it; the main display if left out or not connected), `mode` (optional, `fullscreen` (default) or `window`), `close_without_device` (optional, default `true`) | Shows the page on that display without taking focus: covering it, or in a resizable window. A second press or the close button hides it (full screen: shown when the mouse moves; a window: its own); with `close_without_device`, so does unplugging the device (D-088 to D-090). |
| `applescript` | `source`, `timeout_ms` (optional, 100 to 60000, default 5000) | Runs AppleScript. The first run against an app shows macOS's Automation prompt naming Detent. |

