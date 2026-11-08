// Pin map and control table for the first device (XIAO ESP32-S3).
//
// PLACEHOLDER: Keith's wiring isn't known yet (open question in
// docs/roadmap.md). Change this table, and only this table, when it is. HELLO
// reports exactly these controls.
//
// Buttons switch to ground; the firmware enables internal pull ups, so a press
// reads LOW and an unwired pin reads as released. D6/D7 (GPIO43/44) are left
// free for the UART, which carries panic output when debugging.

#ifndef DETENT_BOARD_PINS_H
#define DETENT_BOARD_PINS_H

#include <stdint.h>

namespace detent {
namespace board {

struct ButtonPin {
  uint8_t control;  // protocol control_id
  uint8_t gpio;
};

// XIAO pin labels: D0=GPIO1 D1=GPIO2 D2=GPIO3 D3=GPIO4 D4=GPIO5 D5=GPIO6
// D6=GPIO43 D7=GPIO44 D8=GPIO7 D9=GPIO8 D10=GPIO9.
const uint8_t kEncoderPushControl = 6;  // routed by the encoder mode (lib/modes)

const ButtonPin kButtons[] = {
    {1, 1},                    // D0
    {2, 2},                    // D1
    {3, 3},                    // D2
    {4, 4},                    // D3
    {kEncoderPushControl, 9},  // D10, the encoder's push switch
};
const uint8_t kButtonCount = sizeof kButtons / sizeof kButtons[0];

const uint8_t kEncoderControl = 5;
const uint8_t kEncoderA = 7;  // D8
const uint8_t kEncoderB = 8;  // D9

// SSD1306 OLED on I2C (used by the display task).
const uint8_t kOledSda = 5;  // D4
const uint8_t kOledScl = 6;  // D5

}  // namespace board
}  // namespace detent

#endif  // DETENT_BOARD_PINS_H
