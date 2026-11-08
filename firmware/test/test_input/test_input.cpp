// Native tests for lib/input: button debounce and long press, quadrature
// decoding, and detent coalescing, driven by simulated pin sequences.

#include <unity.h>

#include "input.h"

using detent::Button;
using detent::ButtonEvent;
using detent::Encoder;

void setUp() {}
void tearDown() {}

namespace {

// Feeds a button the same raw level every ms from `from` to `to` inclusive and
// collects its events in order.
struct Recorder {
  ButtonEvent events[16];
  uint32_t at[16];
  int count = 0;
  void feed(Button& b, bool pressed, uint32_t from, uint32_t to) {
    // By offset, so a range that wraps past zero still runs.
    for (uint32_t i = 0; i <= to - from; i++) {
      const uint32_t t = from + i;
      ButtonEvent e = b.update(pressed, t);
      if (e != ButtonEvent::None && count < 16) {
        events[count] = e;
        at[count] = t;
        count++;
      }
    }
  }
};

}  // namespace

// --- buttons ---

static void test_clean_press_and_release() {
  Button b;
  Recorder r;
  r.feed(b, false, 0, 100);
  r.feed(b, true, 101, 300);
  r.feed(b, false, 301, 400);
  TEST_ASSERT_EQUAL_INT(2, r.count);
  TEST_ASSERT_EQUAL(ButtonEvent::Down, r.events[0]);
  TEST_ASSERT_EQUAL_UINT32(101 + detent::kDebounceMs, r.at[0]);
  TEST_ASSERT_EQUAL(ButtonEvent::Up, r.events[1]);
  TEST_ASSERT_EQUAL_UINT32(301 + detent::kDebounceMs, r.at[1]);
}

static void test_bounce_gives_one_down_and_one_up() {
  Button b;
  Recorder r;
  r.feed(b, false, 0, 50);
  // Contact bounce: flips every 2 ms for 8 ms, then settles pressed.
  for (uint32_t t = 51; t < 59; t += 2) {
    r.feed(b, true, t, t);
    r.feed(b, false, t + 1, t + 1);
  }
  r.feed(b, true, 59, 200);
  // Release with bounce too.
  for (uint32_t t = 201; t < 209; t += 2) {
    r.feed(b, false, t, t);
    r.feed(b, true, t + 1, t + 1);
  }
  r.feed(b, false, 209, 300);
  TEST_ASSERT_EQUAL_INT(2, r.count);
  TEST_ASSERT_EQUAL(ButtonEvent::Down, r.events[0]);
  TEST_ASSERT_EQUAL(ButtonEvent::Up, r.events[1]);
}

static void test_glitch_shorter_than_debounce_is_ignored() {
  Button b;
  Recorder r;
  r.feed(b, false, 0, 50);
  r.feed(b, true, 51, 51 + detent::kDebounceMs - 2);
  r.feed(b, false, 51 + detent::kDebounceMs - 1, 200);
  TEST_ASSERT_EQUAL_INT(0, r.count);
}

static void test_long_press_fires_once_at_600ms_before_up() {
  Button b;
  Recorder r;
  r.feed(b, false, 0, 10);
  r.feed(b, true, 11, 2000);
  r.feed(b, false, 2001, 2100);
  TEST_ASSERT_EQUAL_INT(3, r.count);
  TEST_ASSERT_EQUAL(ButtonEvent::Down, r.events[0]);
  TEST_ASSERT_EQUAL(ButtonEvent::LongPress, r.events[1]);
  TEST_ASSERT_EQUAL_UINT32(r.at[0] + detent::kLongPressMs, r.at[1]);
  TEST_ASSERT_EQUAL(ButtonEvent::Up, r.events[2]);
}

static void test_short_press_has_no_long_press() {
  Button b;
  Recorder r;
  r.feed(b, true, 0, 500);
  r.feed(b, false, 501, 600);
  TEST_ASSERT_EQUAL_INT(2, r.count);
  TEST_ASSERT_EQUAL(ButtonEvent::Down, r.events[0]);
  TEST_ASSERT_EQUAL(ButtonEvent::Up, r.events[1]);
}

static void test_long_press_survives_a_late_poll() {
  // The loop may stall (a USB send) and poll late; long press still fires once.
  Button b;
  TEST_ASSERT_EQUAL(ButtonEvent::None, b.update(true, 0));
  TEST_ASSERT_EQUAL(ButtonEvent::Down, b.update(true, detent::kDebounceMs));
  TEST_ASSERT_EQUAL(ButtonEvent::LongPress, b.update(true, 5000));
  TEST_ASSERT_EQUAL(ButtonEvent::None, b.update(true, 5001));
  TEST_ASSERT_EQUAL(ButtonEvent::None, b.update(false, 5002));
  TEST_ASSERT_EQUAL(ButtonEvent::Up, b.update(false, 5002 + detent::kDebounceMs));
}

static void test_button_timing_survives_millis_wrap() {
  Button b;
  Recorder r;
  const uint32_t start = 0xFFFFFFFFu - 300;
  r.feed(b, true, start, start + 1000);  // wraps past zero
  TEST_ASSERT_EQUAL_INT(2, r.count);
  TEST_ASSERT_EQUAL(ButtonEvent::LongPress, r.events[1]);
}

