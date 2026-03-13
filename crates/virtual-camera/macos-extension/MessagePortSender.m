#import "MessagePortSender.h"
#import <CoreFoundation/CoreFoundation.h>
#import <Foundation/Foundation.h>

/// Mach service name — must match the Extension's CFMessagePort name.
/// Sandbox allows `com.kalidokit.rust.*` via application-groups entitlement.
static NSString *const kPortName = @"com.kalidokit.rust.vcam";

static CFMessagePortRef sRemotePort = NULL;

bool vcam_mach_connect(void) {
    if (sRemotePort != NULL) {
        // Already connected — verify port is still valid
        if (CFMessagePortIsValid(sRemotePort)) return true;
        CFRelease(sRemotePort);
        sRemotePort = NULL;
    }

    sRemotePort = CFMessagePortCreateRemote(kCFAllocatorDefault,
                                             (__bridge CFStringRef)kPortName);
    if (sRemotePort == NULL) {
        static int failCount = 0;
        if (failCount++ % 60 == 0) {
            NSLog(@"[VCam Host] CFMessagePortCreateRemote(%@) failed — Extension not ready?", kPortName);
        }
        return false;
    }

    NSLog(@"[VCam Host] Connected to Extension via Mach port: %@", kPortName);
    return true;
}

bool vcam_mach_send_frame(const uint8_t *bgra, uint32_t width, uint32_t height) {
    if (sRemotePort == NULL || !CFMessagePortIsValid(sRemotePort)) {
        if (!vcam_mach_connect()) return false;
    }

    // Message format: [width: u32 LE][height: u32 LE][BGRA pixel data]
    size_t pixel_size = (size_t)width * height * 4;
    size_t total_size = 8 + pixel_size;

    // Build the message data
    NSMutableData *msg = [NSMutableData dataWithLength:total_size];
    uint8_t *buf = (uint8_t *)msg.mutableBytes;
    memcpy(buf + 0, &width, 4);
    memcpy(buf + 4, &height, 4);
    memcpy(buf + 8, bgra, pixel_size);

    SInt32 status = CFMessagePortSendRequest(sRemotePort,
                                              0,  // msgid
                                              (__bridge CFDataRef)msg,
                                              1.0,  // send timeout (sec)
                                              0.0,  // recv timeout (no reply)
                                              NULL,  // no reply mode
                                              NULL); // no reply data

    if (status != kCFMessagePortSuccess) {
        // Port might have become invalid (Extension restarted)
        CFRelease(sRemotePort);
        sRemotePort = NULL;
        return false;
    }

    return true;
}

void vcam_mach_disconnect(void) {
    if (sRemotePort != NULL) {
        CFRelease(sRemotePort);
        sRemotePort = NULL;
        NSLog(@"[VCam Host] Disconnected from Extension Mach port");
    }
}
