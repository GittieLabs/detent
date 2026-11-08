// Detent firmware.
//
// usb_device owns the composite USB device; lib/link_layer owns the
// conversation (HELLO, HEARTBEAT, REQUEST_HELLO, HELLO_ACK). This file wires
// them together. Input, encoder modes, and the OLED arrive in later M1 tasks.

#include <Arduino.h>
#include <USBHIDConsumerControl.h>

#include "board_pins.h"
#include "detent_protocol.h"
#include "injector.h"
#include "input.h"
#include "link_layer.h"
#include "modes.h"
#include "report.h"
#include "usb_device.h"
#include "version.h"

// Set by the xiao_esp32s3_debug env only: boot and reset diagnostics on CDC.
#ifndef DETENT_DIAG
#define DETENT_DIAG 0
#endif
#if DETENT_DIAG
#include "diag.h"
#endif

// Set by the xiao_esp32s3_debug env only: trace pin levels, encoder edges, and
// control events on CDC.
#ifndef DETENT_INPUT_LOG
#define DETENT_INPUT_LOG 0
#endif
#if DETENT_INPUT_LOG
#define INPUT_LOG(...) detent::usb::log().printf(__VA_ARGS__)
#else
#define INPUT_LOG(...) \
  do {                 \
  } while (0)
#endif

// Set by the xiao_esp32s3_debug env only. Release firmware ignores TEST_INJECT.
#ifndef DETENT_TEST_INJECT
#define DETENT_TEST_INJECT 0
#endif

