#include "usb_device.h"

#include <USB.h>
#include <USBHIDConsumerControl.h>
#include <USBHIDVendor.h>
#include <esp_system.h>
#include <freertos/queue.h>
#include <string.h>

#include "usb_identity.h"

#if ARDUINO_USB_MODE != 0
#error "Detent needs TinyUSB on the OTG peripheral (ARDUINO_USB_MODE=0)"
#endif

// The protocol's report IDs are the ones this core hardcodes (D-036).
static_assert(HID_REPORT_ID_VENDOR == DETENT_VENDOR_REPORT_ID,
              "core vendor report ID drifted from the protocol");
static_assert(HID_REPORT_ID_CONSUMER_CONTROL == DETENT_CONSUMER_REPORT_ID,
              "core consumer control report ID drifted from the protocol");

namespace detent {
namespace usb {
namespace {

// Declaration order is descriptor order: vendor first, so it is the HID
// interface's first top level collection.
USBHIDVendor vendor(DETENT_REPORT_LEN, false);
USBHIDConsumerControl consumer;
USBCDC cdc;

char serial_number[kSerialBufLen];

// Output reports from the host, handed from the USB event task to loop().
const UBaseType_t kRxDepth = 4;
QueueHandle_t rx_queue = nullptr;

// Mount and unmount events, in order.
const UBaseType_t kConnDepth = 8;
QueueHandle_t conn_queue = nullptr;

void on_usb_event(void*, esp_event_base_t, int32_t id, void*) {
  if (conn_queue == nullptr) return;
  uint8_t mounted;
  if (id == ARDUINO_USB_STARTED_EVENT) {
    mounted = 1;
  } else if (id == ARDUINO_USB_STOPPED_EVENT) {
    mounted = 0;
  } else {
    return;
  }
  xQueueSend(conn_queue, &mounted, 0);
}

void on_vendor_event(void*, esp_event_base_t, int32_t id, void* data) {
  if (id != ARDUINO_USB_HID_VENDOR_OUTPUT_EVENT || rx_queue == nullptr) {
    return;
  }
  const auto* ev = static_cast<arduino_usb_hid_vendor_event_data_t*>(data);
  uint8_t report[DETENT_REPORT_LEN] = {0};
  memcpy(report, ev->buffer, ev->len < DETENT_REPORT_LEN ? ev->len : DETENT_REPORT_LEN);
  // Drop when full; the host retries (REQUEST_HELLO) and seq exposes gaps.
  xQueueSend(rx_queue, report, 0);
}

}  // namespace

void begin() {
  uint8_t mac[6];
  esp_efuse_mac_get_default(mac);
  format_serial(mac, serial_number);

  USB.VID(DETENT_USB_VID);
  USB.PID(DETENT_USB_PID);  // placeholder until the usb-pids allocation (D-014)
  USB.manufacturerName(kManufacturer);
  USB.productName(kProductName);
  USB.serialNumber(serial_number);

  rx_queue = xQueueCreate(kRxDepth, DETENT_REPORT_LEN);
  conn_queue = xQueueCreate(kConnDepth, sizeof(uint8_t));
  USB.onEvent(on_usb_event);
  vendor.onEvent(on_vendor_event);
  vendor.begin();
  consumer.begin();
  // CDC stays so uploads can reset into the bootloader (1200 baud touch).
  cdc.begin();
  USB.begin();
}

bool send_report(const uint8_t report[DETENT_REPORT_LEN]) {
  return vendor.write(report, DETENT_REPORT_LEN) == DETENT_REPORT_LEN;
}

bool poll_report(uint8_t report[DETENT_REPORT_LEN]) {
  return rx_queue != nullptr && xQueueReceive(rx_queue, report, 0) == pdTRUE;
}

bool poll_connection(bool* mounted) {
  uint8_t m;
  if (conn_queue == nullptr || xQueueReceive(conn_queue, &m, 0) != pdTRUE) return false;
  *mounted = m != 0;
  return true;
}

void consumer_tap(uint16_t usage) {
  consumer.press(usage);
  consumer.release();
}

const char* serial() { return serial_number; }

Print& log() { return cdc; }

}  // namespace usb
}  // namespace detent
