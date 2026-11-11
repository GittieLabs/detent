# Boards

The boards Detent runs on and the USB IDs each enumerates with. The source of
truth is the models table in `protocol/src/ids.rs` (`MODELS`); the app, the
`ctl-probe` tool and the firmware header (`DETENT_USB_*`, `DETENT_PICO_*`)
all come from it. The wire format is the same on every board
(`docs/protocol.md`).

This list lives here, not in the Identity section of `protocol.md`, until
Keith decides whether that edit bumps the protocol version (open question in
`docs/roadmap.md`).

| Board | Firmware env | VID:PID | Model in the table | Status |
|---|---|---|---|---|
| Seeed XIAO ESP32-S3 | `xiao_esp32s3` | `303A:8FDE` (PLACEHOLDER PID, D-014) | Dev | Shipping board |
| Adafruit ESP32-S3 Feather (4 MB flash, 2 MB PSRAM) | `feather_esp32s3` | `303A:8FDE`, shared with the XIAO for now | Dev | BOOT as button 1 and no encoder until headers (D-097); own PID or variant is an open question |
| Raspberry Pi Pico (RP2040) | `pico`, `pico_spike` | `1209:0001` (pid.codes testing pair, PLACEHOLDER) | Pico | USB spike passed (D-092); full board waits on soldered headers |

A board's layout (its buttons, encoders and which button is each encoder's push switch) is set in its header in `firmware/include/boards/` and reported in HELLO (D-094). The USB IDs only choose the name shown and the starter template, so a different layout needs no new ID.

Adding a board: a header in `firmware/include/boards/`, an env in
`firmware/platformio.ini` and CI, and a row in `MODELS` if it has its own IDs.
A board that isn't in `MODELS` can still be probed with
`DETENT_PROBE_IDS=VID:PID ctl-probe`.
