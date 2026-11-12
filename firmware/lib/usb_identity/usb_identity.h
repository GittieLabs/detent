// USB identity strings. Pure C++11, tested natively; src/usb_device.cpp applies
// them before USB starts. See "Identity" in docs/protocol.md.

#ifndef DETENT_USB_IDENTITY_H
#define DETENT_USB_IDENTITY_H

#include <stddef.h>
#include <stdint.h>

#include "detent_protocol.h"

// The model name in the product string "Detent <model name>" comes from the
// generated header, by the build's -DDETENT_BOARD_* flag (D-113). Hosts know a
// Detent by that string (D-112), so a custom board keeps the "Detent " prefix.
// Only a build with no board flag (the native tests of other libraries) gets
// this stand in.
#ifndef DETENT_MODEL_NAME
#define DETENT_MODEL_NAME "Test"
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
