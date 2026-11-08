// Native tests for lib/menu: the firmware owned OLED menu (Volume, Mapped,
// Setup, Device info) and the text each screen shows.

#include <string.h>
#include <unity.h>

#include "detent_protocol.h"
#include "menu.h"

using detent::Menu;
using detent::MenuAction;
using detent::Screen;

namespace {

detent::DeviceView view(uint8_t mode = DETENT_ENCODER_MODE_VOLUME, bool mounted = true,
                        bool paired = false) {
  detent::DeviceView v;
  v.product = "Detent Dev";
  v.serial = "E072A1FC4EDC";
  v.fw_major = 0;
  v.fw_minor = 1;
  v.fw_patch = 0;
  v.protocol = DETENT_PROTOCOL_VERSION;
  v.mode = mode;
  v.mounted = mounted;
  v.paired = paired;
  return v;
}

char lines[detent::kScreenLines][detent::kLineChars + 1];

void render(const Menu& m, const detent::DeviceView& v) { m.render(v, lines); }

}  // namespace

void setUp() { memset(lines, 0, sizeof lines); }
void tearDown() {}

// --- status screen ---

static void test_starts_on_the_status_screen() {
  Menu m;
  TEST_ASSERT_EQUAL(Screen::Status, m.screen());
  render(m, view());
  TEST_ASSERT_EQUAL_STRING("Detent Dev", lines[0]);
  TEST_ASSERT_EQUAL_STRING("Mode: Volume", lines[1]);
  TEST_ASSERT_EQUAL_STRING("USB: connected", lines[2]);
  TEST_ASSERT_EQUAL_STRING("Hold knob: menu", lines[4]);
}

static void test_status_shows_mode_and_usb_state() {
  Menu m;
  render(m, view(DETENT_ENCODER_MODE_MAPPED, true, true));
  TEST_ASSERT_EQUAL_STRING("Mode: Mapped", lines[1]);
  TEST_ASSERT_EQUAL_STRING("USB: paired", lines[2]);
  render(m, view(DETENT_ENCODER_MODE_VOLUME, false, false));
  TEST_ASSERT_EQUAL_STRING("USB: no host", lines[2]);
}

// --- menu list ---

static void test_open_lists_the_firmware_items_with_a_cursor() {
  Menu m;
  m.open();
  TEST_ASSERT_EQUAL(Screen::Menu, m.screen());
  TEST_ASSERT_EQUAL_UINT8(4, m.item_count());
  render(m, view(DETENT_ENCODER_MODE_VOLUME));
  TEST_ASSERT_EQUAL_STRING("Menu", lines[0]);
  TEST_ASSERT_EQUAL_STRING("> Volume *", lines[1]);  // * marks the active mode
  TEST_ASSERT_EQUAL_STRING("  Mapped", lines[2]);
  TEST_ASSERT_EQUAL_STRING("  Setup", lines[3]);
  TEST_ASSERT_EQUAL_STRING("  Device info", lines[4]);
}

static void test_scroll_moves_and_wraps_both_ways() {
  Menu m;
  m.open();
  m.scroll(1);
  TEST_ASSERT_EQUAL_UINT8(1, m.cursor());
  m.scroll(2);
  TEST_ASSERT_EQUAL_UINT8(3, m.cursor());
  m.scroll(1);
  TEST_ASSERT_EQUAL_UINT8(0, m.cursor());  // wraps to the top
  m.scroll(-1);
  TEST_ASSERT_EQUAL_UINT8(3, m.cursor());  // wraps to the bottom
  m.scroll(-9);                            // big turns wrap too
  TEST_ASSERT_EQUAL_UINT8(2, m.cursor());
  render(m, view());
  TEST_ASSERT_EQUAL_STRING("> Setup", lines[3]);
}

static void test_opening_again_starts_at_the_top() {
  Menu m;
  m.open();
  m.scroll(2);
  m.close();
  m.open();
  TEST_ASSERT_EQUAL_UINT8(0, m.cursor());
}

// --- selections ---

static void test_selecting_a_mode_returns_it() {
  Menu m;
  m.open();
  TEST_ASSERT_EQUAL(MenuAction::SetVolume, m.select());
  m.open();
  m.scroll(1);
  TEST_ASSERT_EQUAL(MenuAction::SetMapped, m.select());
}

