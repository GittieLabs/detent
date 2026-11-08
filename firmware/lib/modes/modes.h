// Encoder modes: what turns and the encoder's push switch do. Owned by the
// firmware; reported in HELLO and HEARTBEAT (docs/protocol.md). Pure C++11,
// tested natively (D-035).
//
//   Volume  turns step the volume (Consumer Control); a short press toggles
//           mute. Works with no app running. The boot mode.
//   Mapped  turns go to the app as ENCODER; the push switch as BUTTON.
//   Menu    turns scroll the OLED menu; a short press selects.
//
// A long press of the push switch opens the menu from any mode and closes it
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
  ModeRouter(uint8_t encoder_control, uint8_t push_control, ModeSink& sink)
      : encoder_(encoder_control), push_(push_control), sink_(sink) {}

  void on_turn(int8_t delta);
  void on_push(ButtonEvent e);

  // Returns false, changing nothing, for an undefined mode.
  bool set_mode(uint8_t mode);
  uint8_t mode() const { return mode_; }

 private:
  uint8_t encoder_;
  uint8_t push_;
  ModeSink& sink_;
  uint8_t mode_ = DETENT_ENCODER_MODE_VOLUME;
  uint8_t before_menu_ = DETENT_ENCODER_MODE_VOLUME;
  bool pressing_ = false;
  bool long_ = false;
  uint8_t press_mode_ = DETENT_ENCODER_MODE_VOLUME;
};

}  // namespace detent

#endif  // DETENT_MODES_H
