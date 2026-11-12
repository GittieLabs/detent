#include "../usb_device.h"

#include <USB.h>
#include <USBHIDConsumerControl.h>
#include <USBHIDVendor.h>
#include <esp_system.h>
#if __has_include(<esp_mac.h>)
#include <esp_mac.h>  // ESP-IDF 5: esp_efuse_mac_get_default moved here
#endif
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
void timeline_note(char kind, uint16_t a, uint8_t b);
namespace {

// Declaration order is descriptor order: vendor first, so it is the HID
// interface's first top level collection.
USBHIDVendor vendor(DETENT_REPORT_LEN, false);
USBHIDConsumerControl consumer;
USBCDC cdc;

char serial_number[kSerialBufLen];
RxStats stats = {0, 0, 0, 0xFFFF};
TxStats tx = {0, 0, 0, 0};

TimelineEntry ring[kTimelineLen];
int ring_next = 0;   // guarded by ring_mux
int ring_count = 0;
portMUX_TYPE ring_mux = portMUX_INITIALIZER_UNLOCKED;

// Output reports are read from USBHIDVendor's own byte queue, which the USB
// task fills synchronously: whole 63 byte reports, no shared buffers. The
// output event (copied pointer into TinyUSB's receive buffer, which TinyUSB
// may reuse before the event is handled) is used only to watch lengths. If
// the core ever passes on a short report, its queue overflowed and the byte
// stream lost framing, so loop() drops everything queued and starts clean.
const size_t kRxReports = 8;
volatile bool resync = false;

// Mount and unmount events, in order.
const UBaseType_t kConnDepth = 8;
QueueHandle_t conn_queue = nullptr;

void on_usb_event(void*, esp_event_base_t, int32_t id, void*) {
  if (id == ARDUINO_USB_STARTED_EVENT) timeline_note('M', 0, 0);
  if (id == ARDUINO_USB_STOPPED_EVENT) timeline_note('U', 0, 0);
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
  if (id != ARDUINO_USB_HID_VENDOR_OUTPUT_EVENT) return;
  const auto* ev = static_cast<arduino_usb_hid_vendor_event_data_t*>(data);
  timeline_note('R', ev->len, 0);
  stats.events++;
  if (ev->len < stats.min_len) stats.min_len = ev->len;
  if (ev->len < DETENT_REPORT_LEN) {
    stats.short_++;
    resync = true;
  }
}

}  // namespace

void begin() {
  uint8_t mac[6];
  esp_efuse_mac_get_default(mac);
  format_serial(mac, serial_number);

  USB.VID(DETENT_USB_VID);
  USB.PID(DETENT_USB_PID);  // this board's local PID (D-112)
  USB.manufacturerName(kManufacturer);
  USB.productName(kProductName);
  USB.serialNumber(serial_number);

  // Before begin(), which otherwise sizes the queue at 256 bytes.
  vendor.setRxBufferSize(kRxReports * DETENT_REPORT_LEN);
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
  const uint32_t start = millis();
  timeline_note('S', 0, report[1]);
  const bool ok = vendor.write(report, DETENT_REPORT_LEN) == DETENT_REPORT_LEN;
  const uint32_t took = millis() - start;
  timeline_note(ok ? 'K' : 'F', static_cast<uint16_t>(took), report[1]);
  tx.attempts++;
  if (took > tx.max_ms) tx.max_ms = took;
  if (!ok) {
    tx.failures++;
    if (tx.first_fail_ms == 0) tx.first_fail_ms = start ? start : 1;
  }
  return ok;
}

bool poll_report(uint8_t report[DETENT_REPORT_LEN]) {
  if (resync) {
    resync = false;
    while (vendor.available() > 0) vendor.read();
    stats.dropped++;
    return false;
  }
  if (vendor.available() < static_cast<int>(DETENT_REPORT_LEN)) return false;
  return vendor.read(report, DETENT_REPORT_LEN) == DETENT_REPORT_LEN;
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

RxStats rx_stats() { return stats; }

TxStats tx_stats() { return tx; }

void timeline_note(char kind, uint16_t a, uint8_t b) {
  portENTER_CRITICAL(&ring_mux);
  ring[ring_next] = {static_cast<uint32_t>(millis()), kind, a, b};
  ring_next = (ring_next + 1) % kTimelineLen;
  if (ring_count < kTimelineLen) ring_count++;
  portEXIT_CRITICAL(&ring_mux);
}

int timeline(TimelineEntry* out) {
  portENTER_CRITICAL(&ring_mux);
  const int n = ring_count;
  const int first = (ring_next - n + kTimelineLen) % kTimelineLen;
  for (int i = 0; i < n; i++) out[i] = ring[(first + i) % kTimelineLen];
  portEXIT_CRITICAL(&ring_mux);
  return n;
}

}  // namespace usb
}  // namespace detent
