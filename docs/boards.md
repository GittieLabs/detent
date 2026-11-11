# Boards

The boards Detent runs on and the USB IDs each enumerates with. The source of
truth is the models table in `protocol/src/ids.rs` (`MODELS`); the app, the
`ctl-probe` tool and the firmware header (`DETENT_USB_*`, `DETENT_PICO_*`)
all come from it. The wire format is the same on every board
(`docs/protocol.md`).

This list lives here, not in the Identity section of `protocol.md`, until
Keith decides whether that edit bumps the protocol version (open question in
`docs/roadmap.md`).

| Board | Firmware env | VID:PID | Model in the table | Encoder push | Status |
|---|---|---|---|---|---|
| Seeed XIAO ESP32-S3 | `xiao_esp32s3` | `303A:8FDE` (PLACEHOLDER PID, D-014) | Dev | control 6 | Shipping board |
| ESP32-S3 DevKitC class | `devkitc_s3` | `303A:8FDE`, shared with the XIAO for now | Dev | control 6 | PLACEHOLDER pins; own PID or variant is an open question |
| Raspberry Pi Pico (RP2040) | `pico`, `pico_spike` | `1209:0001` (pid.codes testing pair, PLACEHOLDER) | Pico | control 6 | USB spike passed (D-092); full board waits on soldered headers |

Adding a board: a header in `firmware/include/boards/`, an env in
`firmware/platformio.ini` and CI, and a row in `MODELS` if it has its own IDs.
A board that isn't in `MODELS` can still be probed with
`DETENT_PROBE_IDS=VID:PID ctl-probe`.
