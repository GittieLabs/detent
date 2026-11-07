// Native tests for lib/usb_identity: the USB serial number and product string.

#include <string.h>
#include <unity.h>

#include "usb_identity.h"

void setUp() {}
void tearDown() {}

static void test_serial_is_12_uppercase_hex_of_the_base_mac() {
  // The build Mac's XIAO; its ROM reported MAC E0:72:A1:FC:4E:DC.
  const uint8_t mac[6] = {0xE0, 0x72, 0xA1, 0xFC, 0x4E, 0xDC};
  char serial[detent::kSerialBufLen];
  detent::format_serial(mac, serial);
  TEST_ASSERT_EQUAL_STRING("E072A1FC4EDC", serial);
  TEST_ASSERT_EQUAL_size_t(12, strlen(serial));
}

static void test_serial_keeps_leading_zeros() {
  const uint8_t mac[6] = {0x00, 0x01, 0x0A, 0xF0, 0x00, 0x0F};
  char serial[detent::kSerialBufLen];
  detent::format_serial(mac, serial);
  TEST_ASSERT_EQUAL_STRING("00010AF0000F", serial);
}

static void test_different_units_get_different_serials() {
  const uint8_t a[6] = {0xE0, 0x72, 0xA1, 0xFC, 0x4E, 0xDC};
  const uint8_t b[6] = {0xE0, 0x72, 0xA1, 0xFC, 0x4E, 0xDD};
  char sa[detent::kSerialBufLen];
  char sb[detent::kSerialBufLen];
  detent::format_serial(a, sa);
  detent::format_serial(b, sb);
  TEST_ASSERT_TRUE(strcmp(sa, sb) != 0);
}

static void test_product_and_manufacturer_strings() {
  TEST_ASSERT_EQUAL_STRING("GittieLabs", detent::kManufacturer);
  // "Detent <model name>"; the model name is a placeholder until Keith picks one.
  TEST_ASSERT_EQUAL_STRING("Detent Dev", detent::kProductName);
}

int main() {
  UNITY_BEGIN();
  RUN_TEST(test_serial_is_12_uppercase_hex_of_the_base_mac);
  RUN_TEST(test_serial_keeps_leading_zeros);
  RUN_TEST(test_different_units_get_different_serials);
  RUN_TEST(test_product_and_manufacturer_strings);
  return UNITY_END();
}
