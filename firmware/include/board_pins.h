// The board this firmware is built for: its pins, status LED and OLED, from
// one header per board in include/boards/, chosen by the env's
// -DDETENT_BOARD_* flag (M7). Change a board's pins there, and only there.
// HELLO reports exactly its controls.
//
// Each board header defines:
//   kButtons, kButtonCount       every button, push switches included
//   kEncoders, kEncoderCount     every encoder: control ID, A and B pins, and
//                                its push switch's button ID or kNoPush
// Any number of encoders works (D-096). For none, C++ still needs one entry:
//   constexpr EncoderPins kEncoders[] = {{0, 0, 0, kNoPush}};  // unused
//   constexpr uint8_t kEncoderCount = 0;
// A layout HELLO can't report (a push switch that isn't a listed button, or
// shared, or a reused control ID) is a compile error below.

#ifndef DETENT_BOARD_PINS_H
#define DETENT_BOARD_PINS_H

#include <stdint.h>

#include "layout.h"

namespace detent {
namespace board {

// How the board's status LED is driven.
enum class LedKind : uint8_t { ActiveLow, ActiveHigh, Rgb };

// A pin the debug build's pin finder may watch (DETENT_INPUT_LOG).
struct SparePin {
  const char* label;
  uint8_t gpio;
};

}  // namespace board
}  // namespace detent

#if defined(DETENT_BOARD_XIAO_ESP32S3)
#include "boards/xiao_esp32s3.h"
#elif defined(DETENT_BOARD_DEVKITC_S3)
#include "boards/devkitc_s3.h"
#elif defined(DETENT_BOARD_PICO)
#include "boards/pico.h"
#else
#error "No board: build with -DDETENT_BOARD_XIAO_ESP32S3, _DEVKITC_S3 or _PICO"
#endif

namespace detent {
namespace board {
namespace check {

// C++11 constexpr: one return each, so loops are recursion.
constexpr bool is_button(uint8_t id, uint8_t i = 0) {
  return i < kButtonCount && (kButtons[i].control == id || is_button(id, i + 1));
}
constexpr int button_uses(uint8_t id, uint8_t i = 0) {
  return i == kButtonCount ? 0 : (kButtons[i].control == id ? 1 : 0) + button_uses(id, i + 1);
}
constexpr int encoder_uses(uint8_t id, uint8_t i = 0) {
  return i == kEncoderCount ? 0 : (kEncoders[i].control == id ? 1 : 0) + encoder_uses(id, i + 1);
}
constexpr int push_uses(uint8_t id, uint8_t i = 0) {
  return i == kEncoderCount ? 0 : (kEncoders[i].push == id ? 1 : 0) + push_uses(id, i + 1);
}
constexpr bool buttons_ok(uint8_t i = 0) {
  return i == kButtonCount || (kButtons[i].control != kNoPush &&
                               button_uses(kButtons[i].control) + encoder_uses(kButtons[i].control) == 1 &&
                               buttons_ok(i + 1));
}
constexpr bool encoders_ok(uint8_t i = 0) {
  return i == kEncoderCount ||
         (kEncoders[i].control != kNoPush && !is_button(kEncoders[i].control) &&
          encoder_uses(kEncoders[i].control) == 1 &&
          (kEncoders[i].push == kNoPush ||
           (is_button(kEncoders[i].push) && push_uses(kEncoders[i].push) == 1)) &&
          encoders_ok(i + 1));
}
constexpr int pushes(uint8_t i = 0) {
  return i == kEncoderCount ? 0 : (kEncoders[i].push != kNoPush ? 1 : 0) + pushes(i + 1);
}

static_assert(buttons_ok() && encoders_ok(),
              "board layout: control IDs must be unique, and each push switch one listed button "
              "of one encoder");
static_assert(kButtonCount + kEncoderCount <= DETENT_MAX_CONTROLS, "board layout: too many controls");
static_assert(5 + 2 * (kButtonCount + kEncoderCount) + pushes() <= DETENT_MAX_PAYLOAD_LEN,
              "board layout: HELLO wouldn't fit one report");

}  // namespace check
}  // namespace board
}  // namespace detent

#endif  // DETENT_BOARD_PINS_H
