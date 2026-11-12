# Build your own device

You can wire your own Detent device: any number of buttons, as many knobs (rotary encoders) as fit, an OLED and a status LED. You describe the wiring in one firmware file. When the device connects, it tells the app what controls it has, so you never change the app.

This page walks through a complete example, **Detent Macro**: an ESP32-S3 DevKitC with eight keys, two knobs with push switches, an OLED and the board's RGB LED.

## What you need

- **A board with an ESP32-S3 or an RP2040** (Raspberry Pi Pico). Both have native USB, which Detent needs. Boards built on the original ESP32 (no "S3") can't be Detent devices.
- **Buttons** that connect a pin to ground when pressed. The firmware turns on the pin's internal pull up, so no resistors are needed.
- **Rotary encoders** (optional): two contact pins (A and B) to ground, and often a push switch, which is just another button.
- **A 128×64 OLED** (optional): SSD1306 class, on I²C.
- The repository, [PlatformIO](https://platformio.org), and Rust (for `just probe`).

## 1. Pick your pins

Use any free GPIO except the ones your board needs for itself. On an ESP32-S3 DevKitC with an N16R8 module, keep these free:

- GPIO19 and 20: the native USB that Detent runs on.
- GPIO26 to 37: the module's flash and octal PSRAM.
- GPIO0, 3, 45 and 46: strapping pins, read at boot.
- GPIO43 and 44: the serial port to the UART chip.

Other boards differ. Read the existing header for your board in `firmware/include/boards/` first; its comments list what to avoid.

## 2. Describe the board

Create `firmware/include/boards/macro_pad.h`:

```cpp
#ifndef DETENT_BOARD_MACRO_PAD_H
#define DETENT_BOARD_MACRO_PAD_H

namespace detent {
namespace board {

// {control ID, GPIO}. Keys switch to ground; a press reads LOW.
constexpr ButtonPin kButtons[] = {
    {1, 4},  {2, 5},  {3, 6},  {4, 7},   // top row
    {5, 15}, {6, 16}, {7, 17}, {8, 18},  // bottom row
    {21, 1},                             // left knob's push switch
    {22, 2},                             // right knob's push switch
};
constexpr uint8_t kButtonCount = sizeof kButtons / sizeof kButtons[0];

// {control ID, pin A, pin B, control ID of its push switch or kNoPush}.
constexpr EncoderPins kEncoders[] = {
    {20, 10, 11, 21},  // left knob
    {23, 12, 13, 22},  // right knob
};
constexpr uint8_t kEncoderCount = sizeof kEncoders / sizeof kEncoders[0];

// 128x64 OLED on I2C.
const uint8_t kOledSda = 8;
const uint8_t kOledScl = 9;
const uint8_t kOledAddress = 0x3C;
constexpr int kI2cPowerPin = -1;  // or a pin that switches the display's power

// The status LED: Rgb, ActiveHigh or ActiveLow.
const LedKind kLedKind = LedKind::Rgb;
const uint8_t kLedPin = 48;

// Free pins the debug build's pin finder may watch. Never a pin in use.
constexpr SparePin kSpare[] = {{"GPIO14", 14}, {"GPIO21", 21}};

}  // namespace board
}  // namespace detent

#endif  // DETENT_BOARD_MACRO_PAD_H
```

**Control IDs** are how the app and your mappings name each control. Any number from 0 to 254 works (255 means "no push switch"). Each ID is used once, and an encoder's push switch must be one of the listed buttons.

**A board with no encoder** still needs one placeholder entry, because C++ won't accept an empty list:

```cpp
constexpr EncoderPins kEncoders[] = {{0, 0, 0, kNoPush}};  // unused
constexpr uint8_t kEncoderCount = 0;
```

## 3. Hook it into the build

In `firmware/include/board_pins.h`, add a branch for your board's flag, before the `#else`:

```cpp
#elif defined(DETENT_BOARD_MACRO_PAD)
#include "boards/macro_pad.h"
```

In `firmware/platformio.ini`, add an env. Start from the existing env for the same board and swap the board flag:

```ini
[env:macro_pad]
extends = env:devkitc_esp32s3
build_flags =
    ${env:devkitc_esp32s3.build_flags}
    -UDETENT_BOARD_DEVKITC_ESP32S3
    -DDETENT_BOARD_MACRO_PAD
    -DDETENT_USB_VID=0x303A
    -DDETENT_USB_PID=0x8FE1
    '-DDETENT_MODEL_NAME="Macro"'
```

The last three lines give the device its USB identity:

- **The model name** makes the USB product string, here *Detent Macro*. The app recognizes a device by that string starting with `Detent `, so keep it.
- **The VID** is your chip maker's: `0x303A` for the ESP32-S3, `0x2E8A` for the RP2040.
- **The PID** is any number not already used in `docs/boards.md`. It only names your board in system tools.

The app doesn't care about the VID and PID. It matches each device by its serial number, so changing them later keeps your pairing and mappings.

## 4. Build

```
pio run -d firmware -e macro_pad
```

The build checks your layout. A repeated control ID, or a push switch that isn't one of the buttons, stops it with:

```
error: static assertion failed: board layout: control IDs must be unique, and each push switch one listed button of one encoder
```

Too many controls stops it too (see [Limits](#limits)).

## 5. Flash and check

Plug the board in by its native USB port (on a DevKitC, the one labelled **USB**) and flash it:

```
just fw-upload macro_pad
```

If the upload can't find the board, hold **BOOT**, press **RESET**, release **BOOT**, and run it again.

Then check what the device reports:

```
just probe
```

It prints a HELLO listing every control with its ID and kind (button, encoder, or encoder with its push switch), then a heartbeat every 2 s. Press each key and turn each knob; each one should print with the ID you gave it. Knobs only send turns to the app in **Mapped** mode (switch modes in the [device menu](device-menu.md)); in Volume mode they change the Mac's volume directly.

Finally, open Detent and [pair the device](pairing.md). Mappings shows your controls and the knobs with their Push cells.

## Limits

- **27 controls at most**, buttons and encoders together.
- **HELLO must fit in 59 bytes**: 5, plus 2 for each control, plus 1 for each encoder with a push switch. Detent Macro has 12 controls (10 buttons, 2 encoders) and 2 push switches: 5 + 24 + 2 = 31 bytes.

  For example, 24 buttons and one encoder whose push switch is one of the 24 come to 25 controls and 56 bytes, which fits. The build refuses a layout that doesn't.

## Keep control IDs stable

Your mappings remember controls by ID. If you rewire or reflash, keep each control's ID: renumber one and its mappings point at a control the device no longer has. Moving a control to another pin is fine, as long as its ID stays the same.

## Share your board

To get a board into Detent for everyone, open a pull request with:

- The header in `firmware/include/boards/` and its branch in `board_pins.h`.
- The env in `platformio.ini`, plus a `_debug` env like the others have.
- Its env in the firmware build lists in `justfile` (`fw` and `ci`) and `.github/workflows/ci.yml`.
- A row in `MODELS` in `protocol/src/ids.rs`, with its board flag, VID, PID and model name, instead of the three `-D` lines. Then run `just gen-header` and `cargo test --workspace`.
- A row in `docs/boards.md`.
