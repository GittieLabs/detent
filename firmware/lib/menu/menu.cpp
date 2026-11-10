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

uint8_t Menu::item_count() const { return kItemCount + item_total_; }

void Menu::set_app_item(uint8_t total, uint8_t index, uint8_t id, const char* label) {
  if (total == 0) {
    item_total_ = 0;
    pending_total_ = 0;
    pending_have_ = 0;
    if (cursor_ >= kItemCount) cursor_ = 0;
    return;
  }
  const uint8_t kept = total > kMaxAppItems ? kMaxAppItems : total;
  if (index >= kept) return;  // past the items kept
  if (pending_total_ != kept) {
    pending_total_ = kept;
    pending_have_ = 0;
  }
  pending_[index].id = id;
  fit_label(label, pending_[index].label);
  pending_have_ |= static_cast<uint8_t>(1u << index);
  const uint8_t all = static_cast<uint8_t>((1u << kept) - 1u);
  if (pending_have_ == all) {
    memcpy(items_, pending_, sizeof items_);
    item_total_ = kept;
    pending_total_ = 0;
    pending_have_ = 0;
    if (cursor_ >= item_count()) cursor_ = 0;
  }
}

void Menu::clear_app_items() {
  set_app_item(0, 0, 0, "");
  status_[0] = '\0';
}

void Menu::set_status(const char* text) {
  size_t n = strlen(text);
  if (n > static_cast<size_t>(kMaxStatus)) n = kMaxStatus;
  memcpy(status_, text, n);
  status_[n] = '\0';
}

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
  const int n = item_count();
  int c = (static_cast<int>(cursor_) + delta) % n;
  if (c < 0) c += n;
  cursor_ = static_cast<uint8_t>(c);
}

MenuAction Menu::select(uint8_t* app_item) {
  switch (screen_) {
    case Screen::Menu:
      if (cursor_ >= kItemCount) {
        if (app_item) *app_item = items_[cursor_ - kItemCount].id;
        return MenuAction::AppItem;
      }
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
      LINE(lines[3], "%s", status_);
      LINE(lines[4], "Hold knob: menu");
      break;
    case Screen::Menu:
      LINE(lines[0], "Menu");
      {
        // A window of the list that keeps the cursor in view.
        const int rows = kScreenLines - 1;
        const int n = item_count();
        int first = cursor_ - (rows - 1);
        if (first < 0) first = 0;
        for (int r = 0; r < rows && first + r < n; r++) {
          const int i = first + r;
          char label[kMaxLabel + 1];
          fit_label(i < kItemCount ? kItemLabels[i] : items_[i - kItemCount].label, label);
          const bool active = (i == kItemVolume && v.mode == DETENT_ENCODER_MODE_VOLUME) ||
                              (i == kItemMapped && v.mode == DETENT_ENCODER_MODE_MAPPED);
          LINE(lines[r + 1], "%s%s%s", i == cursor_ ? "> " : "  ", label, active ? " *" : "");
        }
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
