// Physical controls: debounced buttons with long press, and a quadrature
// encoder with detent coalescing. Pure C++11 with caller supplied time and pin
// levels, so it is tested natively (D-035). Timing matches docs/protocol.md.

#ifndef DETENT_INPUT_H
#define DETENT_INPUT_H

#include <stdint.h>

namespace detent {

// A level must hold this long to count. Mechanical switches bounce for a few ms.
const uint32_t kDebounceMs = 10;
// LongPress fires once at this much held time, before Up (protocol: 600 ms).
const uint32_t kLongPressMs = 600;
// Detents within this window of the first pending one go out as one delta.
const uint32_t kCoalesceMs = 20;
// Quadrature transitions per detent (EC11 style encoders).
const int kStepsPerDetent = 4;

enum class ButtonEvent : uint8_t { None, Down, Up, LongPress };

class Button {
 public:
  // `pressed` is the raw contact level (already inverted for active low).
  // Returns at most one event per call; call often.
  ButtonEvent update(bool pressed, uint32_t now_ms);

  bool pressed() const { return stable_; }

 private:
  bool raw_ = false;
  uint32_t raw_since_ms_ = 0;
  bool stable_ = false;
  uint32_t down_ms_ = 0;
  bool long_fired_ = false;
};

class Encoder {
 public:
  // Sets the resting levels without counting a step. Call at startup.
  void reset(bool a, bool b);

  // Feed the A and B levels on every change (from a pin change interrupt).
  // Transitions that skip a state (both contacts changed) are ignored.
  void on_edge(bool a, bool b);

  // Returns true and fills `delta` (positive is clockwise, never zero) when
  // detents are pending and the coalescing window has elapsed. Deltas beyond
  // int8_t go out over several calls.
  bool poll(uint32_t now_ms, int8_t* delta);

 private:
  uint8_t state_ = 3;    // (a << 1) | b
  int32_t quarters_ = 0;  // quadrature steps not yet sent
  bool window_open_ = false;
  uint32_t window_start_ms_ = 0;
};

}  // namespace detent

#endif  // DETENT_INPUT_H
