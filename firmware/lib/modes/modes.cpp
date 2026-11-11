#include "modes.h"

namespace detent {

bool ModeRouter::set_mode(uint8_t mode) {
  if (mode != DETENT_ENCODER_MODE_VOLUME && mode != DETENT_ENCODER_MODE_MENU &&
      mode != DETENT_ENCODER_MODE_MAPPED) {
    return false;
  }
  if (mode == mode_) return true;
  if (mode == DETENT_ENCODER_MODE_MENU) before_menu_ = mode_;
  mode_ = mode;
  sink_.mode_changed(mode_);
  return true;
}

void ModeRouter::on_turn(uint8_t control, int8_t delta) {
  switch (mode_) {
    case DETENT_ENCODER_MODE_VOLUME:
      sink_.volume_steps(delta);
      break;
    case DETENT_ENCODER_MODE_MAPPED:
      sink_.encoder(control, delta);
      break;
    case DETENT_ENCODER_MODE_MENU:
      sink_.menu_scroll(delta);
      break;
    default:
      break;
  }
}

void ModeRouter::on_push(uint8_t control, ButtonEvent e) {
  Press& p = presses_[control];
  switch (e) {
    case ButtonEvent::Down:
      p.pressing = true;
      p.long_press = false;
      p.mode = mode_;
      if (p.mode == DETENT_ENCODER_MODE_MAPPED) sink_.button(control, DETENT_ACTION_DOWN);
      break;
    case ButtonEvent::LongPress:
      if (!p.pressing) break;
      p.long_press = true;
      set_mode(p.mode == DETENT_ENCODER_MODE_MENU ? before_menu_ : DETENT_ENCODER_MODE_MENU);
      break;
    case ButtonEvent::Up:
      if (!p.pressing) break;
      p.pressing = false;
      if (p.mode == DETENT_ENCODER_MODE_MAPPED) {
        // Always pair the Down the app saw, even after a long press.
        sink_.button(control, DETENT_ACTION_UP);
      } else if (!p.long_press && p.mode == DETENT_ENCODER_MODE_VOLUME) {
        sink_.mute_toggle();
      } else if (!p.long_press && p.mode == DETENT_ENCODER_MODE_MENU) {
        sink_.menu_select();
      }
      break;
    default:
      break;
  }
}

}  // namespace detent
