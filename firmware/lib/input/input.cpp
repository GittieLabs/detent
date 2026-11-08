#include "input.h"

namespace detent {

ButtonEvent Button::update(bool pressed, uint32_t now_ms) {
  if (pressed != raw_) {
    raw_ = pressed;
    raw_since_ms_ = now_ms;
  }
  // Unsigned subtraction handles millis() wrapping.
  if (raw_ != stable_ && now_ms - raw_since_ms_ >= kDebounceMs) {
    stable_ = raw_;
    if (stable_) {
      down_ms_ = now_ms;
      long_fired_ = false;
      return ButtonEvent::Down;
    }
    return ButtonEvent::Up;
  }
  if (stable_ && !long_fired_ && now_ms - down_ms_ >= kLongPressMs) {
    long_fired_ = true;
    return ButtonEvent::LongPress;
  }
  return ButtonEvent::None;
}

namespace {

// Indexed by (previous state << 2) | new state, state = (a << 1) | b.
// +1 is one quarter step clockwise (11 -> 01 -> 00 -> 10 -> 11), -1 the
// reverse, 0 for no change or an impossible jump (both contacts changed).
const int8_t kQuadrature[16] = {
    //        to: 00  01  10  11
    /* 00 */ 0, -1, +1, 0,
    /* 01 */ +1, 0, 0, -1,
    /* 10 */ -1, 0, 0, +1,
    /* 11 */ 0, +1, -1, 0,
};

}  // namespace

void Encoder::reset(bool a, bool b) {
  state_ = static_cast<uint8_t>((a ? 2 : 0) | (b ? 1 : 0));
  quarters_ = 0;
  window_open_ = false;
}

void Encoder::on_edge(bool a, bool b) {
  const uint8_t next = static_cast<uint8_t>((a ? 2 : 0) | (b ? 1 : 0));
  quarters_ += kQuadrature[(state_ << 2) | next];
  state_ = next;
}

bool Encoder::poll(uint32_t now_ms, int8_t* delta) {
  const int32_t detents = quarters_ / kStepsPerDetent;  // toward zero
  if (detents == 0) {
    window_open_ = false;
    return false;
  }
  if (!window_open_) {
    window_open_ = true;
    window_start_ms_ = now_ms;
  }
  if (now_ms - window_start_ms_ < kCoalesceMs) return false;

  const int32_t out = detents > 127 ? 127 : (detents < -127 ? -127 : detents);
  quarters_ -= out * kStepsPerDetent;
  *delta = static_cast<int8_t>(out);
  // Anything left (clamped) starts a fresh window now.
  window_start_ms_ = now_ms;
  window_open_ = quarters_ / kStepsPerDetent != 0;
  return true;
}

}  // namespace detent
