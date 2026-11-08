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
uint8_t scan[8];
int scan_count = 0;
char shown[kScreenLines][kLineChars + 1];
bool drawn_once = false;

}  // namespace

bool begin() {
  Wire.begin(board::kOledSda, board::kOledScl);
  for (uint8_t a = 0x08; a < 0x78 && scan_count < 8; a++) {
    Wire.beginTransmission(a);
    if (Wire.endTransmission() == 0) scan[scan_count++] = a;
  }
  Wire.beginTransmission(board::kOledAddress);
  found = Wire.endTransmission() == 0;
  if (!found) return false;
  oled.setBusClock(400000);
  oled.begin();
  oled.setFont(u8g2_font_6x12_tr);
#if DETENT_DIAG
  // Self test: every pixel on at full contrast for 2 s, so a dark panel can be
  // told apart from a drawing problem.
  oled.setPowerSave(0);
  oled.setContrast(255);
  oled.clearBuffer();
  oled.drawBox(0, 0, 128, 64);
  oled.sendBuffer();
  delay(2000);
#endif
  return true;
}

int scanned(const uint8_t** addresses) {
  *addresses = scan;
  return scan_count;
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
