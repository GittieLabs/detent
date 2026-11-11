// A board's controls as its header in include/boards/ lists them: buttons and
// any number of encoders (none, one, several), each encoder with an optional
// push switch that is one of the buttons (M7, D-094, D-096). Pure C++11,
// tested natively (D-035).

#ifndef DETENT_LAYOUT_H
#define DETENT_LAYOUT_H

#include <stdint.h>

#include "report.h"

namespace detent {

struct ButtonPin {
  uint8_t control;  // protocol control_id
  uint8_t gpio;
};

// An encoder without a push switch. Never use 255 as a control ID.
const uint8_t kNoPush = 0xFF;

struct EncoderPins {
  uint8_t control;  // protocol control_id
  uint8_t a;        // GPIO of contact A
  uint8_t b;        // GPIO of contact B
  uint8_t push;     // control_id of a button in the same board, or kNoPush
};

// The index of the encoder whose push switch is button `control`, or -1.
int push_owner(const EncoderPins* encoders, uint8_t count, uint8_t control);

// The index of encoder `control`, or -1.
int encoder_index(const EncoderPins* encoders, uint8_t count, uint8_t control);

// Whether the layout is one HELLO can report: control IDs unique (255 unused),
// each push switch a listed button belonging to one encoder.
bool layout_ok(const ButtonPin* buttons, uint8_t button_count, const EncoderPins* encoders,
               uint8_t encoder_count);

// Fills `out` (button_count + encoder_count entries) with what HELLO lists:
// every button in order, then every encoder, carrying its push switch.
// Returns the number filled.
uint8_t hello_controls(const ButtonPin* buttons, uint8_t button_count,
                       const EncoderPins* encoders, uint8_t encoder_count, Control* out);

}  // namespace detent

#endif  // DETENT_LAYOUT_H
