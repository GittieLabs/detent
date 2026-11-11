// Adafruit ESP32-S3 Feather, 4 MB flash and 2 MB PSRAM (product 5477): the
// same chip as the XIAO with more GPIO, so the same firmware with this pin
// map (M7, D-097).
//
// PLACEHOLDER: Keith's Feather has no headers yet. Until he solders them and
// wires it, it uses only what's on the board: the BOOT button as button 1, the
// red LED, and the STEMMA QT connector for an OLED. No encoder (D-096). Add
// the buttons and encoder here once it's wired.

#ifndef DETENT_BOARD_FEATHER_ESP32S3_H
#define DETENT_BOARD_FEATHER_ESP32S3_H

namespace detent {
namespace board {

// BOOT (GPIO0) has its own pull up and reads LOW when pressed. Holding it
// while the board resets enters the ROM bootloader instead; pressed any other
// time it's an ordinary button.
constexpr ButtonPin kButtons[] = {
    {1, 0},  // BOOT
};
constexpr uint8_t kButtonCount = sizeof kButtons / sizeof kButtons[0];

// None until the headers are on.
constexpr EncoderPins kEncoders[] = {{0, 0, 0, kNoPush}};  // unused
constexpr uint8_t kEncoderCount = 0;

// 128x64 OLED on the STEMMA QT connector (I2C on GPIO3 and GPIO4). The
// connector's power is switched by GPIO7, which the firmware drives high
// before starting the display.
const uint8_t kOledSda = 3;
const uint8_t kOledScl = 4;
const uint8_t kOledAddress = 0x3C;
constexpr int kI2cPowerPin = 7;

// The red LED by the USB port (GPIO13), active high. The NeoPixel is left
// off: it needs its own power pin and a slower write.
const LedKind kLedKind = LedKind::ActiveHigh;
const uint8_t kLedPin = 13;

// Pins nothing uses, for the debug build's pin finder (DETENT_INPUT_LOG).
// A0 and A1 (GPIO18 and GPIO17).
constexpr SparePin kSpare[] = {{"A0", 18}, {"A1", 17}};

}  // namespace board
}  // namespace detent

#endif  // DETENT_BOARD_FEATHER_ESP32S3_H
