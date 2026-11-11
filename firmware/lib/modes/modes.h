// Encoder modes: what turns and the encoders' push switches do. Owned by the
// firmware; reported in HELLO and HEARTBEAT (docs/protocol.md). Pure C++11,
// tested natively (D-035). There is one mode, and every encoder and push
// switch follows it (D-096); a board may have none, one, or several.
//
//   Volume  turns step the volume (Consumer Control); a short press toggles
//           mute. Works with no app running. The boot mode.
//   Mapped  turns go to the app as ENCODER; the push switch as BUTTON.
//   Menu    turns scroll the OLED menu; a short press selects.
//
// A long press of any push switch opens the menu from any mode and closes it
// again, back to the mode it came from (D-048). Each press acts in the mode it
// started in, so the release that ends a long press never mutes or selects.

#ifndef DETENT_MODES_H
#define DETENT_MODES_H

#include <stdint.h>

#include "detent_protocol.h"
#include "input.h"

namespace detent {

struct ModeSink {
  virtual ~ModeSink() {}
  virtual void volume_steps(int8_t delta) = 0;  // Consumer Control
  virtual void mute_toggle() = 0;               // Consumer Control
  virtual void encoder(uint8_t control, int8_t delta) = 0;  // ENCODER to the app
  virtual void button(uint8_t control, uint8_t action) = 0;  // BUTTON to the app
  virtual void menu_scroll(int8_t delta) = 0;
  virtual void menu_select() = 0;
  virtual void mode_changed(uint8_t mode) = 0;  // DETENT_ENCODER_MODE_*
};

class ModeRouter {
 public:
  explicit ModeRouter(ModeSink& sink) : sink_(sink) {}

  // A turn of encoder `control`.
  void on_turn(uint8_t control, int8_t delta);
  // An event of push switch `control` (a button that is an encoder's push).
  void on_push(uint8_t control, ButtonEvent e);

  // Returns false, changing nothing, for an undefined mode.
  bool set_mode(uint8_t mode);
  // Leaves the menu for the mode it was opened from (an app item was
  // picked); does nothing outside the menu.
  void leave_menu() {
    if (mode_ == DETENT_ENCODER_MODE_MENU) set_mode(before_menu_);
  }
  uint8_t mode() const { return mode_; }

 private:
  // One press in progress per push switch, by control ID, so two held at
  // once each pair their own Down and Up.
  struct Press {
    bool pressing = false;
    bool long_press = false;
    uint8_t mode = DETENT_ENCODER_MODE_VOLUME;
  };

  ModeSink& sink_;
  uint8_t mode_ = DETENT_ENCODER_MODE_VOLUME;
  uint8_t before_menu_ = DETENT_ENCODER_MODE_VOLUME;
  Press presses_[256];
};

}  // namespace detent

#endif  // DETENT_MODES_H
