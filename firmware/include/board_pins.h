// The board this firmware is built for: its pins, status LED and OLED, from
// one header per board in include/boards/, chosen by the env's
// -DDETENT_BOARD_* flag (M7). Change a board's pins there, and only there.
// HELLO reports exactly its controls.

#ifndef DETENT_BOARD_PINS_H
#define DETENT_BOARD_PINS_H

#include <stdint.h>

namespace detent {
namespace board {

struct ButtonPin {
  uint8_t control;  // protocol control_id
  uint8_t gpio;
};

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

#endif  // DETENT_BOARD_PINS_H
