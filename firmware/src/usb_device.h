// The composite USB device: one HID interface with the vendor channel (report
// ID 6) and Consumer Control (report ID 4), plus CDC for uploads and logs
// (D-036). Owns identity: VID/PID from the generated header, serial from the
// base MAC, strings from lib/usb_identity.

#ifndef DETENT_USB_DEVICE_H
#define DETENT_USB_DEVICE_H

#include <Arduino.h>

#include "detent_protocol.h"

namespace detent {
namespace usb {

// Sets identity and starts USB. Call once from setup().
void begin();

// Sends one DETENT_REPORT_LEN byte report on the vendor channel.
bool send_report(const uint8_t report[DETENT_REPORT_LEN]);

// Takes the oldest output report from the host, if any. Reports arrive on the
// USB event task and wait here for loop(). Returns false when none is queued.
bool poll_report(uint8_t report[DETENT_REPORT_LEN]);

// Takes the oldest host connection change, if any: true when the host
// mounted (configured) the device, false when it unmounted it. Changes arrive
// on the USB event task and wait here for loop(), in order.
bool poll_connection(bool* mounted);

// Consumer Control usages the encoder's Volume mode sends (HID Usage Tables).
const uint16_t kVolumeUp = 0x00E9;
const uint16_t kVolumeDown = 0x00EA;
const uint16_t kMute = 0x00E2;

// Presses and releases one Consumer Control usage (volume, mute).
void consumer_tap(uint16_t usage);

// The USB serial number string (12 hex chars).
const char* serial();

// CDC serial, for logs.
Print& log();

// Receive path counters, for diagnostics.
struct RxStats {
  uint32_t events;   // vendor output events seen
  uint32_t short_;   // events with fewer than DETENT_REPORT_LEN bytes
  uint32_t dropped;  // events dropped because our queue was full
  uint16_t min_len;
};
RxStats rx_stats();

struct TxStats {
  uint32_t attempts;
  uint32_t failures;
  uint32_t max_ms;     // longest single send
  uint32_t first_fail_ms;  // millis() of the first failure, 0 if none
};
TxStats tx_stats();

// Debug timeline: a ring of timestamped events, oldest first.
// kind: 'M' mount, 'U' unmount, 'R' rx event (a = len, b = type byte),
// 'S' send start (b = type byte), 'K' send ok (a = ms), 'F' send fail (a = ms).
struct TimelineEntry {
  uint32_t ms;
  char kind;
  uint16_t a;
  uint8_t b;
};
const int kTimelineLen = 48;
int timeline(TimelineEntry* out);  // returns the count copied
void timeline_note(char kind, uint16_t a, uint8_t b);

}  // namespace usb
}  // namespace detent

#endif  // DETENT_USB_DEVICE_H
