// The OLED: an SSD1306 class 128x64 display over I2C (pins in board_pins.h).
// Draws the text lines lib/menu renders; nothing here decides what to show.

#ifndef DETENT_DISPLAY_H
#define DETENT_DISPLAY_H

#include "menu.h"

namespace detent {
namespace display {

// Starts I2C and the display. Returns false, and every draw does nothing, when
// no display answers at its I2C address, so a board without one doesn't stall
// the loop on I2C timeouts.
bool begin();
bool present();

// Addresses that answered an I2C bus scan at begin(), for diagnostics.
int scanned(const uint8_t** addresses);

// Redraws only when the lines changed.
void draw(const char lines[kScreenLines][kLineChars + 1]);

}  // namespace display
}  // namespace detent

#endif  // DETENT_DISPLAY_H
