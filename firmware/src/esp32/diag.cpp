#include "diag.h"

#include <Arduino.h>
#include <esp32s3/rom/rtc.h>
#include <esp_attr.h>
#include <esp_system.h>

#include "display.h"
#include "../usb_device.h"

namespace detent {
namespace diag {
namespace {

const uint32_t kMagic = 0xD7E4D1A6;
const int kHistory = 8;

struct Persisted {
  uint32_t magic;
  uint32_t boots;          // since the last power on
  uint8_t idf[kHistory];   // esp_reset_reason(), newest first
  uint8_t rom[kHistory];   // rtc_get_reset_reason(0), newest first
};

RTC_NOINIT_ATTR Persisted persisted;

uint32_t mounts = 0;
uint32_t unmounts = 0;
uint32_t last_print = 0;

void push(uint8_t* ring, uint8_t value) {
  for (int i = kHistory - 1; i > 0; i--) ring[i] = ring[i - 1];
  ring[0] = value;
}

}  // namespace

void on_boot() {
  if (persisted.magic != kMagic) {
    memset(&persisted, 0, sizeof persisted);
    persisted.magic = kMagic;
  }
  persisted.boots++;
  push(persisted.idf, static_cast<uint8_t>(esp_reset_reason()));
  push(persisted.rom, static_cast<uint8_t>(rtc_get_reset_reason(0)));
}

void on_mount() { mounts++; }
void on_unmount() { unmounts++; }

void tick(uint32_t now_ms, uint32_t rejected) {
  if (now_ms - last_print < 2000) return;
  last_print = now_ms;
  Print& out = usb::log();
  const uint8_t* i2c;
  const int i2c_n = display::scanned(&i2c);
  out.print("diag: i2c=[");
  for (int i = 0; i < i2c_n; i++) out.printf(i ? " %02X" : "%02X", i2c[i]);
  out.printf("] oled=%s boots=%lu uptime_ms=%lu mounts=%lu unmounts=%lu idf=[",
             display::present() ? "yes" : "no",
             static_cast<unsigned long>(persisted.boots), static_cast<unsigned long>(now_ms),
             static_cast<unsigned long>(mounts), static_cast<unsigned long>(unmounts));
  for (int i = 0; i < kHistory; i++) out.printf(i ? " %u" : "%u", persisted.idf[i]);
  out.print("] rom=[");
  for (int i = 0; i < kHistory; i++) out.printf(i ? " %u" : "%u", persisted.rom[i]);
  const usb::RxStats rx = usb::rx_stats();
  const usb::TxStats t = usb::tx_stats();
  out.printf("] tx=%lu tx_fail=%lu tx_max_ms=%lu tx_first_fail_ms=%lu",
             static_cast<unsigned long>(t.attempts), static_cast<unsigned long>(t.failures),
             static_cast<unsigned long>(t.max_ms), static_cast<unsigned long>(t.first_fail_ms));
  out.printf(" rx_events=%lu rx_short=%lu rx_min_len=%u rx_dropped=%lu rejected=%lu\n",
             static_cast<unsigned long>(rx.events), static_cast<unsigned long>(rx.short_),
             rx.min_len, static_cast<unsigned long>(rx.dropped),
             static_cast<unsigned long>(rejected));
  usb::TimelineEntry tl[usb::kTimelineLen];
  const int n = usb::timeline(tl);
  out.print("timeline:");
  for (int i = 0; i < n; i++) {
    out.printf(" %lu%c", static_cast<unsigned long>(tl[i].ms), tl[i].kind);
    if (tl[i].kind == 'R') out.printf("(%u,0x%02X)", tl[i].a, tl[i].b);
    if (tl[i].kind == 'S') out.printf("(0x%02X)", tl[i].b);
    if (tl[i].kind == 'K' || tl[i].kind == 'F') out.printf("(%u)", tl[i].a);
  }
  out.print("\n");
}

}  // namespace diag
}  // namespace detent
