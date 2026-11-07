#include "usb_identity.h"

namespace detent {

void format_serial(const uint8_t mac[6], char out[kSerialBufLen]) {
  static const char kHex[] = "0123456789ABCDEF";
  for (size_t i = 0; i < 6; i++) {
    out[2 * i] = kHex[mac[i] >> 4];
    out[2 * i + 1] = kHex[mac[i] & 0x0F];
  }
  out[12] = '\0';
}

}  // namespace detent
