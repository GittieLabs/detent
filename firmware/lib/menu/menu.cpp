#include "menu.h"

#include <stdio.h>
#include <string.h>

#include "detent_protocol.h"

namespace detent {
namespace {

enum Item : uint8_t { kItemVolume, kItemMapped, kItemSetup, kItemInfo, kItemCount };

const char* const kItemLabels[kItemCount] = {"Volume", "Mapped", "Setup", "Device info"};

const char* mode_name(uint8_t mode) {
  switch (mode) {
    case DETENT_ENCODER_MODE_VOLUME: return "Volume";
    case DETENT_ENCODER_MODE_MAPPED: return "Mapped";
    case DETENT_ENCODER_MODE_MENU: return "Menu";
    default: return "?";
  }
}


}  // namespace

// snprintf into a line: always NUL terminated, at most kLineChars, and the
// format stays a literal at each call so the compiler checks it.
#define LINE(out, ...) snprintf((out), kLineChars + 1, __VA_ARGS__)

void fit_label(const char* in, char out[kMaxLabel + 1]) {
  size_t n = strlen(in);
  if (n > static_cast<size_t>(kMaxLabel)) n = kMaxLabel;
  memcpy(out, in, n);
  out[n] = '\0';
}

uint8_t Menu::item_count() const { return kItemCount; }

void Menu::open() {
  screen_ = Screen::Menu;
  cursor_ = 0;
}

MenuAction Menu::close() {
  const bool was_setup = screen_ == Screen::Setup;
  screen_ = Screen::Status;
  return was_setup ? MenuAction::SetupExited : MenuAction::None;
}

void Menu::scroll(int8_t delta) {
  if (screen_ != Screen::Menu) return;
  const int n = kItemCount;
  int c = (static_cast<int>(cursor_) + delta) % n;
  if (c < 0) c += n;
  cursor_ = static_cast<uint8_t>(c);
}

MenuAction Menu::select() {
  switch (screen_) {
    case Screen::Menu:
      switch (cursor_) {
        case kItemVolume: return MenuAction::SetVolume;
        case kItemMapped: return MenuAction::SetMapped;
        case kItemSetup: screen_ = Screen::Setup; return MenuAction::None;
        case kItemInfo: screen_ = Screen::Info; return MenuAction::None;
        default: return MenuAction::None;
      }
    case Screen::Setup:
      screen_ = Screen::Menu;
      return MenuAction::SetupExited;
    case Screen::Info:
      screen_ = Screen::Menu;
      return MenuAction::None;
    case Screen::Status:
    default:
      return MenuAction::None;
  }
}

void Menu::render(const DeviceView& v, char lines[kScreenLines][kLineChars + 1]) const {
  for (int i = 0; i < kScreenLines; i++) lines[i][0] = '\0';
  switch (screen_) {
    case Screen::Status:
      LINE(lines[0], "%s", v.product);
      LINE(lines[1], "Mode: %s", mode_name(v.mode));
      LINE(lines[2], "USB: %s", !v.mounted ? "no host" : v.paired ? "paired" : "connected");
      LINE(lines[4], "Hold knob: menu");
      break;
    case Screen::Menu:
      LINE(lines[0], "Menu");
      for (int i = 0; i < kItemCount && i + 1 < kScreenLines; i++) {
        char label[kMaxLabel + 1];
        fit_label(kItemLabels[i], label);
        const bool active = (i == kItemVolume && v.mode == DETENT_ENCODER_MODE_VOLUME) ||
                            (i == kItemMapped && v.mode == DETENT_ENCODER_MODE_MAPPED);
        LINE(lines[i + 1], "%s%s%s", i == cursor_ ? "> " : "  ", label, active ? " *" : "");
      }
      break;
    case Screen::Setup:
      LINE(lines[0], "Setup");
      LINE(lines[1], "Serial");
      LINE(lines[2], "%s", v.serial);
      LINE(lines[3], "Press: done");
      break;
    case Screen::Info:
      LINE(lines[0], "Device info");
      // Each line fits 21 chars even with every number at 255.
      LINE(lines[1], "fw %u.%u.%u", v.fw_major, v.fw_minor, v.fw_patch);
      LINE(lines[2], "%s", v.serial);
      LINE(lines[3], "proto %u  paired %s", v.protocol, v.paired ? "yes" : "no");
      LINE(lines[4], "Press: back");
      break;
  }
}

}  // namespace detent
