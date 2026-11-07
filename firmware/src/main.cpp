// Detent firmware, M1 spike: HID layout on Arduino-ESP32 2.0.17 (option A).
//
// THROWAWAY. The precompiled TinyUSB allows one HID interface, so the vendor
// channel and Consumer Control share it, told apart by the core's fixed report
// IDs (vendor 6, consumer control 4). The spike answers whether macOS lets a
// plain process open the vendor collection and whether media keys keep working
// while it's open. The real firmware replaces this file in later M1 tasks.
//
// Vendor reports are 63 bytes after the report ID. Commands from the host
// (byte 0 of an output report):
//   'E'  echo the whole report back as an input report
//   'U'  Consumer Control volume up (press and release)
//   'W'  Consumer Control volume down
// Every second the device sends an input report 'D', <counter>.

#include <Arduino.h>
#include <USB.h>
#include <USBHIDConsumerControl.h>
#include <USBHIDVendor.h>

// Generated from the protocol crate by scripts/gen-header.sh. Never edit it.
#include "detent_protocol.h"
// Not used by the spike yet; included so the ESP32 build compiles the codec.
#include "report.h"

#if ARDUINO_USB_MODE != 0
#error "Detent needs TinyUSB on the OTG peripheral (ARDUINO_USB_MODE=0)"
#endif

// The protocol's report IDs are the ones this core hardcodes (D-036).
static_assert(HID_REPORT_ID_VENDOR == DETENT_VENDOR_REPORT_ID,
              "core vendor report ID drifted from the protocol");
static_assert(HID_REPORT_ID_CONSUMER_CONTROL == DETENT_CONSUMER_REPORT_ID,
              "core consumer control report ID drifted from the protocol");

// XIAO ESP32-S3 user LED (GPIO21) is active low.
static const uint8_t LED_PIN = LED_BUILTIN;
static const uint8_t LED_ON = LOW;
static const uint8_t LED_OFF = HIGH;

// Declaration order is descriptor order: vendor first, so it is the
// interface's first top level collection.
USBHIDVendor Vendor(DETENT_REPORT_LEN, false);
USBHIDConsumerControl Consumer;
USBCDC SerialUSB;

// Filled by the HID event task, drained in loop().
static volatile bool pending = false;
static uint8_t pending_buf[DETENT_REPORT_LEN];
static uint16_t pending_len = 0;

static void onVendorEvent(void*, esp_event_base_t, int32_t id, void* data) {
  if (id != ARDUINO_USB_HID_VENDOR_OUTPUT_EVENT || pending) {
    return;
  }
  const auto* ev = static_cast<arduino_usb_hid_vendor_event_data_t*>(data);
  pending_len = ev->len < DETENT_REPORT_LEN ? ev->len : DETENT_REPORT_LEN;
  memcpy(pending_buf, ev->buffer, pending_len);
  pending = true;
}

static void sendReport(const uint8_t* bytes, size_t len) {
  uint8_t report[DETENT_REPORT_LEN] = {0};
  memcpy(report, bytes, len < DETENT_REPORT_LEN ? len : DETENT_REPORT_LEN);
  Vendor.write(report, DETENT_REPORT_LEN);
}

static void handleCommand() {
  const uint8_t cmd = pending_len ? pending_buf[0] : 0;
  SerialUSB.printf("spike: output report len=%u cmd=0x%02X\n", pending_len, cmd);
  switch (cmd) {
    case 'E':
      sendReport(pending_buf, pending_len);
      break;
    case 'U':
      Consumer.press(CONSUMER_CONTROL_VOLUME_INCREMENT);
      Consumer.release();
      break;
    case 'W':
      Consumer.press(CONSUMER_CONTROL_VOLUME_DECREMENT);
      Consumer.release();
      break;
    default:
      break;
  }
  pending = false;
}

void setup() {
  pinMode(LED_PIN, OUTPUT);
  digitalWrite(LED_PIN, LED_OFF);

  USB.VID(DETENT_USB_VID);
  USB.PID(DETENT_USB_PID);  // placeholder until the usb-pids allocation (D-014)
  USB.manufacturerName("GittieLabs");
  USB.productName("Detent Dev");  // PLACEHOLDER: "Detent <model name>"

  Vendor.onEvent(onVendorEvent);
  Vendor.begin();
  Consumer.begin();
  // CDC stays so `pio run -t upload` can reset into the bootloader.
  SerialUSB.begin();
  USB.begin();
}

void loop() {
  static uint32_t last = 0;
  static uint8_t counter = 0;
  static bool on = false;

  if (pending) {
    handleCommand();
  }

  const uint32_t now = millis();
  if (now - last >= 1000) {
    last = now;
    on = !on;
    digitalWrite(LED_PIN, on ? LED_ON : LED_OFF);
    const uint8_t beat[2] = {'D', counter++};
    sendReport(beat, sizeof beat);
  }
}
