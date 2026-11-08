// The OLED's screens and the firmware owned menu: Volume, Mapped, Setup,
// Device info. Pure C++11: it keeps state and renders text lines, and
// src/display.cpp draws them. Tested natively (D-035).
//
// App owned items (the MENU message) join this list in M6.

#ifndef DETENT_MENU_H
#define DETENT_MENU_H

#include <stdint.h>

namespace detent {

const int kScreenLines = 5;   // 128x64 at a 12 px line height
const int kLineChars = 21;    // 6 px wide font
const int kMaxLabel = 16;     // protocol: menu labels are at most 16 chars

enum class Screen : uint8_t { Status, Menu, Setup, Info };

// What a selection asks the firmware to do.
enum class MenuAction : uint8_t { None, SetVolume, SetMapped, SetupExited };

// What the screens show; filled by the firmware each render.
struct DeviceView {
  const char* product;
  const char* serial;
  uint8_t fw_major;
  uint8_t fw_minor;
  uint8_t fw_patch;
  uint8_t protocol;
  uint8_t mode;  // DETENT_ENCODER_MODE_*
  bool mounted;
  bool paired;
};

// Copies at most kMaxLabel chars of `in`.
void fit_label(const char* in, char out[kMaxLabel + 1]);

class Menu {
 public:
  // Shows the menu list, cursor at the top.
  void open();
  // Back to the status screen. Returns SetupExited if Setup was showing,
  // because leaving Setup any way sends HELLO.
  MenuAction close();

  // Moves the cursor in the list (wrapping); ignored on other screens.
  void scroll(int8_t delta);
  // Acts on the list item, or leaves Setup or Info.
  MenuAction select();

  Screen screen() const { return screen_; }
  uint8_t cursor() const { return cursor_; }
  uint8_t item_count() const;

  void render(const DeviceView& view, char lines[kScreenLines][kLineChars + 1]) const;

 private:
  Screen screen_ = Screen::Status;
  uint8_t cursor_ = 0;
};

}  // namespace detent

#endif  // DETENT_MENU_H
