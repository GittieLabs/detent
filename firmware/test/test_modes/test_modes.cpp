// Native tests for lib/modes: what the encoder and its push switch do in
// Volume, Mapped, and Menu modes, and how modes change.

#include <unity.h>

#include "detent_protocol.h"
#include "modes.h"

using detent::ButtonEvent;
using detent::ModeRouter;
using detent::ModeSink;

namespace {

const uint8_t kEncoder = 5;
const uint8_t kPush = 6;

struct FakeSink : ModeSink {
  int volume = 0;  // net volume steps
  int mutes = 0;
  int encoder_sum = 0;
  int encoder_calls = 0;
  int buttons = 0;
  uint8_t last_button_control = 0;
  uint8_t last_button_action = 0;
  int scroll = 0;
  int selects = 0;
  int mode_changes = 0;
  uint8_t last_mode = 0xFF;

  void volume_steps(int8_t delta) override { volume += delta; }
  void mute_toggle() override { mutes++; }
  void encoder(uint8_t, int8_t delta) override {
    encoder_sum += delta;
    encoder_calls++;
  }
  void button(uint8_t control, uint8_t action) override {
    buttons++;
    last_button_control = control;
    last_button_action = action;
  }
  void menu_scroll(int8_t delta) override { scroll += delta; }
  void menu_select() override { selects++; }
  void mode_changed(uint8_t mode) override {
    mode_changes++;
    last_mode = mode;
  }
};

FakeSink* sink;
ModeRouter* router;

void press(ButtonEvent a, ButtonEvent b = ButtonEvent::None, ButtonEvent c = ButtonEvent::None) {
  router->on_push(a);
  if (b != ButtonEvent::None) router->on_push(b);
  if (c != ButtonEvent::None) router->on_push(c);
}

}  // namespace

void setUp() {
  sink = new FakeSink();
  router = new ModeRouter(kEncoder, kPush, *sink);
}

void tearDown() {
  delete router;
  delete sink;
}

static void test_boots_in_volume_mode() {
  TEST_ASSERT_EQUAL_UINT8(DETENT_ENCODER_MODE_VOLUME, router->mode());
}

static void test_volume_turns_change_volume_only() {
  router->on_turn(3);
  router->on_turn(-1);
  TEST_ASSERT_EQUAL_INT(2, sink->volume);
  TEST_ASSERT_EQUAL_INT(0, sink->encoder_calls);
  TEST_ASSERT_EQUAL_INT(0, sink->buttons);
}

static void test_volume_short_press_toggles_mute_on_release() {
  router->on_push(ButtonEvent::Down);
  TEST_ASSERT_EQUAL_INT(0, sink->mutes);  // not on press: it might be a long press
  router->on_push(ButtonEvent::Up);
  TEST_ASSERT_EQUAL_INT(1, sink->mutes);
  TEST_ASSERT_EQUAL_INT(0, sink->buttons);
}

static void test_long_press_opens_menu_without_muting() {
  press(ButtonEvent::Down, ButtonEvent::LongPress, ButtonEvent::Up);
  TEST_ASSERT_EQUAL_UINT8(DETENT_ENCODER_MODE_MENU, router->mode());
  TEST_ASSERT_EQUAL_INT(0, sink->mutes);
  TEST_ASSERT_EQUAL_INT(1, sink->mode_changes);
  TEST_ASSERT_EQUAL_UINT8(DETENT_ENCODER_MODE_MENU, sink->last_mode);
}

static void test_menu_turns_scroll_and_short_press_selects() {
  press(ButtonEvent::Down, ButtonEvent::LongPress, ButtonEvent::Up);
  router->on_turn(2);
  router->on_turn(-1);
  TEST_ASSERT_EQUAL_INT(1, sink->scroll);
  TEST_ASSERT_EQUAL_INT(0, sink->volume);
  press(ButtonEvent::Down, ButtonEvent::Up);
  TEST_ASSERT_EQUAL_INT(1, sink->selects);
  TEST_ASSERT_EQUAL_INT(0, sink->mutes);
}

