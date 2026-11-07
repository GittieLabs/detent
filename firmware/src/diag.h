// Boot diagnostics for the debug firmware (DETENT_DIAG). Counts boots and keeps
// the last reset reasons in RTC memory, which survives every reset except
// power loss, and prints them on the CDC port every 2 s so a host can tell a
// device reboot from a USB only reset. Investigation aid for the M1 early
// write bug; compiled out of release firmware.

#ifndef DETENT_DIAG_H
#define DETENT_DIAG_H

#include <stdint.h>

namespace detent {
namespace diag {

// Call first thing in setup().
void on_boot();

// Count USB connection events.
void on_mount();
void on_unmount();

// Call from loop(); prints a line every 2 s while the CDC port is open.
// `rejected` is the link's count of reports that failed to parse.
void tick(uint32_t now_ms, uint32_t rejected);

}  // namespace diag
}  // namespace detent

#endif  // DETENT_DIAG_H
