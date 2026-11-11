// Native tests for lib/layout: a board's buttons and any number of encoders
// (none, one, two), which button is whose push switch, and what HELLO lists.

#include <unity.h>

#include "detent_protocol.h"
#include "layout.h"
#include "report.h"

using detent::ButtonPin;
using detent::Control;
using detent::EncoderPins;
using detent::kNoPush;

namespace {

const ButtonPin kButtons[] = {{1, 10}, {2, 11}, {6, 12}, {8, 13}};

void expect(const Control& c, uint8_t id, uint8_t kind, uint8_t push) {
  TEST_ASSERT_EQUAL_UINT8(id, c.id);
  TEST_ASSERT_EQUAL_UINT8(kind, c.kind);
  TEST_ASSERT_EQUAL_UINT8(push, c.push);
}

}  // namespace

void setUp() {}
void tearDown() {}

static void test_no_encoder_lists_buttons_only() {
  Control out[4];
  TEST_ASSERT_EQUAL_UINT8(4, detent::hello_controls(kButtons, 4, nullptr, 0, out));
  expect(out[0], 1, DETENT_KIND_BUTTON, 0);
  expect(out[3], 8, DETENT_KIND_BUTTON, 0);
  TEST_ASSERT_EQUAL_INT(-1, detent::push_owner(nullptr, 0, 6));
  TEST_ASSERT_EQUAL_INT(-1, detent::encoder_index(nullptr, 0, 5));
}

static void test_one_encoder_with_its_push() {
  const EncoderPins enc[] = {{5, 20, 21, 6}};
  Control out[5];
  TEST_ASSERT_EQUAL_UINT8(5, detent::hello_controls(kButtons, 4, enc, 1, out));
  expect(out[4], 5, DETENT_KIND_ENCODER_WITH_PUSH, 6);
  TEST_ASSERT_EQUAL_INT(0, detent::push_owner(enc, 1, 6));
  TEST_ASSERT_EQUAL_INT(-1, detent::push_owner(enc, 1, 1));
  TEST_ASSERT_EQUAL_INT(0, detent::encoder_index(enc, 1, 5));
  // What the firmware packs is a valid v2 HELLO.
  uint8_t report[DETENT_REPORT_LEN];
  TEST_ASSERT_TRUE(detent::pack_hello(report, 0, 0, 2, 0, 0, out, 5));
}

static void test_two_encoders_one_without_a_push() {
  const EncoderPins enc[] = {{5, 20, 21, 6}, {7, 22, 23, kNoPush}};
  Control out[6];
  TEST_ASSERT_EQUAL_UINT8(6, detent::hello_controls(kButtons, 4, enc, 2, out));
  expect(out[4], 5, DETENT_KIND_ENCODER_WITH_PUSH, 6);
  expect(out[5], 7, DETENT_KIND_ENCODER, 0);
  TEST_ASSERT_EQUAL_INT(1, detent::encoder_index(enc, 2, 7));
  TEST_ASSERT_EQUAL_INT(-1, detent::push_owner(enc, 2, 8));
  uint8_t report[DETENT_REPORT_LEN];
  TEST_ASSERT_TRUE(detent::pack_hello(report, 0, 0, 2, 0, 0, out, 6));
}

static void test_two_encoders_each_with_a_push() {
  const EncoderPins enc[] = {{5, 20, 21, 6}, {7, 22, 23, 8}};
  Control out[6];
  TEST_ASSERT_EQUAL_UINT8(6, detent::hello_controls(kButtons, 4, enc, 2, out));
  TEST_ASSERT_EQUAL_INT(1, detent::push_owner(enc, 2, 8));
  uint8_t report[DETENT_REPORT_LEN];
  TEST_ASSERT_TRUE(detent::pack_hello(report, 0, 0, 2, 0, 0, out, 6));
}

static void test_layout_checks_catch_board_header_mistakes() {
  const EncoderPins good[] = {{5, 20, 21, 6}, {7, 22, 23, 8}};
  TEST_ASSERT_TRUE(detent::layout_ok(kButtons, 4, good, 2));
  const EncoderPins unlisted[] = {{5, 20, 21, 9}};
  TEST_ASSERT_FALSE(detent::layout_ok(kButtons, 4, unlisted, 1));
  const EncoderPins shared[] = {{5, 20, 21, 6}, {7, 22, 23, 6}};
  TEST_ASSERT_FALSE(detent::layout_ok(kButtons, 4, shared, 2));
  const EncoderPins clash[] = {{2, 20, 21, kNoPush}};  // encoder reuses button 2's ID
  TEST_ASSERT_FALSE(detent::layout_ok(kButtons, 4, clash, 1));
  TEST_ASSERT_TRUE(detent::layout_ok(kButtons, 4, nullptr, 0));
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_no_encoder_lists_buttons_only);
  RUN_TEST(test_one_encoder_with_its_push);
  RUN_TEST(test_two_encoders_one_without_a_push);
  RUN_TEST(test_two_encoders_each_with_a_push);
  RUN_TEST(test_layout_checks_catch_board_header_mistakes);
  return UNITY_END();
}
