# Boards and firmware

Detent's firmware runs on these boards. Each reports its own buttons and encoders when it connects, so the app adapts to whatever you wire up.

| Board | Firmware env | Notes |
| --- | --- | --- |
| Seeed XIAO ESP32-S3 | `xiao_esp32s3` | The reference board: buttons, one encoder with a push switch, an OLED. |
| Adafruit ESP32-S3 Feather | `feather_esp32s3` | Works with nothing soldered: the BOOT button is button 1, the red LED shows status, and an OLED plugs into the STEMMA QT connector. |
| Raspberry Pi Pico (RP2040) | `pico` | In progress: its USB link works; buttons, encoder and OLED are being tested. |

Boards built on the original ESP32 (no "S3") can't be Detent devices: they have no native USB.

## What a device has

- **Buttons**, any number. Each can fire on a press or a long press.
- **Encoders**, none, one or several, each with an optional push switch. In the device's **Volume** mode a knob changes the Mac's volume and its push mutes, with the app open or closed. In **Mapped** mode turns and pushes go to the app like any other control. **Menu** mode scrolls the OLED menu.
- **An OLED** (128×64, SSD1306 class, I²C), optional: the device's status line and its [menu](device-menu.md).
- **A status LED**: off with no Mac, blinking while it isn't paired, solid once it is.

## Flashing

With [PlatformIO](https://platformio.org) installed and the board on USB, from the repository:

```
just fw-upload xiao_esp32s3
```

Use the env from the table for other boards. `just fw` builds every board without flashing.

## Your own layout

A board's pins live in one header in `firmware/include/boards/`: its buttons, its encoders and which button is each encoder's push switch. A layout the device couldn't describe to the app fails to compile, so mistakes show up before you flash.