static void test_long_press_in_menu_returns_to_the_previous_mode() {
  TEST_ASSERT_TRUE(router->set_mode(DETENT_ENCODER_MODE_MAPPED));
  press(ButtonEvent::Down, ButtonEvent::LongPress, ButtonEvent::Up);
  TEST_ASSERT_EQUAL_UINT8(DETENT_ENCODER_MODE_MENU, router->mode());
  press(ButtonEvent::Down, ButtonEvent::LongPress, ButtonEvent::Up);
  TEST_ASSERT_EQUAL_UINT8(DETENT_ENCODER_MODE_MAPPED, router->mode());
  TEST_ASSERT_EQUAL_INT(0, sink->selects);
}

static void test_mapped_turns_and_presses_go_to_the_app() {
  router->set_mode(DETENT_ENCODER_MODE_MAPPED);
  router->on_turn(-2);
  TEST_ASSERT_EQUAL_INT(-2, sink->encoder_sum);
  TEST_ASSERT_EQUAL_INT(0, sink->volume);
  router->on_push(ButtonEvent::Down);
  TEST_ASSERT_EQUAL_INT(1, sink->buttons);  // immediately, not on release
  TEST_ASSERT_EQUAL_UINT8(kPush, sink->last_button_control);
  TEST_ASSERT_EQUAL_UINT8(DETENT_ACTION_DOWN, sink->last_button_action);
  router->on_push(ButtonEvent::Up);
  TEST_ASSERT_EQUAL_UINT8(DETENT_ACTION_UP, sink->last_button_action);
}

static void test_mapped_long_press_opens_menu_but_still_releases() {
  router->set_mode(DETENT_ENCODER_MODE_MAPPED);
  press(ButtonEvent::Down, ButtonEvent::LongPress, ButtonEvent::Up);
  TEST_ASSERT_EQUAL_UINT8(DETENT_ENCODER_MODE_MENU, router->mode());
  // Down and Up reach the app (no stuck button); LongPress is consumed.
  TEST_ASSERT_EQUAL_INT(2, sink->buttons);
  TEST_ASSERT_EQUAL_UINT8(DETENT_ACTION_UP, sink->last_button_action);
}

static void test_release_after_switching_mode_does_not_act_in_the_new_mode() {
  // The Up that ends the long press must not select a menu item.
  press(ButtonEvent::Down, ButtonEvent::LongPress, ButtonEvent::Up);
  TEST_ASSERT_EQUAL_INT(0, sink->selects);
}

static void test_set_mode_validates_and_reports() {
  TEST_ASSERT_FALSE(router->set_mode(7));
  TEST_ASSERT_EQUAL_INT(0, sink->mode_changes);
  TEST_ASSERT_TRUE(router->set_mode(DETENT_ENCODER_MODE_MAPPED));
  TEST_ASSERT_EQUAL_INT(1, sink->mode_changes);
  TEST_ASSERT_TRUE(router->set_mode(DETENT_ENCODER_MODE_MAPPED));  // no change
  TEST_ASSERT_EQUAL_INT(1, sink->mode_changes);
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_boots_in_volume_mode);
  RUN_TEST(test_volume_turns_change_volume_only);
  RUN_TEST(test_volume_short_press_toggles_mute_on_release);
  RUN_TEST(test_long_press_opens_menu_without_muting);
  RUN_TEST(test_menu_turns_scroll_and_short_press_selects);
  RUN_TEST(test_long_press_in_menu_returns_to_the_previous_mode);
  RUN_TEST(test_mapped_turns_and_presses_go_to_the_app);
  RUN_TEST(test_mapped_long_press_opens_menu_but_still_releases);
  RUN_TEST(test_release_after_switching_mode_does_not_act_in_the_new_mode);
  RUN_TEST(test_set_mode_validates_and_reports);
  return UNITY_END();
}
