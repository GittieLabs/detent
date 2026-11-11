// The composite USB device on the RP2040 (Arduino-Pico core with Adafruit
// TinyUSB): the same device the XIAO is (D-036). One HID interface with the
// vendor channel (report ID 6, 63 byte reports, usage page 0xFF00 usage 1)
// and Consumer Control (report ID 4), plus CDC for uploads and logs.
// Implements src/usb_device.h, so the shared firmware runs unchanged.

#include <Adafruit_TinyUSB.h>
#include <pico/unique_id.h>
#include <string.h>

#include "../usb_device.h"
#include "usb_identity.h"

// PLACEHOLDER (open question in docs/roadmap.md): Raspberry Pi assigns free
// PIDs under its VID 0x2E8A on request. Until then pid.codes' testing pair,
// which is reserved for exactly this and never ships.
#ifndef DETENT_PICO_VID
#define DETENT_PICO_VID 0x1209
#endif
#ifndef DETENT_PICO_PID
#define DETENT_PICO_PID 0x0001
#endif

namespace detent {
namespace usb {
namespace {

// Vendor collection first, as on the XIAO.
const uint8_t kReportDescriptor[] = {
    TUD_HID_REPORT_DESC_GENERIC_INOUT(DETENT_REPORT_LEN, HID_REPORT_ID(DETENT_VENDOR_REPORT_ID)),
    TUD_HID_REPORT_DESC_CONSUMER(HID_REPORT_ID(DETENT_CONSUMER_REPORT_ID)),
};

Adafruit_USBD_HID hid;

// The pico's flash chip has a 64 bit unique ID: 16 hex chars.
char serial_number[2 * PICO_UNIQUE_BOARD_ID_SIZE_BYTES + 1];

// Output reports from the host, filled in the TinyUSB callback.
const int kRxReports = 8;
uint8_t rx[kRxReports][DETENT_REPORT_LEN];
volatile int rx_head = 0;  // next to read
volatile int rx_tail = 0;  // next to write
RxStats stats = {0, 0, 0, 0xFFFF};
TxStats tx = {0, 0, 0, 0};
bool was_mounted = false;

uint16_t on_get_report(uint8_t, hid_report_type_t, uint8_t*, uint16_t) { return 0; }

// With report IDs in the descriptor, an interrupt OUT report arrives with its
// ID as the first byte and `report_id` 0; a SET_REPORT arrives with the ID
// in `report_id` and without it in the buffer. Both are accepted.
void on_set_report(uint8_t report_id, hid_report_type_t, const uint8_t* buf, uint16_t len) {
  stats.events++;
  if (report_id == 0 && len > 0 && buf[0] == DETENT_VENDOR_REPORT_ID) {
    report_id = buf[0];
    buf++;
    len--;
  }
  if (report_id != DETENT_VENDOR_REPORT_ID) return;
  if (len < stats.min_len) stats.min_len = len;
  if (len < DETENT_REPORT_LEN) {
    stats.short_++;
    return;
  }
  const int next = (rx_tail + 1) % kRxReports;
  if (next == rx_head) {
    stats.dropped++;
    return;
  }
  memcpy(rx[rx_tail], buf, DETENT_REPORT_LEN);
  rx_tail = next;
}

bool wait_ready(uint32_t ms) {
  const uint32_t start = millis();
  while (!hid.ready()) {
    if (millis() - start > ms || !TinyUSBDevice.mounted()) return false;
    delay(1);
  }
  return true;
}

}  // namespace

void begin() {
  pico_unique_board_id_t id;
  pico_get_unique_board_id(&id);
  static const char kHex[] = "0123456789ABCDEF";
  for (int i = 0; i < PICO_UNIQUE_BOARD_ID_SIZE_BYTES; i++) {
    serial_number[2 * i] = kHex[id.id[i] >> 4];
    serial_number[2 * i + 1] = kHex[id.id[i] & 0x0F];
  }
  serial_number[2 * PICO_UNIQUE_BOARD_ID_SIZE_BYTES] = '\0';

  if (!TinyUSBDevice.isInitialized()) TinyUSBDevice.begin(0);
  TinyUSBDevice.setID(DETENT_PICO_VID, DETENT_PICO_PID);
  TinyUSBDevice.setManufacturerDescriptor(kManufacturer);
  TinyUSBDevice.setProductDescriptor(kProductName);
  TinyUSBDevice.setSerialDescriptor(serial_number);

  hid.setReportDescriptor(kReportDescriptor, sizeof kReportDescriptor);
  hid.setPollInterval(1);
  hid.setReportCallback(on_get_report, on_set_report);
  hid.begin();

  // The core started USB before setup(); re-enumerate with our identity and
  // HID interface (CDC stays, for the 1200 baud touch into the bootloader).
  if (TinyUSBDevice.mounted()) {
    TinyUSBDevice.detach();
    delay(10);
    TinyUSBDevice.attach();
  }
}

bool send_report(const uint8_t report[DETENT_REPORT_LEN]) {
  tx.attempts++;
  const uint32_t start = millis();
  const bool ok = wait_ready(50) &&
                  hid.sendReport(DETENT_VENDOR_REPORT_ID, report, DETENT_REPORT_LEN);
  const uint32_t took = millis() - start;
  if (took > tx.max_ms) tx.max_ms = took;
  if (!ok) {
    tx.failures++;
    if (tx.first_fail_ms == 0) tx.first_fail_ms = start ? start : 1;
  }
  return ok;
}

bool poll_report(uint8_t report[DETENT_REPORT_LEN]) {
  if (rx_head == rx_tail) return false;
  memcpy(report, rx[rx_head], DETENT_REPORT_LEN);
  rx_head = (rx_head + 1) % kRxReports;
  return true;
}

// TinyUSB on this core has no mount callback to queue from, so the state is
// polled and changes reported in order.
bool poll_connection(bool* mounted) {
  const bool now = TinyUSBDevice.mounted();
  if (now == was_mounted) return false;
  was_mounted = now;
  *mounted = now;
  return true;
}

void consumer_tap(uint16_t usage) {
  if (!wait_ready(50)) return;
  hid.sendReport16(DETENT_CONSUMER_REPORT_ID, usage);
  if (!wait_ready(50)) return;
  hid.sendReport16(DETENT_CONSUMER_REPORT_ID, 0);
}

const char* serial() { return serial_number; }

Print& log() { return Serial; }

RxStats rx_stats() { return stats; }

TxStats tx_stats() { return tx; }

void timeline_note(char, uint16_t, uint8_t) {}

int timeline(TimelineEntry*) { return 0; }

}  // namespace usb
}  // namespace detent
