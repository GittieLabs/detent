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

// Presses and releases one Consumer Control usage (volume, mute).
void consumer_tap(uint16_t usage);

// The USB serial number string (12 hex chars).
const char* serial();

// CDC serial, for logs.
Print& log();

}  // namespace usb
}  // namespace detent

#endif  // DETENT_USB_DEVICE_H
