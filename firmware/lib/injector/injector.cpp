#include "injector.h"

namespace detent {

bool Injector::load(const AppMessage& msg, uint32_t now_ms) {
  if (!enabled_ || msg.type != DETENT_MSG_TEST_INJECT) return false;
  count_ = msg.inject_count <= DETENT_MAX_INJECT_EVENTS ? msg.inject_count : DETENT_MAX_INJECT_EVENTS;
  for (uint8_t i = 0; i < count_; i++) events_[i] = msg.inject[i];
  next_ = 0;
  base_ms_ = now_ms;
  return true;
}

bool Injector::next_due(uint32_t now_ms, InjectEvent* out) {
  if (!active()) return false;
  const InjectEvent& ev = events_[next_];
  // Unsigned subtraction handles millis() wrapping.
  if (now_ms - base_ms_ < ev.delay_ms) return false;
  // Schedule from when this event was due, not when the tick came, so a late
  // tick doesn't stretch the rest of the script.
  base_ms_ += ev.delay_ms;
  *out = ev;
  next_++;
  return true;
}

}  // namespace detent
