#include "layout.h"

namespace detent {

int push_owner(const EncoderPins* encoders, uint8_t count, uint8_t control) {
  if (control == kNoPush) return -1;
  for (uint8_t i = 0; i < count; i++) {
    if (encoders[i].push == control) return i;
  }
  return -1;
}

int encoder_index(const EncoderPins* encoders, uint8_t count, uint8_t control) {
  for (uint8_t i = 0; i < count; i++) {
    if (encoders[i].control == control) return i;
  }
  return -1;
}

bool layout_ok(const ButtonPin* buttons, uint8_t button_count, const EncoderPins* encoders,
               uint8_t encoder_count) {
  bool seen[256] = {false};
  bool is_button[256] = {false};
  for (uint8_t i = 0; i < button_count; i++) {
    const uint8_t id = buttons[i].control;
    if (id == kNoPush || seen[id]) return false;
    seen[id] = is_button[id] = true;
  }
  bool claimed[256] = {false};
  for (uint8_t i = 0; i < encoder_count; i++) {
    const uint8_t id = encoders[i].control;
    if (id == kNoPush || seen[id]) return false;
    seen[id] = true;
    const uint8_t push = encoders[i].push;
    if (push == kNoPush) continue;
    if (!is_button[push] || claimed[push]) return false;
    claimed[push] = true;
  }
  return true;
}

uint8_t hello_controls(const ButtonPin* buttons, uint8_t button_count,
                       const EncoderPins* encoders, uint8_t encoder_count, Control* out) {
  uint8_t n = 0;
  for (uint8_t i = 0; i < button_count; i++) {
    out[n++] = {buttons[i].control, DETENT_KIND_BUTTON, 0};
  }
  for (uint8_t i = 0; i < encoder_count; i++) {
    const EncoderPins& e = encoders[i];
    if (e.push == kNoPush) {
      out[n++] = {e.control, DETENT_KIND_ENCODER, 0};
    } else {
      out[n++] = {e.control, DETENT_KIND_ENCODER_WITH_PUSH, e.push};
    }
  }
  return n;
}

}  // namespace detent
