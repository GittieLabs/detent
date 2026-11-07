#include "link_layer.h"

namespace detent {

Link::Link(const LinkConfig& config, LinkTransport& transport)
    : config_(config), transport_(transport) {}

void Link::send(const uint8_t report[DETENT_REPORT_LEN]) {
  transport_.send(report);
  // A failed send still uses a seq number, so the app sees the gap.
  seq_++;
}

void Link::send_hello() {
  uint8_t r[DETENT_REPORT_LEN];
  if (pack_hello(r, seq_, config_.fw_major, config_.fw_minor, config_.fw_patch, encoder_mode_,
                 config_.controls, config_.control_count)) {
    send(r);
  }
}

void Link::on_mount(uint32_t now_ms) {
  mounted_ = true;
  last_heartbeat_ms_ = now_ms;
  send_hello();
}

void Link::on_unmount() {
  mounted_ = false;
  paired_ = false;
}

ParseError Link::on_report(const uint8_t* report, size_t len, uint32_t now_ms, AppMessage* out) {
  (void)now_ms;
  if (!mounted_) return ParseError::None;
  const ParseError err = parse_app(report, len, out);
  if (err != ParseError::None) {
    rejected_++;
    return err;
  }
  switch (out->type) {
    case DETENT_MSG_REQUEST_HELLO:
      send_hello();
      break;
    case DETENT_MSG_HELLO_ACK:
      paired_ = out->paired;
      break;
    default:
      break;  // the caller handles MENU, STATUS, TEST_INJECT
  }
  return ParseError::None;
}

void Link::on_setup_exit(uint32_t now_ms) {
  (void)now_ms;
  if (mounted_) send_hello();
}

void Link::tick(uint32_t now_ms) {
  // Unsigned subtraction handles millis() wrapping.
  if (!mounted_ || now_ms - last_heartbeat_ms_ < kHeartbeatMs) return;
  last_heartbeat_ms_ = now_ms;
  uint8_t r[DETENT_REPORT_LEN];
  if (pack_heartbeat(r, seq_, now_ms / 1000, encoder_mode_)) send(r);
}

}  // namespace detent
