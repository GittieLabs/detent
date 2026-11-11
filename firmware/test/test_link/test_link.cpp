// Native tests for lib/link_layer: when HELLO and HEARTBEAT go out, seq handling,
// pairing, and what gets handed back to the caller. Expected bytes come from
// lib/report's pack functions, which are golden tested against Rust.

#include <string.h>
#include <unity.h>

#include "../vectors.h"
#include "link_layer.h"
#include "report.h"

using detent::AppMessage;
using detent::Control;
using detent::Link;
using detent::ParseError;

namespace {

const size_t kMaxSent = 600;

struct FakeTransport : detent::LinkTransport {
  uint8_t sent[kMaxSent][DETENT_REPORT_LEN];
  size_t count = 0;
  bool send(const uint8_t report[DETENT_REPORT_LEN]) override {
    if (count < kMaxSent) memcpy(sent[count], report, DETENT_REPORT_LEN);
    count++;
    return true;
  }
  const uint8_t* last() const { return sent[count - 1]; }
};

const Control kControls[] = {{1, DETENT_KIND_BUTTON, 0}, {2, DETENT_KIND_BUTTON, 0}, {9, DETENT_KIND_ENCODER, 0}};
const detent::LinkConfig kConfig = {0, 1, 2, kControls, 3};

FakeTransport* tx;
Link* lnk;

uint8_t expected[DETENT_REPORT_LEN];

const uint8_t* hello(uint8_t seq, uint8_t mode) {
  detent::pack_hello(expected, seq, 0, 1, 2, mode, kControls, 3);
  return expected;
}

const uint8_t* heartbeat(uint8_t seq, uint32_t uptime_s, uint8_t mode) {
  detent::pack_heartbeat(expected, seq, uptime_s, mode);
  return expected;
}

// An app to device report built with fields; mirrors the Rust codec's layout.
void app_report(uint8_t* r, uint8_t type, const uint8_t* payload, uint8_t len) {
  memset(r, 0, DETENT_REPORT_LEN);
  r[0] = DETENT_PROTOCOL_VERSION;
  r[1] = type;
  r[3] = len;
  if (len) memcpy(r + 4, payload, len);
}

ParseError deliver(const uint8_t* r, uint32_t now) {
  AppMessage m;
  return lnk->on_report(r, DETENT_REPORT_LEN, now, &m);
}

}  // namespace

void setUp() {
  tx = new FakeTransport();
  lnk = new Link(kConfig, *tx);
}

void tearDown() {
  delete lnk;
  delete tx;
}

static void test_silent_until_mounted() {
  lnk->tick(10000);
  uint8_t r[DETENT_REPORT_LEN];
  app_report(r, DETENT_MSG_REQUEST_HELLO, nullptr, 0);
  deliver(r, 10000);
  TEST_ASSERT_EQUAL_size_t(0, tx->count);
}

static void test_hello_on_mount() {
  lnk->on_mount(500);
  TEST_ASSERT_EQUAL_size_t(1, tx->count);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(hello(0, DETENT_ENCODER_MODE_VOLUME), tx->last(), DETENT_REPORT_LEN);
}

static void test_hello_on_request_hello() {
  lnk->on_mount(0);
  uint8_t r[DETENT_REPORT_LEN];
  app_report(r, DETENT_MSG_REQUEST_HELLO, nullptr, 0);
  TEST_ASSERT_EQUAL(ParseError::None, deliver(r, 100));
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(hello(1, DETENT_ENCODER_MODE_VOLUME), tx->last(), DETENT_REPORT_LEN);
}

static void test_hello_on_setup_exit() {
  lnk->on_mount(0);
  lnk->on_setup_exit(50);
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  TEST_ASSERT_EQUAL_UINT8(DETENT_MSG_HELLO, tx->last()[1]);
}

static void test_heartbeat_every_two_seconds_from_mount() {
  lnk->on_mount(1000);  // HELLO, seq 0
  lnk->tick(2999);
  TEST_ASSERT_EQUAL_size_t(1, tx->count);
  lnk->tick(3000);
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(heartbeat(1, 3, DETENT_ENCODER_MODE_VOLUME), tx->last(), DETENT_REPORT_LEN);
  // Next one is 2 s after the last, even if a tick came late.
  lnk->tick(3500);
  lnk->tick(4999);
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  lnk->tick(5000);
  TEST_ASSERT_EQUAL_size_t(3, tx->count);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(heartbeat(2, 5, DETENT_ENCODER_MODE_VOLUME), tx->last(), DETENT_REPORT_LEN);
}

static void test_late_tick_sends_one_heartbeat_not_a_burst() {
  lnk->on_mount(0);
  lnk->tick(9000);  // 4.5 intervals late
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  lnk->tick(9001);
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  lnk->tick(11000);
  TEST_ASSERT_EQUAL_size_t(3, tx->count);
}

static void test_heartbeat_survives_millis_wrap() {
  const uint32_t near_wrap = 0xFFFFFFFFu - 1000;
  lnk->on_mount(near_wrap);
  lnk->tick(near_wrap + 2000);  // wraps past zero
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  TEST_ASSERT_EQUAL_UINT8(DETENT_MSG_HEARTBEAT, tx->last()[1]);
}

static void test_mode_shows_in_hello_and_heartbeat() {
  lnk->set_encoder_mode(DETENT_ENCODER_MODE_MAPPED);
  lnk->on_mount(0);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(hello(0, DETENT_ENCODER_MODE_MAPPED), tx->last(), DETENT_REPORT_LEN);
  lnk->tick(2000);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(heartbeat(1, 2, DETENT_ENCODER_MODE_MAPPED), tx->last(), DETENT_REPORT_LEN);
}

