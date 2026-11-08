#include "display.h"

#include <U8g2lib.h>
#include <Wire.h>
#include <string.h>

#include "board_pins.h"

namespace detent {
namespace display {
namespace {

// Full frame buffer (1 KB): one I2C transfer per redraw.
#if DETENT_OLED_SH1106
U8G2_SH1106_128X64_NONAME_F_HW_I2C oled(U8G2_R0, U8X8_PIN_NONE, board::kOledScl, board::kOledSda);
#else
U8G2_SSD1306_128X64_NONAME_F_HW_I2C oled(U8G2_R0, U8X8_PIN_NONE, board::kOledScl, board::kOledSda);
#endif

bool found = false;
char shown[kScreenLines][kLineChars + 1];
bool drawn_once = false;

}  // namespace

bool begin() {
  Wire.begin(board::kOledSda, board::kOledScl);
  Wire.beginTransmission(board::kOledAddress);
  found = Wire.endTransmission() == 0;
  if (!found) return false;
  oled.setBusClock(400000);
  oled.begin();
  oled.setFont(u8g2_font_6x12_tr);
  return true;
}

bool present() { return found; }

void draw(const char lines[kScreenLines][kLineChars + 1]) {
  if (!found) return;
  if (drawn_once && memcmp(lines, shown, sizeof shown) == 0) return;
  memcpy(shown, lines, sizeof shown);
  drawn_once = true;
  oled.clearBuffer();
  for (int i = 0; i < kScreenLines; i++) {
    // Baselines 10, 22, 34, 46, 58: 12 px rows on a 64 px screen.
    oled.drawStr(0, 10 + 12 * i, lines[i]);
  }
  oled.sendBuffer();
}

}  // namespace display
}  // namespace detent
