// Detent report codec, firmware side. Mirrors protocol::codec in Rust and is
// tested byte for byte against its golden vectors (firmware/test/vectors.h).
//
// Every buffer is the DETENT_REPORT_LEN (63) byte report that follows the HID
// report ID. The firmware packs device to app messages and parses app to
// device messages. Parsing is strict (D-037) and checks rules in the same
// order as Rust, so both reject a malformed report with the same error kind.
//
// Pure C++11, no heap, no Arduino: builds for the ESP32 and the native tests.

#ifndef DETENT_REPORT_H
#define DETENT_REPORT_H

#include <stddef.h>
#include <stdint.h>

#include "detent_protocol.h"

namespace detent {

struct Control {
  uint8_t id;
  uint8_t kind;  // DETENT_KIND_*
};

// Pack functions fill `out` (DETENT_REPORT_LEN bytes) and return true, or
// return false, leaving `out` unspecified, for a message the app would reject.

// controls may be null when count is 0. IDs must be unique.
bool pack_hello(uint8_t* out, uint8_t seq, uint8_t fw_major, uint8_t fw_minor, uint8_t fw_patch,
                uint8_t encoder_mode, const Control* controls, uint8_t count);
bool pack_button(uint8_t* out, uint8_t seq, uint8_t control, uint8_t action);
// delta must be nonzero; positive is clockwise.
bool pack_encoder(uint8_t* out, uint8_t seq, uint8_t control, int8_t delta);
void pack_menu_select(uint8_t* out, uint8_t seq, uint8_t item);
bool pack_heartbeat(uint8_t* out, uint8_t seq, uint32_t uptime_s, uint8_t encoder_mode);

// Names match the Rust DecodeError variants; None means success.
enum class ParseError : uint8_t {
  None,
  ReportLength,
  Version,
  PayloadLength,
  TrailingBytes,
  UnknownType,
  PayloadSize,
  InvalidField,
  TooManyEvents,
};

const char* parse_error_name(ParseError err);

struct InjectEvent {
  uint8_t type;   // DETENT_MSG_BUTTON or DETENT_MSG_ENCODER
  uint8_t control;
  uint8_t value;  // button action, or encoder delta as int8_t
  uint16_t delay_ms;
};

// A parsed app to device message. Only the fields for `type` are meaningful.
struct AppMessage {
  uint8_t type;  // DETENT_MSG_*
  uint8_t seq;
  // HELLO_ACK
  bool paired;
  // MENU. menu_total == 0 means clear the app owned items.
  uint8_t menu_total;
  uint8_t menu_index;
  uint8_t menu_item;
  char label[DETENT_MAX_LABEL_LEN + 1];  // NUL terminated
  // STATUS
  char status[DETENT_MAX_STATUS_LEN + 1];  // NUL terminated
  // TEST_INJECT
  uint8_t inject_count;
  InjectEvent inject[DETENT_MAX_INJECT_EVENTS];
};

// Parses an app to device report. On ParseError::None, `out` holds the
// message; otherwise its contents are unspecified.
ParseError parse_app(const uint8_t* report, size_t len, AppMessage* out);

}  // namespace detent

#endif  // DETENT_REPORT_H
