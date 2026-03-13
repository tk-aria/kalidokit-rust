#ifndef MESSAGE_PORT_SENDER_H
#define MESSAGE_PORT_SENDER_H

#include <stdint.h>
#include <stdbool.h>

/// Initialize the CFMessagePort remote connection to the Extension.
/// Returns true on success. Can be called multiple times (idempotent).
bool vcam_mach_connect(void);

/// Send a BGRA frame to the Extension via CFMessagePort.
/// Returns true on success.
bool vcam_mach_send_frame(const uint8_t *bgra, uint32_t width, uint32_t height);

/// Disconnect and clean up.
void vcam_mach_disconnect(void);

#endif
