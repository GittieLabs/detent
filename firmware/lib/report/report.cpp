#include "report.h"

#include <string.h>

namespace detent {
namespace {

const size_t PAYLOAD_OFFSET = DETENT_HEADER_LEN;
const size_t HELLO_FIXED = 5;
const size_t MENU_LEN = 4 + DETENT_MAX_LABEL_LEN;
const size_t STATUS_LEN = 1 + DETENT_MAX_STATUS_LEN;
const size_t INJECT_EVENT_LEN = 5;

bool valid_mode(uint8_t m) {
  return m == DETENT_ENCODER_MODE_VOLUME || m == DETENT_ENCODER_MODE_MENU ||
         m == DETENT_ENCODER_MODE_MAPPED;
}

bool valid_kind(uint8_t k) {
  return k == DETENT_KIND_BUTTON || k == DETENT_KIND_ENCODER || k == DETENT_KIND_ENCODER_WITH_PUSH;
}

size_t entry_len(const Control& c) { return c.kind == DETENT_KIND_ENCODER_WITH_PUSH ? 3 : 2; }

bool valid_action(uint8_t a) {
  return a == DETENT_ACTION_DOWN || a == DETENT_ACTION_UP || a == DETENT_ACTION_LONG_PRESS;
}

bool printable(uint8_t b) { return b >= 0x20 && b <= 0x7E; }

// Zeroes `out` and writes the header. Payload goes at out + PAYLOAD_OFFSET.
uint8_t* begin(uint8_t* out, uint8_t type, uint8_t seq, uint8_t len) {
  memset(out, 0, DETENT_REPORT_LEN);
  out[0] = DETENT_PROTOCOL_VERSION;
  out[1] = type;
  out[2] = seq;
  out[3] = len;
  return out + PAYLOAD_OFFSET;
}

bool all_zero(const uint8_t* p, size_t n) {
  for (size_t i = 0; i < n; i++) {
    if (p[i] != 0) return false;
  }
  return true;
}

// Fixed width text field: `len` printable bytes, the rest zero. Same checks,
// same order as Rust's decode_text.
ParseError read_text(uint8_t len, const uint8_t* field, size_t width, size_t min, char* dst) {
  if (len < min || len > width) return ParseError::InvalidField;
  if (!all_zero(field + len, width - len)) return ParseError::TrailingBytes;
  for (size_t i = 0; i < len; i++) {
    if (!printable(field[i])) return ParseError::InvalidField;
  }
  memcpy(dst, field, len);
  dst[len] = '\0';
  return ParseError::None;
}

ParseError parse_menu(const uint8_t* p, size_t n, AppMessage* m) {
  if (n != MENU_LEN) return ParseError::PayloadSize;
  m->menu_total = p[0];
  m->menu_index = p[1];
  m->menu_item = p[2];
  const uint8_t label_len = p[3];
  if (m->menu_total == 0) {
    if (m->menu_index != 0 || m->menu_item != 0 || label_len != 0) return ParseError::InvalidField;
    if (!all_zero(p + 4, DETENT_MAX_LABEL_LEN)) return ParseError::TrailingBytes;
    m->label[0] = '\0';
    return ParseError::None;
  }
  if (m->menu_index >= m->menu_total) return ParseError::InvalidField;
  return read_text(label_len, p + 4, DETENT_MAX_LABEL_LEN, 1, m->label);
}

ParseError parse_inject(const uint8_t* p, size_t n, AppMessage* m) {
  if (n == 0) return ParseError::PayloadSize;
  const uint8_t count = p[0];
  if (count > DETENT_MAX_INJECT_EVENTS) return ParseError::TooManyEvents;
  if (count == 0) return ParseError::InvalidField;
  if (n != 1 + INJECT_EVENT_LEN * count) return ParseError::PayloadSize;
  for (uint8_t i = 0; i < count; i++) {
    const uint8_t* e = p + 1 + INJECT_EVENT_LEN * i;
    InjectEvent& ev = m->inject[i];
    ev.type = e[0];
    ev.control = e[1];
    ev.value = e[2];
    ev.delay_ms = static_cast<uint16_t>(e[3] | (e[4] << 8));
    if (ev.type == DETENT_MSG_BUTTON) {
      if (!valid_action(ev.value)) return ParseError::InvalidField;
    } else if (ev.type == DETENT_MSG_ENCODER) {
      if (ev.value == 0) return ParseError::InvalidField;
    } else {
      return ParseError::InvalidField;
    }
  }
  m->inject_count = count;
  return ParseError::None;
}

}  // namespace

bool pack_hello(uint8_t* out, uint8_t seq, uint8_t fw_major, uint8_t fw_minor, uint8_t fw_patch,
                uint8_t encoder_mode, const Control* controls, uint8_t count) {
  if (count > DETENT_MAX_CONTROLS || !valid_mode(encoder_mode)) return false;
  if (count > 0 && controls == nullptr) return false;
  bool seen[256] = {false};
  bool is_button[256] = {false};
  size_t len = HELLO_FIXED;
  for (uint8_t i = 0; i < count; i++) {
    if (!valid_kind(controls[i].kind) || seen[controls[i].id]) return false;
    seen[controls[i].id] = true;
    is_button[controls[i].id] = controls[i].kind == DETENT_KIND_BUTTON;
    len += entry_len(controls[i]);
  }
  if (len > DETENT_MAX_PAYLOAD_LEN) return false;
  bool claimed[256] = {false};
  for (uint8_t i = 0; i < count; i++) {
    if (controls[i].kind != DETENT_KIND_ENCODER_WITH_PUSH) continue;
    const uint8_t push = controls[i].push;
    if (!is_button[push] || claimed[push]) return false;
    claimed[push] = true;
  }
  uint8_t* p = begin(out, DETENT_MSG_HELLO, seq, static_cast<uint8_t>(len));
  p[0] = fw_major;
  p[1] = fw_minor;
  p[2] = fw_patch;
  p[3] = encoder_mode;
  p[4] = count;
  size_t at = HELLO_FIXED;
  for (uint8_t i = 0; i < count; i++) {
    p[at++] = controls[i].id;
    p[at++] = controls[i].kind;
    if (controls[i].kind == DETENT_KIND_ENCODER_WITH_PUSH) p[at++] = controls[i].push;
  }
  return true;
}

bool pack_button(uint8_t* out, uint8_t seq, uint8_t control, uint8_t action) {
  if (!valid_action(action)) return false;
  uint8_t* p = begin(out, DETENT_MSG_BUTTON, seq, 2);
  p[0] = control;
  p[1] = action;
  return true;
}

bool pack_encoder(uint8_t* out, uint8_t seq, uint8_t control, int8_t delta) {
  if (delta == 0) return false;
  uint8_t* p = begin(out, DETENT_MSG_ENCODER, seq, 2);
  p[0] = control;
  p[1] = static_cast<uint8_t>(delta);
  return true;
}

void pack_menu_select(uint8_t* out, uint8_t seq, uint8_t item) {
  uint8_t* p = begin(out, DETENT_MSG_MENU_SELECT, seq, 1);
  p[0] = item;
}

bool pack_heartbeat(uint8_t* out, uint8_t seq, uint32_t uptime_s, uint8_t encoder_mode) {
  if (!valid_mode(encoder_mode)) return false;
  uint8_t* p = begin(out, DETENT_MSG_HEARTBEAT, seq, 5);
  p[0] = static_cast<uint8_t>(uptime_s);
  p[1] = static_cast<uint8_t>(uptime_s >> 8);
  p[2] = static_cast<uint8_t>(uptime_s >> 16);
  p[3] = static_cast<uint8_t>(uptime_s >> 24);
  p[4] = encoder_mode;
  return true;
}

const char* parse_error_name(ParseError err) {
  switch (err) {
    case ParseError::None: return "None";
    case ParseError::ReportLength: return "ReportLength";
    case ParseError::Version: return "Version";
    case ParseError::PayloadLength: return "PayloadLength";
    case ParseError::TrailingBytes: return "TrailingBytes";
    case ParseError::UnknownType: return "UnknownType";
    case ParseError::PayloadSize: return "PayloadSize";
    case ParseError::InvalidField: return "InvalidField";
    case ParseError::TooManyEvents: return "TooManyEvents";
  }
  return "Unknown";
}

ParseError parse_app(const uint8_t* report, size_t len, AppMessage* out) {
  // Header checks, same order as Rust's parse_header.
  if (report == nullptr || out == nullptr || len != DETENT_REPORT_LEN) return ParseError::ReportLength;
  if (report[0] != DETENT_PROTOCOL_VERSION) return ParseError::Version;
  const uint8_t plen = report[3];
  if (plen > DETENT_MAX_PAYLOAD_LEN) return ParseError::PayloadLength;
  if (!all_zero(report + PAYLOAD_OFFSET + plen, DETENT_REPORT_LEN - PAYLOAD_OFFSET - plen)) {
    return ParseError::TrailingBytes;
  }

  memset(out, 0, sizeof *out);
  out->type = report[1];
  out->seq = report[2];
  const uint8_t* p = report + PAYLOAD_OFFSET;

  switch (out->type) {
    case DETENT_MSG_HELLO_ACK:
      if (plen != 1) return ParseError::PayloadSize;
      if (p[0] > 1) return ParseError::InvalidField;
      out->paired = p[0] == 1;
      return ParseError::None;
    case DETENT_MSG_MENU:
      return parse_menu(p, plen, out);
    case DETENT_MSG_STATUS:
      if (plen != STATUS_LEN) return ParseError::PayloadSize;
      return read_text(p[0], p + 1, DETENT_MAX_STATUS_LEN, 0, out->status);
    case DETENT_MSG_REQUEST_HELLO:
      return plen == 0 ? ParseError::None : ParseError::PayloadSize;
    case DETENT_MSG_TEST_INJECT:
      return parse_inject(p, plen, out);
    default:
      return ParseError::UnknownType;
  }
}

}  // namespace detent
