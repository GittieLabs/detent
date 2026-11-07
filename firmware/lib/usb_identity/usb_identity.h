// USB identity strings. Pure C++11, tested natively; src/usb_device.cpp applies
// them before USB starts. See "Identity" in docs/protocol.md.

#ifndef DETENT_USB_IDENTITY_H
#define DETENT_USB_IDENTITY_H

#include <stddef.h>
#include <stdint.h>

// PLACEHOLDER: the model name for the product string "Detent <model name>"
// (open question in docs/roadmap.md). Override with -DDETENT_MODEL_NAME="...".
#ifndef DETENT_MODEL_NAME
#define DETENT_MODEL_NAME "Dev"
#endif

namespace detent {

const char* const kManufacturer = "GittieLabs";
const char* const kProductName = "Detent " DETENT_MODEL_NAME;

// 12 hex chars plus NUL.
const size_t kSerialBufLen = 13;

// Formats the chip's 6 byte base MAC as 12 uppercase hex chars, the USB serial
// number. Two units of the same model differ only here (D-004).
void format_serial(const uint8_t mac[6], char out[kSerialBufLen]);

}  // namespace detent

#endif  // DETENT_USB_IDENTITY_H
