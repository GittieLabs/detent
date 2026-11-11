// ESP32-S3 DevKitC class board (Keith's is a clone): the same chip as the
// XIAO with far more GPIO, so the same firmware with this pin map (M7).
//
// PLACEHOLDER: Keith hasn't wired one yet (open question in docs/roadmap.md).
// The pins below avoid the strapping pins (0, 3, 45, 46), the native USB pair
// (19, 20, which Detent needs: use the board's port labelled USB, not UART),
// and the flash and PSRAM pins (26 to 37). Control IDs match the XIAO's.

#ifndef DETENT_BOARD_DEVKITC_S3_H
#define DETENT_BOARD_DEVKITC_S3_H

namespace detent {
namespace board {

constexpr ButtonPin kButtons[] = {
    {1, 4},
    {2, 5},
    {3, 6},
    {4, 7},
    {6, 15},  // the encoder's push switch
};
constexpr uint8_t kButtonCount = sizeof kButtons / sizeof kButtons[0];

constexpr EncoderPins kEncoders[] = {{5, 16, 17, 6}};
constexpr uint8_t kEncoderCount = sizeof kEncoders / sizeof kEncoders[0];

// The S3's default I2C pins.
const uint8_t kOledSda = 8;
const uint8_t kOledScl = 9;
const uint8_t kOledAddress = 0x3C;

// The addressable RGB LED (GPIO48 on DevKitC-1 v1.0, GPIO38 on v1.1; the
// variant's RGB_BUILTIN says which). Shown green.
const LedKind kLedKind = LedKind::Rgb;
#ifdef RGB_BUILTIN
const uint8_t kLedPin = RGB_BUILTIN;
#else
const uint8_t kLedPin = 48;
#endif

constexpr SparePin kSpare[] = {{"G10", 10}, {"G11", 11}};

}  // namespace board
}  // namespace detent

#endif  // DETENT_BOARD_DEVKITC_S3_H
