// The little that differs per chip in the shared firmware (M7): the encoder
// interrupt's attribute and the lock between it and loop().

#ifndef DETENT_PLATFORM_H
#define DETENT_PLATFORM_H

#include <Arduino.h>

#if defined(ARDUINO_ARCH_ESP32)

// Interrupt handlers run from IRAM so a flash access can't stall them.
#define DETENT_ISR IRAM_ATTR

namespace detent {
// A spinlock, as the ESP32's interrupts and loop() can run on either core.
class IrqLock {
 public:
  void lock_isr() { portENTER_CRITICAL_ISR(&mux_); }
  void unlock_isr() { portEXIT_CRITICAL_ISR(&mux_); }
  void lock() { portENTER_CRITICAL(&mux_); }
  void unlock() { portEXIT_CRITICAL(&mux_); }

 private:
  portMUX_TYPE mux_ = portMUX_INITIALIZER_UNLOCKED;
};
}  // namespace detent

#elif defined(ARDUINO_ARCH_RP2040)

#include <hardware/sync.h>

#define DETENT_ISR

namespace detent {
// loop() and the pin interrupt share core 0: masking interrupts in loop() is
// enough, and the handler itself can't be interrupted by itself.
class IrqLock {
 public:
  void lock_isr() {}
  void unlock_isr() {}
  void lock() { saved_ = save_and_disable_interrupts(); }
  void unlock() { restore_interrupts(saved_); }

 private:
  uint32_t saved_ = 0;
};
}  // namespace detent

#else
#error "Detent's firmware supports ESP32 and RP2040 boards"
#endif

#endif  // DETENT_PLATFORM_H
