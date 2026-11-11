// M7 task 1: the Pico USB spike. Enumerates as Detent's composite device and
// runs the real link layer (HELLO, HEARTBEAT, HELLO_ACK); BOOTSEL is button
// 1, and also taps volume up so the Consumer Control path can be checked.
// The full firmware replaces this in a later M7 task.

#include <Arduino.h>

#include "../usb_device.h"
#include "../version.h"
#include "detent_protocol.h"
#include "link_layer.h"

namespace {

const detent::Control kControls[] = {{1, DETENT_KIND_BUTTON}};
const detent::LinkConfig kConfig = {DETENT_FW_MAJOR, DETENT_FW_MINOR, DETENT_FW_PATCH, kControls, 1};

struct UsbTransport : detent::LinkTransport {
  bool send(const uint8_t report[DETENT_REPORT_LEN]) override {
    return detent::usb::send_report(report);
  }
};

UsbTransport transport;
detent::Link link_(kConfig, transport);
bool bootsel_was = false;

}  // namespace

void setup() {
  pinMode(LED_BUILTIN, OUTPUT);
  detent::usb::begin();
}

void loop() {
  const uint32_t now = millis();
  bool mounted;
  while (detent::usb::poll_connection(&mounted)) {
    if (mounted) {
      link_.on_mount(now);
    } else {
      link_.on_unmount();
    }
  }
  uint8_t report[DETENT_REPORT_LEN];
  while (detent::usb::poll_report(report)) {
    detent::AppMessage msg;
    link_.on_report(report, DETENT_REPORT_LEN, now, &msg);
  }
  link_.tick(now);

  // BOOTSEL: button 1 to the app, and a volume up tap.
  const bool down = BOOTSEL;
  if (down != bootsel_was) {
    bootsel_was = down;
    link_.send_button(1, down ? DETENT_ACTION_DOWN : DETENT_ACTION_UP);
    if (down) detent::usb::consumer_tap(0x00E9);  // Volume Increment
  }

  // LED: off with no host, blinking mounted, solid paired (as on the XIAO).
  const bool on = link_.paired() || (link_.mounted() && (now / 500) % 2 == 0);
  digitalWrite(LED_BUILTIN, on ? HIGH : LOW);
  delay(1);
}