static void test_setup_shows_the_serial_and_exiting_sends_hello() {
  Menu m;
  m.open();
  m.scroll(2);
  TEST_ASSERT_EQUAL(MenuAction::None, m.select());
  TEST_ASSERT_EQUAL(Screen::Setup, m.screen());
  render(m, view());
  TEST_ASSERT_EQUAL_STRING("Setup", lines[0]);
  TEST_ASSERT_EQUAL_STRING("E072A1FC4EDC", lines[2]);
  TEST_ASSERT_EQUAL_STRING("Press: done", lines[3]);
  m.scroll(3);  // ignored in Setup
  TEST_ASSERT_EQUAL(Screen::Setup, m.screen());
  TEST_ASSERT_EQUAL(MenuAction::SetupExited, m.select());
  TEST_ASSERT_EQUAL(Screen::Menu, m.screen());
}

static void test_closing_from_setup_also_counts_as_leaving_setup() {
  Menu m;
  m.open();
  m.scroll(2);
  m.select();
  TEST_ASSERT_EQUAL(MenuAction::SetupExited, m.close());
  TEST_ASSERT_EQUAL(Screen::Status, m.screen());
  m.open();
  TEST_ASSERT_EQUAL(MenuAction::None, m.close());
}

static void test_device_info_and_back() {
  Menu m;
  m.open();
  m.scroll(3);
  TEST_ASSERT_EQUAL(MenuAction::None, m.select());
  TEST_ASSERT_EQUAL(Screen::Info, m.screen());
  render(m, view(DETENT_ENCODER_MODE_VOLUME, true, true));
  TEST_ASSERT_EQUAL_STRING("Device info", lines[0]);
  TEST_ASSERT_EQUAL_STRING("fw 0.1.0", lines[1]);
  TEST_ASSERT_EQUAL_STRING("E072A1FC4EDC", lines[2]);
  TEST_ASSERT_EQUAL_STRING("proto 1  paired yes", lines[3]);
  TEST_ASSERT_EQUAL(MenuAction::None, m.select());
  TEST_ASSERT_EQUAL(Screen::Menu, m.screen());
  TEST_ASSERT_EQUAL_UINT8(3, m.cursor());  // back where it was
}

static void test_scroll_and_select_on_status_do_nothing() {
  Menu m;
  m.scroll(2);
  TEST_ASSERT_EQUAL(MenuAction::None, m.select());
  TEST_ASSERT_EQUAL(Screen::Status, m.screen());
}

// --- labels ---

static void test_labels_fit_16_chars() {
  char out[detent::kMaxLabel + 1];
  detent::fit_label("Volume", out);
  TEST_ASSERT_EQUAL_STRING("Volume", out);
  detent::fit_label("A very long dashboard name", out);
  TEST_ASSERT_EQUAL_size_t(16, strlen(out));
  TEST_ASSERT_EQUAL_STRING("A very long dash", out);
  detent::fit_label("", out);
  TEST_ASSERT_EQUAL_STRING("", out);
}

static void test_every_rendered_line_fits_the_screen() {
  Menu m;
  const Screen screens[] = {Screen::Status, Screen::Menu, Screen::Setup, Screen::Info};
  for (Screen s : screens) {
    m.close();
    if (s != Screen::Status) {
      m.open();
      if (s == Screen::Setup) { m.scroll(2); m.select(); }
      if (s == Screen::Info) { m.scroll(3); m.select(); }
    }
    render(m, view(DETENT_ENCODER_MODE_MAPPED, true, true));
    for (int i = 0; i < detent::kScreenLines; i++) {
      TEST_ASSERT_TRUE(strlen(lines[i]) <= detent::kLineChars);
    }
  }
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_starts_on_the_status_screen);
  RUN_TEST(test_status_shows_mode_and_usb_state);
  RUN_TEST(test_open_lists_the_firmware_items_with_a_cursor);
  RUN_TEST(test_scroll_moves_and_wraps_both_ways);
  RUN_TEST(test_opening_again_starts_at_the_top);
  RUN_TEST(test_selecting_a_mode_returns_it);
  RUN_TEST(test_setup_shows_the_serial_and_exiting_sends_hello);
  RUN_TEST(test_closing_from_setup_also_counts_as_leaving_setup);
  RUN_TEST(test_device_info_and_back);
  RUN_TEST(test_scroll_and_select_on_status_do_nothing);
  RUN_TEST(test_labels_fit_16_chars);
  RUN_TEST(test_every_rendered_line_fits_the_screen);
  return UNITY_END();
}
