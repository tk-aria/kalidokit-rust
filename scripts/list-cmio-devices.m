// list-cmio-devices.m
// Lists all CoreMediaIO devices (both physical and virtual cameras).
// Build: clang -fobjc-arc -fmodules -framework CoreMediaIO -framework Foundation -o list-cmio-devices list-cmio-devices.m

#import <Foundation/Foundation.h>
#import <CoreMediaIO/CMIOHardwareSystem.h>
#import <CoreMediaIO/CMIOHardwareObject.h>
#import <CoreMediaIO/CMIOHardwareDevice.h>
#import <CoreMediaIO/CMIOHardwareStream.h>

static NSString *getDeviceName(CMIOObjectID deviceID) {
    CMIOObjectPropertyAddress addr = {
        .mSelector = kCMIOObjectPropertyName,
        .mScope = kCMIOObjectPropertyScopeWildcard,
        .mElement = kCMIOObjectPropertyElementWildcard
    };
    UInt32 dataSize = 0;
    CMIOObjectGetPropertyDataSize(deviceID, &addr, 0, NULL, &dataSize);
    if (dataSize == 0) return @"(unknown)";

    CFStringRef name = NULL;
    UInt32 used = 0;
    CMIOObjectGetPropertyData(deviceID, &addr, 0, NULL, dataSize, &used, &name);
    return name ? (__bridge_transfer NSString *)name : @"(null)";
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        // Allow CMIO to discover Camera Extension providers
        CMIOObjectPropertyAddress allowAddr = {
            .mSelector = kCMIOHardwarePropertyAllowScreenCaptureDevices,
            .mScope = kCMIOObjectPropertyScopeWildcard,
            .mElement = kCMIOObjectPropertyElementWildcard
        };
        UInt32 allow = 1;
        CMIOObjectSetPropertyData(kCMIOObjectSystemObject, &allowAddr, 0, NULL, sizeof(allow), &allow);

        // Get all devices
        CMIOObjectPropertyAddress addr = {
            .mSelector = kCMIOHardwarePropertyDevices,
            .mScope = kCMIOObjectPropertyScopeWildcard,
            .mElement = kCMIOObjectPropertyElementWildcard
        };
        UInt32 dataSize = 0;
        CMIOObjectGetPropertyDataSize(kCMIOObjectSystemObject, &addr, 0, NULL, &dataSize);

        UInt32 count = dataSize / sizeof(CMIODeviceID);
        NSLog(@"Found %u CoreMediaIO devices:", count);

        CMIODeviceID *devices = malloc(dataSize);
        UInt32 used = 0;
        CMIOObjectGetPropertyData(kCMIOObjectSystemObject, &addr, 0, NULL, dataSize, &used, devices);

        for (UInt32 i = 0; i < count; i++) {
            NSString *name = getDeviceName(devices[i]);
            NSLog(@"  [%u] ID=%u name=%@", i, devices[i], name);

            // Get streams for this device
            CMIOObjectPropertyAddress streamAddr = {
                .mSelector = kCMIODevicePropertyStreams,
                .mScope = kCMIOObjectPropertyScopeWildcard,
                .mElement = kCMIOObjectPropertyElementWildcard
            };
            UInt32 streamDataSize = 0;
            CMIOObjectGetPropertyDataSize(devices[i], &streamAddr, 0, NULL, &streamDataSize);
            UInt32 streamCount = streamDataSize / sizeof(CMIOStreamID);

            CMIOStreamID *streams = malloc(streamDataSize);
            UInt32 streamUsed = 0;
            CMIOObjectGetPropertyData(devices[i], &streamAddr, 0, NULL, streamDataSize, &streamUsed, streams);

            for (UInt32 j = 0; j < streamCount; j++) {
                // Get stream direction
                CMIOObjectPropertyAddress dirAddr = {
                    .mSelector = kCMIOStreamPropertyDirection,
                    .mScope = kCMIOObjectPropertyScopeWildcard,
                    .mElement = kCMIOObjectPropertyElementWildcard
                };
                UInt32 direction = 0;
                UInt32 dirUsed = 0;
                CMIOObjectGetPropertyData(streams[j], &dirAddr, 0, NULL, sizeof(direction), &dirUsed, &direction);

                NSLog(@"    Stream %u: direction=%u (%@)", streams[j], direction, direction == 0 ? @"output" : @"input/sink");
            }
            free(streams);
        }
        free(devices);
    }
    return 0;
}
