// Detent firmware, M0: bring up TinyUSB with Detent's USB identity and blink.
// The vendor HID and Consumer Control interfaces arrive in M1.

#include <Arduino.h>
#include <USB.h>

#if ARDUINO_USB_MODE != 0
#error "Detent needs TinyUSB on the OTG peripheral (ARDUINO_USB_MODE=0)"
#endif

// TEMPORARY copy of protocol/src/ids.rs. Moves to the generated
// include/detent_protocol.h in the gen-header task. PID is a placeholder
// until the Espressif usb-pids allocation (D-014, D-026).
static const uint16_t DETENT_VID = 0x303A;
static const uint16_t DETENT_PID = 0x8FDE;

// XIAO ESP32-S3 user LED (GPIO21) is active low.
static const uint8_t LED_PIN = LED_BUILTIN;
static const uint8_t LED_ON = LOW;
static const uint8_t LED_OFF = HIGH;
static const uint32_t BLINK_MS = 500;

USBCDC SerialUSB;

void setup() {
  pinMode(LED_PIN, OUTPUT);
  digitalWrite(LED_PIN, LED_OFF);

  USB.VID(DETENT_VID);
  USB.PID(DETENT_PID);
  USB.manufacturerName("GittieLabs");
  USB.productName("Detent Dev");  // PLACEHOLDER: "Detent <model name>"
  // CDC stays in M0 so `pio run -t upload` can reset into the bootloader
  // without holding BOOT.
  SerialUSB.begin();
  USB.begin();
}

void loop() {
  static uint32_t last = 0;
  static bool on = false;
  const uint32_t now = millis();
  if (now - last >= BLINK_MS) {
    last = now;
    on = !on;
    digitalWrite(LED_PIN, on ? LED_ON : LED_OFF);
  }
}