// --- encoder ---

namespace {

// One detent clockwise for a common EC11 style encoder resting at A=1, B=1:
// 11 -> 01 -> 00 -> 10 -> 11. Counterclockwise is the reverse.
const bool kCwA[] = {0, 0, 1, 1};
const bool kCwB[] = {1, 0, 0, 1};

void turn(Encoder& e, int detents) {
  for (int d = 0; d < (detents < 0 ? -detents : detents); d++) {
    for (int i = 0; i < 4; i++) {
      const int k = detents > 0 ? i : 3 - i;
      // Counterclockwise walks the cycle backwards: 11 -> 10 -> 00 -> 01 -> 11.
      const bool a = detents > 0 ? kCwA[k] : (k == 0 ? 1 : kCwA[k - 1]);
      const bool b = detents > 0 ? kCwB[k] : (k == 0 ? 1 : kCwB[k - 1]);
      e.on_edge(a, b);
    }
  }
}

}  // namespace

static void test_one_detent_each_way() {
  Encoder e;
  e.reset(true, true);
  turn(e, 1);
  int8_t delta = 0;
  TEST_ASSERT_FALSE(e.poll(0, &delta));  // waits for the coalescing window
  TEST_ASSERT_TRUE(e.poll(detent::kCoalesceMs, &delta));
  TEST_ASSERT_EQUAL_INT8(1, delta);
  turn(e, -1);
  e.poll(100, &delta);
  TEST_ASSERT_TRUE(e.poll(100 + detent::kCoalesceMs, &delta));
  TEST_ASSERT_EQUAL_INT8(-1, delta);
}

static void test_half_a_detent_counts_nothing() {
  Encoder e;
  e.reset(true, true);
  e.on_edge(0, 1);
  e.on_edge(0, 0);
  int8_t delta = 0;
  TEST_ASSERT_FALSE(e.poll(1000, &delta));
}

static void test_bounce_on_one_contact_cancels_out() {
  Encoder e;
  e.reset(true, true);
  // A chatters 1 -> 0 -> 1 -> 0 -> 1 with B steady: back and forth, net zero.
  for (int i = 0; i < 4; i++) e.on_edge(i % 2 == 0 ? 0 : 1, 1);
  int8_t delta = 0;
  TEST_ASSERT_FALSE(e.poll(1000, &delta));
}

static void test_impossible_jumps_are_ignored() {
  Encoder e;
  e.reset(true, true);
  e.on_edge(0, 0);  // both changed at once: no direction, ignored
  e.on_edge(1, 1);
  int8_t delta = 0;
  TEST_ASSERT_FALSE(e.poll(1000, &delta));
}

static void test_fast_turn_coalesces_into_one_delta() {
  Encoder e;
  e.reset(true, true);
  int8_t delta = 0;
  turn(e, 1);
  TEST_ASSERT_FALSE(e.poll(0, &delta));  // window opens at the first detent
  turn(e, 2);
  TEST_ASSERT_FALSE(e.poll(detent::kCoalesceMs - 1, &delta));
  TEST_ASSERT_TRUE(e.poll(detent::kCoalesceMs, &delta));
  TEST_ASSERT_EQUAL_INT8(3, delta);
  TEST_ASSERT_FALSE(e.poll(1000, &delta));
}

static void test_reversal_inside_the_window_nets_out() {
  Encoder e;
  e.reset(true, true);
  int8_t delta = 0;
  turn(e, 1);
  e.poll(0, &delta);
  turn(e, -1);
  TEST_ASSERT_FALSE(e.poll(detent::kCoalesceMs, &delta));
  TEST_ASSERT_FALSE(e.poll(1000, &delta));
}

static void test_huge_counts_clamp_and_keep_the_rest() {
  Encoder e;
  e.reset(true, true);
  int8_t delta = 0;
  turn(e, 130);
  e.poll(0, &delta);
  TEST_ASSERT_TRUE(e.poll(detent::kCoalesceMs, &delta));
  TEST_ASSERT_EQUAL_INT8(127, delta);
  TEST_ASSERT_FALSE(e.poll(detent::kCoalesceMs + 1, &delta));
  TEST_ASSERT_TRUE(e.poll(2 * detent::kCoalesceMs + 1, &delta));
  TEST_ASSERT_EQUAL_INT8(3, delta);
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_clean_press_and_release);
  RUN_TEST(test_bounce_gives_one_down_and_one_up);
  RUN_TEST(test_glitch_shorter_than_debounce_is_ignored);
  RUN_TEST(test_long_press_fires_once_at_600ms_before_up);
  RUN_TEST(test_short_press_has_no_long_press);
  RUN_TEST(test_long_press_survives_a_late_poll);
  RUN_TEST(test_button_timing_survives_millis_wrap);
  RUN_TEST(test_one_detent_each_way);
  RUN_TEST(test_half_a_detent_counts_nothing);
  RUN_TEST(test_bounce_on_one_contact_cancels_out);
  RUN_TEST(test_impossible_jumps_are_ignored);
  RUN_TEST(test_fast_turn_coalesces_into_one_delta);
  RUN_TEST(test_reversal_inside_the_window_nets_out);
  RUN_TEST(test_huge_counts_clamp_and_keep_the_rest);
  return UNITY_END();
}
