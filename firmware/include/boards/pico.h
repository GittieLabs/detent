// Raspberry Pi Pico (RP2040), M7. Its USB glue is src/pico (D-092).
//
// PLACEHOLDER: Keith's Pico has no headers yet (its board task waits on
// that). I2C0 on its default pins; control IDs match the XIAO's.

#ifndef DETENT_BOARD_PICO_H
#define DETENT_BOARD_PICO_H

namespace detent {
namespace board {

constexpr ButtonPin kButtons[] = {
    {1, 10},
    {2, 11},
    {3, 12},
    {4, 13},
    {6, 14},  // the encoder's push switch
};
constexpr uint8_t kButtonCount = sizeof kButtons / sizeof kButtons[0];

constexpr EncoderPins kEncoders[] = {{5, 15, 16, 6}};
constexpr uint8_t kEncoderCount = sizeof kEncoders / sizeof kEncoders[0];

const uint8_t kOledSda = 4;  // GP4, I2C0 SDA
const uint8_t kOledScl = 5;  // GP5, I2C0 SCL
const uint8_t kOledAddress = 0x3C;
constexpr int kI2cPowerPin = -1;  // always powered

// The on board LED, GP25, active high.
const LedKind kLedKind = LedKind::ActiveHigh;
const uint8_t kLedPin = 25;

constexpr SparePin kSpare[] = {{"GP20", 20}, {"GP21", 21}};

}  // namespace board
}  // namespace detent

#endif  // DETENT_BOARD_PICO_H
