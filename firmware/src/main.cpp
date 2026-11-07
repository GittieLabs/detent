// Detent firmware.
//
// USB identity and the composite device live in usb_device. The loop below is
// TEMPORARY, kept from the M1 HID layout spike so the channel can be tested
// on hardware until the link layer task replaces it with HELLO, HEARTBEAT,
// REQUEST_HELLO, and HELLO_ACK. Spike commands (byte 0 of a vendor report):
//   'E'  echo the report back
//   'U'  Consumer Control volume up
//   'W'  Consumer Control volume down
// Every second the device sends 'D', <counter>.

#include <Arduino.h>
#include <USBHIDConsumerControl.h>
#include <string.h>

#include "detent_protocol.h"
#include "usb_device.h"
// Not used yet; included so the ESP32 build compiles the codec.
#include "report.h"

// XIAO ESP32-S3 user LED (GPIO21) is active low.
static const uint8_t LED_PIN = LED_BUILTIN;
static const uint8_t LED_ON = LOW;
static const uint8_t LED_OFF = HIGH;

static void handle_spike_command(const uint8_t* report) {
  detent::usb::log().printf("spike: command 0x%02X\n", report[0]);
  switch (report[0]) {
    case 'E':
      detent::usb::send_report(report);
      break;
    case 'U':
      detent::usb::consumer_tap(CONSUMER_CONTROL_VOLUME_INCREMENT);
      break;
    case 'W':
      detent::usb::consumer_tap(CONSUMER_CONTROL_VOLUME_DECREMENT);
      break;
    default:
      break;
  }
}

void setup() {
  pinMode(LED_PIN, OUTPUT);
  digitalWrite(LED_PIN, LED_OFF);
  detent::usb::begin();
}

void loop() {
  static uint32_t last = 0;
  static uint8_t counter = 0;
  static bool on = false;

  uint8_t report[DETENT_REPORT_LEN];
  while (detent::usb::poll_report(report)) {
    handle_spike_command(report);
  }

  const uint32_t now = millis();
  if (now - last >= 1000) {
    last = now;
    on = !on;
    digitalWrite(LED_PIN, on ? LED_ON : LED_OFF);
    uint8_t beat[DETENT_REPORT_LEN] = {'D', counter++};
    detent::usb::send_report(beat);
  }
}
