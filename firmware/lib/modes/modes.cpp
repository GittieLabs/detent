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

void ModeRouter::on_turn(int8_t delta) {
  switch (mode_) {
    case DETENT_ENCODER_MODE_VOLUME:
      sink_.volume_steps(delta);
      break;
    case DETENT_ENCODER_MODE_MAPPED:
      sink_.encoder(encoder_, delta);
      break;
    case DETENT_ENCODER_MODE_MENU:
      sink_.menu_scroll(delta);
      break;
    default:
      break;
  }
}

void ModeRouter::on_push(ButtonEvent e) {
  switch (e) {
    case ButtonEvent::Down:
      pressing_ = true;
      long_ = false;
      press_mode_ = mode_;
      if (press_mode_ == DETENT_ENCODER_MODE_MAPPED) sink_.button(push_, DETENT_ACTION_DOWN);
      break;
    case ButtonEvent::LongPress:
      if (!pressing_) break;
      long_ = true;
      set_mode(press_mode_ == DETENT_ENCODER_MODE_MENU ? before_menu_ : DETENT_ENCODER_MODE_MENU);
      break;
    case ButtonEvent::Up:
      if (!pressing_) break;
      pressing_ = false;
      if (press_mode_ == DETENT_ENCODER_MODE_MAPPED) {
        // Always pair the Down the app saw, even after a long press.
        sink_.button(push_, DETENT_ACTION_UP);
      } else if (!long_ && press_mode_ == DETENT_ENCODER_MODE_VOLUME) {
        sink_.mute_toggle();
      } else if (!long_ && press_mode_ == DETENT_ENCODER_MODE_MENU) {
        sink_.menu_select();
      }
      break;
    default:
      break;
  }
}

}  // namespace detent
