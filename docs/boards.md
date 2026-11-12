# Boards

The boards Detent runs on and the USB IDs each enumerates with. The source of
truth is the models table in `protocol/src/ids.rs` (`MODELS`); the firmware
header gives each build its IDs and model name by its board flag
(`DETENT_USB_VID`, `DETENT_USB_PID`, `DETENT_MODEL_NAME`). The wire format is
the same on every board (`docs/protocol.md`).

The IDs don't decide what counts as a Detent (D-112): the app and `ctl-probe`
find a unit by its vendor channel, a product string starting `Detent `, a
serial and a HELLO, and match a paired unit by serial. The VIDs are the chip
vendors'; the PIDs are chosen locally, with no vendor allocation until Detent
has real adoption.

| Board | Firmware env | VID:PID | Product string | Status |
|---|---|---|---|---|
| Seeed XIAO ESP32-S3 | `xiao_esp32s3` | `303A:8FDE` | `Detent XIAO` | Shipping board |
| Adafruit ESP32-S3 Feather (4 MB flash, 2 MB PSRAM) | `feather_esp32s3` | `303A:8FDF` | `Detent Feather` | BOOT as button 1 and no encoder until headers (D-097) |
| ESP32-S3 DevKitC-1 class, N16R8 (16 MB flash, 8 MB octal PSRAM) | `devkitc_esp32s3` | `303A:8FE0` | `Detent DevKitC` | Env and pin map built 2026-11-12 (D-118); Detent runs on the port labelled USB |
| Raspberry Pi Pico (RP2040) | `pico`, `pico_spike` | `2E8A:F0DE` | `Detent Pico` | USB spike passed (D-092); full board waits on soldered headers |

Firmware 0.2.0 and earlier enumerate as `Detent Dev` on `303A:8FDE` (all
ESP32-S3 boards) or `1209:0001` (the Pico) and speak protocol v2: the app
shows them as needing a firmware update.

A board's layout (its buttons, encoders and which button is each encoder's push switch) is set in its header in `firmware/include/boards/` and reported in HELLO (D-094). The USB IDs only choose the name shown and the starter template, so a different layout needs no new ID.

Adding a board: a header in `firmware/include/boards/`, an env in
`firmware/platformio.ini` and CI, and a row in `MODELS` (its board flag, IDs
and model name), then `just gen-header`. A custom build doesn't need the row
to be found, only a product string starting `Detent `; `ctl-probe` finds it
the same way, and `DETENT_PROBE_IDS=VID:PID ctl-probe` probes one board by its
IDs.
