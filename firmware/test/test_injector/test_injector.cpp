// Native tests for lib/injector: TEST_INJECT scheduling, and that a release
// build (injector disabled) drops TEST_INJECT entirely.

#include <string.h>
#include <unity.h>

#include "../vectors.h"
#include "injector.h"
#include "report.h"

using detent::AppMessage;
using detent::InjectEvent;
using detent::Injector;

namespace {

AppMessage script(const InjectEvent* events, uint8_t count) {
  AppMessage m;
  memset(&m, 0, sizeof m);
  m.type = DETENT_MSG_TEST_INJECT;
  m.inject_count = count;
  memcpy(m.inject, events, count * sizeof(InjectEvent));
  return m;
}

const InjectEvent kDown = {DETENT_MSG_BUTTON, 1, DETENT_ACTION_DOWN, 0};
const InjectEvent kUp = {DETENT_MSG_BUTTON, 1, DETENT_ACTION_UP, 100};
const InjectEvent kTurn = {DETENT_MSG_ENCODER, 5, static_cast<uint8_t>(-2), 50};

}  // namespace

void setUp() {}
void tearDown() {}

static void test_release_build_drops_test_inject() {
  Injector inj(false);
  const InjectEvent evs[] = {kDown};
  const AppMessage m = script(evs, 1);
  TEST_ASSERT_FALSE(inj.load(m, 0));
  InjectEvent out;
  TEST_ASSERT_FALSE(inj.next_due(1000000, &out));
  TEST_ASSERT_FALSE(inj.active());
}

static void test_only_loads_test_inject() {
  Injector inj(true);
  AppMessage m;
  memset(&m, 0, sizeof m);
  m.type = DETENT_MSG_STATUS;
  TEST_ASSERT_FALSE(inj.load(m, 0));
  TEST_ASSERT_FALSE(inj.active());
}

static void test_delays_are_relative_to_the_previous_event() {
  Injector inj(true);
  const InjectEvent evs[] = {kDown, kUp, kTurn};
  TEST_ASSERT_TRUE(inj.load(script(evs, 3), 1000));
  InjectEvent out;
  // kDown: 0 ms after load.
  TEST_ASSERT_TRUE(inj.next_due(1000, &out));
  TEST_ASSERT_EQUAL_UINT8(DETENT_ACTION_DOWN, out.value);
  // kUp: 100 ms after kDown.
  TEST_ASSERT_FALSE(inj.next_due(1099, &out));
  TEST_ASSERT_TRUE(inj.next_due(1100, &out));
  TEST_ASSERT_EQUAL_UINT8(DETENT_ACTION_UP, out.value);
  // kTurn: 50 ms after kUp.
  TEST_ASSERT_FALSE(inj.next_due(1149, &out));
  TEST_ASSERT_TRUE(inj.next_due(1150, &out));
  TEST_ASSERT_EQUAL_UINT8(DETENT_MSG_ENCODER, out.type);
  TEST_ASSERT_EQUAL_INT8(-2, static_cast<int8_t>(out.value));
  TEST_ASSERT_FALSE(inj.active());
  TEST_ASSERT_FALSE(inj.next_due(5000, &out));
}

static void test_late_ticks_release_due_events_in_order() {
  Injector inj(true);
  const InjectEvent evs[] = {kDown, kUp, kTurn};
  inj.load(script(evs, 3), 0);
  InjectEvent out;
  TEST_ASSERT_TRUE(inj.next_due(500, &out));
  TEST_ASSERT_EQUAL_UINT8(DETENT_ACTION_DOWN, out.value);
  TEST_ASSERT_TRUE(inj.next_due(500, &out));
  TEST_ASSERT_EQUAL_UINT8(DETENT_ACTION_UP, out.value);
  TEST_ASSERT_TRUE(inj.next_due(500, &out));
  TEST_ASSERT_EQUAL_UINT8(DETENT_MSG_ENCODER, out.type);
  TEST_ASSERT_FALSE(inj.next_due(500, &out));
}

static void test_a_new_script_replaces_the_running_one() {
  Injector inj(true);
  const InjectEvent first[] = {kDown, kUp};
  inj.load(script(first, 2), 0);
  InjectEvent out;
  TEST_ASSERT_TRUE(inj.next_due(0, &out));  // kDown
  const InjectEvent second[] = {kTurn};
  inj.load(script(second, 1), 10);
  TEST_ASSERT_FALSE(inj.next_due(59, &out));
  TEST_ASSERT_TRUE(inj.next_due(60, &out));
  TEST_ASSERT_EQUAL_UINT8(DETENT_MSG_ENCODER, out.type);
  TEST_ASSERT_FALSE(inj.next_due(10000, &out));  // kUp never comes
}

static void test_schedule_survives_millis_wrap() {
  Injector inj(true);
  const InjectEvent evs[] = {kUp};  // 100 ms
  inj.load(script(evs, 1), 0xFFFFFFFFu - 40);
  InjectEvent out;
  TEST_ASSERT_FALSE(inj.next_due(0xFFFFFFFFu, &out));
  TEST_ASSERT_TRUE(inj.next_due(60, &out));
}

static void test_loads_the_golden_vector() {
  Injector inj(true);
  AppMessage m;
  TEST_ASSERT_EQUAL(detent::ParseError::None,
                    detent::parse_app(DETENT_VEC_PARSE_TEST_INJECT, DETENT_REPORT_LEN, &m));
  TEST_ASSERT_TRUE(inj.load(m, 0));
  InjectEvent out;
  TEST_ASSERT_FALSE(inj.next_due(15, &out));
  TEST_ASSERT_TRUE(inj.next_due(16, &out));  // button 4 down after 16 ms
  TEST_ASSERT_EQUAL_UINT8(4, out.control);
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_release_build_drops_test_inject);
  RUN_TEST(test_only_loads_test_inject);
  RUN_TEST(test_delays_are_relative_to_the_previous_event);
  RUN_TEST(test_late_ticks_release_due_events_in_order);
  RUN_TEST(test_a_new_script_replaces_the_running_one);
  RUN_TEST(test_schedule_survives_millis_wrap);
  RUN_TEST(test_loads_the_golden_vector);
  return UNITY_END();
}
