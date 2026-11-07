// The device side of the link: HELLO, HEARTBEAT, REQUEST_HELLO, HELLO_ACK.
// Pure C++11 with an injected transport and caller supplied time, so it is
// tested natively (D-035). See docs/protocol.md.
//
// - HELLO when the host mounts the device, on REQUEST_HELLO, and on leaving
//   Setup.
// - HEARTBEAT every 2 s while mounted, measured from the last one.
// - One device to app seq counter, wrapping at 255.
// - Paired follows HELLO_ACK and clears on unmount.
// Other valid app messages are handed back to the caller.

#ifndef DETENT_LINK_LAYER_H
#define DETENT_LINK_LAYER_H

#include <stddef.h>
#include <stdint.h>

#include "detent_protocol.h"
#include "report.h"

namespace detent {

const uint32_t kHeartbeatMs = 2000;

struct LinkTransport {
  virtual ~LinkTransport() {}
  // Sends one DETENT_REPORT_LEN byte report on the vendor channel.
  virtual bool send(const uint8_t report[DETENT_REPORT_LEN]) = 0;
};

struct LinkConfig {
  uint8_t fw_major;
  uint8_t fw_minor;
  uint8_t fw_patch;
  const Control* controls;  // what HELLO reports
  uint8_t control_count;
};

class Link {
 public:
  Link(const LinkConfig& config, LinkTransport& transport);

  // USB host mounted (configured) or unmounted the device.
  void on_mount(uint32_t now_ms);
  void on_unmount();

  // A report from the host. REQUEST_HELLO and HELLO_ACK are handled here;
  // any other valid message is returned in `out` for the caller. Malformed
  // reports are counted and dropped. Ignored while unmounted.
  ParseError on_report(const uint8_t* report, size_t len, uint32_t now_ms, AppMessage* out);

  // The user left Setup on the device; the app gets a fresh HELLO.
  void on_setup_exit(uint32_t now_ms);

  // Call often; sends HEARTBEAT when due.
  void tick(uint32_t now_ms);

  // Input events, from real controls or TEST_INJECT. Return false, sending
  // nothing, when unmounted or when the event is invalid (bad action, zero
  // delta).
  bool send_button(uint8_t control, uint8_t action);
  bool send_encoder(uint8_t control, int8_t delta);

  void set_encoder_mode(uint8_t mode) { encoder_mode_ = mode; }
  uint8_t encoder_mode() const { return encoder_mode_; }
  bool mounted() const { return mounted_; }
  bool paired() const { return paired_; }
  uint32_t rejected() const { return rejected_; }

 private:
  void send_hello();
  void send(const uint8_t report[DETENT_REPORT_LEN]);

  LinkConfig config_;
  LinkTransport& transport_;
  uint8_t seq_ = 0;
  uint8_t encoder_mode_ = DETENT_ENCODER_MODE_VOLUME;
  bool mounted_ = false;
  bool paired_ = false;
  uint32_t last_heartbeat_ms_ = 0;
  uint32_t rejected_ = 0;
};

}  // namespace detent

#endif  // DETENT_LINK_LAYER_H