static void test_seq_wraps_at_255() {
  lnk->on_mount(0);  // seq 0
  uint8_t r[DETENT_REPORT_LEN];
  app_report(r, DETENT_MSG_REQUEST_HELLO, nullptr, 0);
  for (int i = 0; i < 256; i++) deliver(r, 0);  // seqs 1..255, then 0
  TEST_ASSERT_EQUAL_size_t(257, tx->count);
  TEST_ASSERT_EQUAL_UINT8(255, tx->sent[255][2]);
  TEST_ASSERT_EQUAL_UINT8(0, tx->sent[256][2]);
}

static void test_pairing_follows_hello_ack_and_clears_on_unmount() {
  lnk->on_mount(0);
  TEST_ASSERT_FALSE(lnk->paired());
  uint8_t r[DETENT_REPORT_LEN];
  const uint8_t yes = 1, no = 0;
  app_report(r, DETENT_MSG_HELLO_ACK, &yes, 1);
  TEST_ASSERT_EQUAL(ParseError::None, deliver(r, 10));
  TEST_ASSERT_TRUE(lnk->paired());
  app_report(r, DETENT_MSG_HELLO_ACK, &no, 1);
  deliver(r, 20);
  TEST_ASSERT_FALSE(lnk->paired());
  app_report(r, DETENT_MSG_HELLO_ACK, &yes, 1);
  deliver(r, 30);
  lnk->on_unmount();
  TEST_ASSERT_FALSE(lnk->paired());
  TEST_ASSERT_FALSE(lnk->mounted());
}

static void test_no_heartbeat_after_unmount_and_hello_again_on_remount() {
  lnk->on_mount(0);
  lnk->on_unmount();
  lnk->tick(10000);
  TEST_ASSERT_EQUAL_size_t(1, tx->count);
  lnk->on_mount(20000);
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  TEST_ASSERT_EQUAL_UINT8(DETENT_MSG_HELLO, tx->last()[1]);
  // Heartbeat clock restarts at the new mount.
  lnk->tick(21999);
  TEST_ASSERT_EQUAL_size_t(2, tx->count);
  lnk->tick(22000);
  TEST_ASSERT_EQUAL_size_t(3, tx->count);
}

static void test_malformed_reports_are_counted_and_dropped() {
  lnk->on_mount(0);
  for (size_t i = 0; i < DETENT_VEC_REJECT_COUNT; i++) {
    TEST_ASSERT_NOT_EQUAL(ParseError::None, deliver(DETENT_VEC_REJECTS[i].bytes, 0));
  }
  TEST_ASSERT_EQUAL_UINT32(DETENT_VEC_REJECT_COUNT, lnk->rejected());
  TEST_ASSERT_EQUAL_size_t(1, tx->count);
}

static void test_other_messages_are_handed_back() {
  lnk->on_mount(0);
  AppMessage m;
  TEST_ASSERT_EQUAL(ParseError::None, lnk->on_report(DETENT_VEC_PARSE_MENU_ITEM, DETENT_REPORT_LEN, 0, &m));
  TEST_ASSERT_EQUAL_UINT8(DETENT_MSG_MENU, m.type);
  TEST_ASSERT_EQUAL_STRING("Mail", m.label);
  TEST_ASSERT_EQUAL_size_t(1, tx->count);
}

static void test_input_events_go_out_with_the_shared_seq() {
  lnk->on_mount(0);  // HELLO, seq 0
  TEST_ASSERT_TRUE(lnk->send_button(4, DETENT_ACTION_DOWN));
  detent::pack_button(expected, 1, 4, DETENT_ACTION_DOWN);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, tx->last(), DETENT_REPORT_LEN);
  TEST_ASSERT_TRUE(lnk->send_encoder(5, -2));
  detent::pack_encoder(expected, 2, 5, -2);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, tx->last(), DETENT_REPORT_LEN);
}

static void test_input_events_refused_when_invalid_or_unmounted() {
  TEST_ASSERT_FALSE(lnk->send_button(4, DETENT_ACTION_DOWN));
  lnk->on_mount(0);
  TEST_ASSERT_FALSE(lnk->send_button(4, 9));
  TEST_ASSERT_FALSE(lnk->send_encoder(5, 0));
  TEST_ASSERT_EQUAL_size_t(1, tx->count);
}


static void test_menu_select_goes_out_with_the_shared_seq_when_mounted() {
  TEST_ASSERT_FALSE(lnk->send_menu_select(10));
  lnk->on_mount(0);  // HELLO, seq 0
  TEST_ASSERT_TRUE(lnk->send_menu_select(10));
  detent::pack_menu_select(expected, 1, 10);
  TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, tx->last(), DETENT_REPORT_LEN);
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_menu_select_goes_out_with_the_shared_seq_when_mounted);
  RUN_TEST(test_silent_until_mounted);
  RUN_TEST(test_hello_on_mount);
  RUN_TEST(test_hello_on_request_hello);
  RUN_TEST(test_hello_on_setup_exit);
  RUN_TEST(test_heartbeat_every_two_seconds_from_mount);
  RUN_TEST(test_late_tick_sends_one_heartbeat_not_a_burst);
  RUN_TEST(test_heartbeat_survives_millis_wrap);
  RUN_TEST(test_mode_shows_in_hello_and_heartbeat);
  RUN_TEST(test_seq_wraps_at_255);
  RUN_TEST(test_pairing_follows_hello_ack_and_clears_on_unmount);
  RUN_TEST(test_no_heartbeat_after_unmount_and_hello_again_on_remount);
  RUN_TEST(test_malformed_reports_are_counted_and_dropped);
  RUN_TEST(test_other_messages_are_handed_back);
  RUN_TEST(test_input_events_go_out_with_the_shared_seq);
  RUN_TEST(test_input_events_refused_when_invalid_or_unmounted);
  return UNITY_END();
}