namespace {

namespace board = detent::board;

// What HELLO reports: every button in board_pins.h, then the encoder. Filled in
// setup(), before USB starts.
const uint8_t kControlCount = board::kButtonCount + 1;
detent::Control controls[kControlCount];

uint8_t action_of(detent::ButtonEvent e) {
  switch (e) {
    case detent::ButtonEvent::Down: return DETENT_ACTION_DOWN;
    case detent::ButtonEvent::Up: return DETENT_ACTION_UP;
    case detent::ButtonEvent::LongPress: return DETENT_ACTION_LONG_PRESS;
    default: return 0;
  }
}

detent::ButtonEvent event_of(uint8_t action) {
  switch (action) {
    case DETENT_ACTION_DOWN: return detent::ButtonEvent::Down;
    case DETENT_ACTION_UP: return detent::ButtonEvent::Up;
    case DETENT_ACTION_LONG_PRESS: return detent::ButtonEvent::LongPress;
    default: return detent::ButtonEvent::None;
  }
}

const detent::LinkConfig kLinkConfig = {
    DETENT_FW_MAJOR, DETENT_FW_MINOR, DETENT_FW_PATCH, controls, kControlCount,
};

detent::Button buttons[board::kButtonCount];

// The encoder is decoded in a pin change interrupt so turns aren't lost while
// loop() is blocked in a USB send; loop() polls coalesced deltas.
detent::Encoder encoder;
portMUX_TYPE encoder_mux = portMUX_INITIALIZER_UNLOCKED;
volatile uint32_t encoder_edges = 0;  // interrupts seen, for the input trace

void IRAM_ATTR on_encoder_edge() {
  encoder_edges = encoder_edges + 1;
  const bool a = digitalRead(board::kEncoderA);
  const bool b = digitalRead(board::kEncoderB);
  portENTER_CRITICAL_ISR(&encoder_mux);
  encoder.on_edge(a, b);
  portEXIT_CRITICAL_ISR(&encoder_mux);
}

struct UsbTransport : detent::LinkTransport {
  bool send(const uint8_t report[DETENT_REPORT_LEN]) override {
    return detent::usb::send_report(report);
  }
};

UsbTransport transport;
detent::Link device_link(kLinkConfig, transport);
detent::Injector injector(DETENT_TEST_INJECT != 0);

// What the encoder and its push switch do, by mode.
struct FirmwareModeSink : detent::ModeSink {
  void volume_steps(int8_t delta) override {
    INPUT_LOG("route: volume %+d\n", delta);
    const uint16_t usage = delta > 0 ? CONSUMER_CONTROL_VOLUME_INCREMENT
                                     : CONSUMER_CONTROL_VOLUME_DECREMENT;
    for (int i = 0; i < (delta > 0 ? delta : -delta); i++) detent::usb::consumer_tap(usage);
  }
  void mute_toggle() override {
    INPUT_LOG("route: mute toggle\n");
    detent::usb::consumer_tap(CONSUMER_CONTROL_MUTE);
  }
  void encoder(uint8_t control, int8_t delta) override { device_link.send_encoder(control, delta); }
  void button(uint8_t control, uint8_t action) override { device_link.send_button(control, action); }
  // The OLED menu arrives with the display task.
  void menu_scroll(int8_t delta) override { detent::usb::log().printf("menu: scroll %d\n", delta); }
  void menu_select() override { detent::usb::log().print("menu: select\n"); }
  void mode_changed(uint8_t mode) override {
    device_link.set_encoder_mode(mode);
    detent::usb::log().printf("mode: %u\n", mode);
  }
};

FirmwareModeSink mode_sink;
detent::ModeRouter modes(board::kEncoderControl, board::kEncoderPushControl, mode_sink);

// Routes one control event, physical or injected (TEST_INJECT replays through
// here so it exercises the real path).
void on_button(uint8_t control, detent::ButtonEvent e) {
  INPUT_LOG("button %u: %s\n", control,
            e == detent::ButtonEvent::Down ? "down" : e == detent::ButtonEvent::Up ? "up" : "long");
  if (control == board::kEncoderPushControl) {
    modes.on_push(e);
  } else {
    device_link.send_button(control, action_of(e));
  }
}

void on_turn(uint8_t control, int8_t delta) {
  INPUT_LOG("turn %u: %+d (mode %u)\n", control, delta, modes.mode());
  if (control == board::kEncoderControl) {
    modes.on_turn(delta);
  } else {
    device_link.send_encoder(control, delta);
  }
}

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

void setup_controls() {
  for (uint8_t i = 0; i < board::kButtonCount; i++) {
    controls[i] = {board::kButtons[i].control, DETENT_KIND_BUTTON};
    pinMode(board::kButtons[i].gpio, INPUT_PULLUP);
  }
  controls[board::kButtonCount] = {board::kEncoderControl, DETENT_KIND_ENCODER};
  pinMode(board::kEncoderA, INPUT_PULLUP);
  pinMode(board::kEncoderB, INPUT_PULLUP);
  encoder.reset(digitalRead(board::kEncoderA), digitalRead(board::kEncoderB));
  attachInterrupt(digitalPinToInterrupt(board::kEncoderA), on_encoder_edge, CHANGE);
  attachInterrupt(digitalPinToInterrupt(board::kEncoderB), on_encoder_edge, CHANGE);
}

#if DETENT_INPUT_LOG
struct SparePin {
  const char* label;
  uint8_t gpio;
};
const SparePin kSpare[] = {{"D4", 5}, {"D5", 6}, {"D6", 43}, {"D7", 44}};
const uint8_t kSpareCount = sizeof kSpare / sizeof kSpare[0];

int spare_levels() {
  int v = 0;
  for (uint8_t i = 0; i < kSpareCount; i++) v = (v << 1) | (digitalRead(kSpare[i].gpio) ? 1 : 0);
  return v;
}

// Logs raw levels whenever any control pin changes, plus the edge count, so a
// wiring fault shows up as pins that never move.
void trace_pins() {
  static bool spares_ready = false;
  if (!spares_ready) {
    spares_ready = true;
    for (uint8_t i = 0; i < kSpareCount; i++) pinMode(kSpare[i].gpio, INPUT_PULLUP);
  }
  static int last = -1;
  static uint32_t last_edges = 0;
  int levels = 0;
  for (uint8_t i = 0; i < board::kButtonCount; i++) {
    levels = (levels << 1) | (digitalRead(board::kButtons[i].gpio) ? 1 : 0);
  }
  levels = (levels << 1) | (digitalRead(board::kEncoderA) ? 1 : 0);
  levels = (levels << 1) | (digitalRead(board::kEncoderB) ? 1 : 0);
  levels |= spare_levels() << 16;
  const uint32_t edges = encoder_edges;
  static uint32_t last_print = 0;
  const uint32_t now = millis();
  // On any change, and a snapshot every 3 s so resting levels are visible.
  if (levels != last || edges != last_edges || now - last_print >= 3000) {
    last = levels;
    last_edges = edges;
    last_print = now;
    detent::usb::log().printf("pins: buttons(1,2,3,4,6)=");
    for (int i = board::kButtonCount - 1; i >= 0; i--) {
      detent::usb::log().print(((levels >> (i + 2)) & 1) ? '1' : '0');
    }
    detent::usb::log().printf(" A=%d B=%d edges=%lu", (levels >> 1) & 1, levels & 1,
                              static_cast<unsigned long>(edges));
    // Pin finder: the XIAO pins nothing uses yet, with pull ups, so a wire on
    // the wrong pin shows up by name.
    for (uint8_t i = 0; i < kSpareCount; i++) {
      detent::usb::log().printf(" %s=%d", kSpare[i].label, digitalRead(kSpare[i].gpio) ? 1 : 0);
    }
    detent::usb::log().print("\n");
  }
}
#endif

void poll_controls(uint32_t now) {
#if DETENT_INPUT_LOG
  trace_pins();
#endif
  for (uint8_t i = 0; i < board::kButtonCount; i++) {
    const bool pressed = digitalRead(board::kButtons[i].gpio) == LOW;
    const detent::ButtonEvent e = buttons[i].update(pressed, now);
    if (e != detent::ButtonEvent::None) on_button(board::kButtons[i].control, e);
  }
  int8_t delta;
  portENTER_CRITICAL(&encoder_mux);
  const bool turned = encoder.poll(now, &delta);
  portEXIT_CRITICAL(&encoder_mux);
  if (turned) on_turn(board::kEncoderControl, delta);
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
    case DETENT_MSG_TEST_INJECT:
      if (injector.load(msg, now)) {
        detent::usb::log().printf("inject: %u events\n", msg.inject_count);
      } else {
        detent::usb::log().print("inject: ignored (release firmware)\n");
      }
      break;
    default:
      // MENU, STATUS: later M1 tasks.
      detent::usb::log().printf("link: type 0x%02X not handled yet\n", msg.type);
      break;
  }
}

}  // namespace

void setup() {
#if DETENT_DIAG
  detent::diag::on_boot();
#endif
  pinMode(LED_PIN, OUTPUT);
  digitalWrite(LED_PIN, LED_OFF);
  setup_controls();
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
#if DETENT_DIAG
    mounted ? detent::diag::on_mount() : detent::diag::on_unmount();
#endif
  }

  uint8_t report[DETENT_REPORT_LEN];
  while (detent::usb::poll_report(report)) {
    handle_report(report, now);
  }

  poll_controls(now);

  detent::InjectEvent ev;
  while (injector.next_due(now, &ev)) {
    if (ev.type == DETENT_MSG_BUTTON) {
      on_button(ev.control, event_of(ev.value));
    } else {
      on_turn(ev.control, static_cast<int8_t>(ev.value));
    }
  }

  device_link.tick(now);
  update_led(now);
#if DETENT_DIAG
  detent::diag::tick(now, device_link.rejected());
#endif
}
