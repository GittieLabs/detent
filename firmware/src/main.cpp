// Detent firmware.
//
// usb_device owns the composite USB device; lib/link_layer owns the
// conversation (HELLO, HEARTBEAT, REQUEST_HELLO, HELLO_ACK). This file wires
// them together. Input, encoder modes, and the OLED arrive in later M1 tasks.

#include <Arduino.h>

#include "detent_protocol.h"
#include "link_layer.h"
#include "report.h"
#include "usb_device.h"
#include "version.h"

namespace {

// PLACEHOLDER: what HELLO reports until Keith's wiring is known (open question
// in docs/roadmap.md); the input task replaces this with board_pins.h.
const detent::Control kControls[] = {
    {1, DETENT_KIND_BUTTON},
    {2, DETENT_KIND_BUTTON},
    {3, DETENT_KIND_BUTTON},
    {4, DETENT_KIND_BUTTON},
    {5, DETENT_KIND_ENCODER},
};

const detent::LinkConfig kLinkConfig = {
    DETENT_FW_MAJOR, DETENT_FW_MINOR, DETENT_FW_PATCH,
    kControls, sizeof kControls / sizeof kControls[0],
};

struct UsbTransport : detent::LinkTransport {
  bool send(const uint8_t report[DETENT_REPORT_LEN]) override {
    return detent::usb::send_report(report);
  }
};

UsbTransport transport;
detent::Link device_link(kLinkConfig, transport);

// XIAO ESP32-S3 user LED (GPIO21) is active low. Off: no host. Blinking:
// mounted, not paired. Solid: paired.
const uint8_t LED_PIN = LED_BUILTIN;
const uint8_t LED_ON = LOW;
const uint8_t LED_OFF = HIGH;

void update_led(uint32_t now) {
  bool on = false;
  if (device_link.paired()) {
    on = true;
  } else if (device_link.mounted()) {
    on = (now / 500) % 2 == 0;
  }
  digitalWrite(LED_PIN, on ? LED_ON : LED_OFF);
}

void handle_report(const uint8_t* report, uint32_t now) {
  detent::AppMessage msg;
  const detent::ParseError err = device_link.on_report(report, DETENT_REPORT_LEN, now, &msg);
  if (err != detent::ParseError::None) {
    detent::usb::log().printf("link: rejected report (%s), type 0x%02X\n",
                              detent::parse_error_name(err), report[1]);
    return;
  }
  switch (msg.type) {
    case DETENT_MSG_REQUEST_HELLO:
    case DETENT_MSG_HELLO_ACK:
      break;  // handled by the link
    default:
      // MENU, STATUS, TEST_INJECT: later M1 tasks.
      detent::usb::log().printf("link: type 0x%02X not handled yet\n", msg.type);
      break;
  }
}

}  // namespace

void setup() {
  pinMode(LED_PIN, OUTPUT);
  digitalWrite(LED_PIN, LED_OFF);
  detent::usb::begin();
}

void loop() {
  const uint32_t now = millis();

  bool mounted;
  while (detent::usb::poll_connection(&mounted)) {
    if (mounted) {
      device_link.on_mount(now);
    } else {
      device_link.on_unmount();
    }
  }

  uint8_t report[DETENT_REPORT_LEN];
  while (detent::usb::poll_report(report)) {
    handle_report(report, now);
  }

  device_link.tick(now);
  update_led(now);
}
