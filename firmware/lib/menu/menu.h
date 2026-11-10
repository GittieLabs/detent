// The OLED's screens and its menu: the firmware owned items (Volume, Mapped,
// Setup, Device info), then up to kMaxAppItems app owned ones from MENU, and
// the status screen's line from STATUS. Pure C++11: it keeps state and
// renders text lines, and src/display.cpp draws them. Tested natively (D-035).

#ifndef DETENT_MENU_H
#define DETENT_MENU_H

#include <stdint.h>

namespace detent {

const int kScreenLines = 5;   // 128x64 at a 12 px line height
const int kLineChars = 21;    // 6 px wide font
const int kMaxLabel = 16;     // protocol: menu labels are at most 16 chars
const int kMaxAppItems = 8;   // app owned items kept; more are ignored
const int kMaxStatus = 20;    // protocol: STATUS text is at most 20 chars

enum class Screen : uint8_t { Status, Menu, Setup, Info };

// What a selection asks the firmware to do.
enum class MenuAction : uint8_t { None, SetVolume, SetMapped, SetupExited, AppItem };

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
  // Acts on the list item, or leaves Setup or Info. For AppItem, `app_item`
  // (if given) gets the picked item's id.
  MenuAction select(uint8_t* app_item = nullptr);

  // One MENU message: item `index` of `total`. The set replaces the shown
  // one only once every index has arrived; total == 0 clears it.
  void set_app_item(uint8_t total, uint8_t index, uint8_t id, const char* label);
  // The host unmounted: app items and the status line go.
  void clear_app_items();
  // STATUS: the status screen's line; empty clears it.
  void set_status(const char* text);

  Screen screen() const { return screen_; }
  uint8_t cursor() const { return cursor_; }
  uint8_t item_count() const;

  void render(const DeviceView& view, char lines[kScreenLines][kLineChars + 1]) const;

 private:
  struct AppItem {
    uint8_t id;
    char label[kMaxLabel + 1];
  };

  Screen screen_ = Screen::Status;
  uint8_t cursor_ = 0;
  AppItem items_[kMaxAppItems] = {};
  uint8_t item_total_ = 0;
  AppItem pending_[kMaxAppItems] = {};
  uint8_t pending_total_ = 0;  // the set arriving; 0 when none
  uint8_t pending_have_ = 0;   // bitmask of indexes received
  char status_[kMaxStatus + 1] = {};
};

}  // namespace detent

#endif  // DETENT_MENU_H
