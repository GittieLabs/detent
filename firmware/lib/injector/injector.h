// TEST_INJECT replay. Debug firmware only: release firmware constructs the
// injector disabled, so TEST_INJECT does nothing there (docs/protocol.md).
//
// Each event fires delay_ms after the previous one (the first, after the
// script arrived). A new script replaces any script still running. Pure
// C++11; tested natively.

#ifndef DETENT_INJECTOR_H
#define DETENT_INJECTOR_H

#include <stdint.h>

#include "report.h"

namespace detent {

class Injector {
 public:
  explicit Injector(bool enabled) : enabled_(enabled) {}

  // Loads a parsed TEST_INJECT. Returns false, doing nothing, when disabled
  // or when `msg` isn't TEST_INJECT.
  bool load(const AppMessage& msg, uint32_t now_ms);

  // Fills `out` with the next event if it's due. Call until it returns false.
  bool next_due(uint32_t now_ms, InjectEvent* out);

  bool active() const { return next_ < count_; }
  bool enabled() const { return enabled_; }

 private:
  bool enabled_;
  InjectEvent events_[DETENT_MAX_INJECT_EVENTS];
  uint8_t count_ = 0;
  uint8_t next_ = 0;
  uint32_t base_ms_ = 0;  // when the previous event fired, or the load
};

}  // namespace detent

#endif  // DETENT_INJECTOR_H
