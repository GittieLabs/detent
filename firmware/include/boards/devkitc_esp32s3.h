// ESP32-S3 DevKitC-1 class board with an N16R8 module (16 MB quad flash,
// 8 MB octal PSRAM) and two USB-C ports (M7, D-116, D-118). Keith's is a
// clone; the same pins fit Espressif's own DevKitC-1.
//
// Two USB-C ports: Detent runs on the one labelled USB (the chip's native
// USB, GPIO19 and GPIO20). The one labelled UART or COM goes through a serial
// bridge chip; it can flash the board but never carries Detent's HID device.
//
// Pins kept free: GPIO19/20 (native USB), GPIO26 to 32 (the module's flash),
// GPIO33 to 37 (the octal PSRAM on an R8 module), the strapping pins 0, 3, 45
// and 46, and GPIO43/44 (UART0, wired to the bridge chip; panic output).
//
// PLACEHOLDER until Keith wires it: the map below is Claude's pick, and Keith
// wires to it (open questions resolved 2026-11-12). Control IDs match the
// XIAO's, so its starter mappings fit.

#ifndef DETENT_BOARD_DEVKITC_ESP32S3_H
#define DETENT_BOARD_DEVKITC_ESP32S3_H

namespace detent {
namespace board {

// Buttons switch to ground; the firmware enables internal pull ups.
constexpr ButtonPin kButtons[] = {
    {1, 4},
    {2, 5},
    {3, 6},
    {4, 7},
    {6, 17},  // the encoder's push switch
};
constexpr uint8_t kButtonCount = sizeof kButtons / sizeof kButtons[0];

// Encoder 5 on GPIO15 (A) and GPIO16 (B), its push switch button 6.
constexpr EncoderPins kEncoders[] = {{5, 15, 16, 6}};
constexpr uint8_t kEncoderCount = sizeof kEncoders / sizeof kEncoders[0];

// 128x64 OLED on I2C, the Arduino-ESP32 defaults for this board (SDA GPIO8,
// SCL GPIO9).
const uint8_t kOledSda = 8;
const uint8_t kOledScl = 9;
const uint8_t kOledAddress = 0x3C;
constexpr int kI2cPowerPin = -1;  // always powered

// The RGB LED (WS2812 class) on GPIO48. Espressif's v1.1 boards moved it to
// GPIO38; some clones need the solder jumper marked RGB bridged. If it stays
// dark, try 38 here.
const LedKind kLedKind = LedKind::Rgb;
const uint8_t kLedPin = 48;

// Pins nothing uses, for the debug build's pin finder (DETENT_INPUT_LOG).
constexpr SparePin kSpare[] = {{"GPIO10", 10}, {"GPIO11", 11}};

}  // namespace board
}  // namespace detent

#endif  // DETENT_BOARD_DEVKITC_ESP32S3_H
