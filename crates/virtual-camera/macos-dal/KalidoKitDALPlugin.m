/// KalidoKit Virtual Camera — CMIO DAL Plugin
///
/// Minimal DAL plugin that exposes a virtual camera device to all macOS apps
/// including sandboxed browsers (Chrome, Safari). Receives frames from the
/// host app via TCP localhost:19876 (same IPC as the CMIOExtension).
///
/// Object hierarchy:
///   Plugin (pluginID) → Device (deviceID) → Stream (streamID)
///
/// References:
///   - Apple CMIOHardwarePlugIn.h
///   - OBS obs-mac-virtualcam DAL plugin
///   - johnboiles/coremediaio-dal-minimal-example

#import <CoreMediaIO/CMIOHardwarePlugIn.h>
#import <CoreMediaIO/CMIOHardwareDevice.h>
#import <CoreMediaIO/CMIOHardwareStream.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <IOKit/audio/IOAudioTypes.h>
#import <mach/mach_time.h>
#import <sys/socket.h>
#import <netinet/in.h>
#import <netinet/tcp.h>
#import <arpa/inet.h>
#import <unistd.h>
#import <fcntl.h>
#import <string.h>
#import <errno.h>

// ---------- Constants ----------

/// Factory UUID — must match Info.plist CFPlugInFactories key.
#define kPlugInFactoryUUID  CFSTR("3F8E656F-ECB4-4D4A-BDEE-CA6A4410B6C8")

static const uint32_t kFrameWidth  = 1280;
static const uint32_t kFrameHeight = 720;
static const uint32_t kFrameRate   = 30;
static const uint16_t kTcpPort     = 19876;
static const size_t   kFrameHeaderSize = 8;

// Object IDs (assigned by us, reported to the DAL)
static CMIOObjectID gPlugInID  = 0;
static CMIOObjectID gDeviceID  = 0;
static CMIOObjectID gStreamID  = 0;

// Plugin interface ref (double-pointer per COM pattern)
static CMIOHardwarePlugInInterface  gPlugInInterface;
static CMIOHardwarePlugInInterface* gPlugInInterfacePtr = &gPlugInInterface;
static CMIOHardwarePlugInRef        gPlugInRef = &gPlugInInterfacePtr;

// Stream state
static CMSimpleQueueRef       gQueue = NULL;
static CMVideoFormatDescriptionRef gFormatDesc = NULL;
static CMIODeviceStreamQueueAlteredProc gQueueAlteredProc = NULL;
static void* gQueueAlteredRefCon = NULL;
static Boolean gStreamRunning = false;

// TCP client state
static int gClientFd = -1;
static dispatch_source_t gReadSource = nil;
static dispatch_source_t gConnectTimer = nil;
static dispatch_queue_t  gNetQueue = nil;
static NSMutableData*    gReadBuffer = nil;

// Frame timer
static dispatch_source_t gFrameTimer = nil;
static dispatch_queue_t  gTimerQueue = nil;
static uint64_t gFrameCounter = 0;

// Latest frame from TCP
static dispatch_queue_t gFrameQueue = nil;
static NSData* gLatestFrameData = nil;
static uint32_t gLatestWidth = 0;
static uint32_t gLatestHeight = 0;

// ---------- Forward declarations ----------

static void startTcpClient(void);
static void stopTcpClient(void);
static void tryConnect(void);
static void readFromHost(void);
static void startFrameTimer(void);
static void stopFrameTimer(void);
static void pushFrame(void);

// ---------- TCP Client (same protocol as CMIOExtension) ----------

static void tryConnect(void) {
    if (gClientFd >= 0) return;

    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return;

    struct sockaddr_in addr = {0};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(kTcpPort);
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);

    if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        close(fd);
        return;
    }

    int flags = fcntl(fd, F_GETFL, 0);
    fcntl(fd, F_SETFL, flags | O_NONBLOCK);
    int flag = 1;
    setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &flag, sizeof(flag));
    int rcvbuf = 4 * 1024 * 1024;
    setsockopt(fd, SOL_SOCKET, SO_RCVBUF, &rcvbuf, sizeof(rcvbuf));

    gClientFd = fd;
    [gReadBuffer setLength:0];

    NSLog(@"[KalidoKit DAL] Connected to host on 127.0.0.1:%u", kTcpPort);

    gReadSource = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, fd, 0, gNetQueue);
    dispatch_source_set_event_handler(gReadSource, ^{ readFromHost(); });
    dispatch_source_set_cancel_handler(gReadSource, ^{
        if (gClientFd == fd) {
            close(fd);
            gClientFd = -1;
            NSLog(@"[KalidoKit DAL] Disconnected from host");
        }
    });
    dispatch_resume(gReadSource);
}

static void readFromHost(void) {
    uint8_t tmp[64 * 1024];

    while (1) {
        ssize_t n = read(gClientFd, tmp, sizeof(tmp));
        if (n < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) break;
            if (gReadSource) { dispatch_source_cancel(gReadSource); gReadSource = nil; }
            return;
        }
        if (n == 0) {
            if (gReadSource) { dispatch_source_cancel(gReadSource); gReadSource = nil; }
            return;
        }
        [gReadBuffer appendBytes:tmp length:n];
    }

    while (gReadBuffer.length >= kFrameHeaderSize) {
        const uint8_t *buf = (const uint8_t *)gReadBuffer.bytes;
        uint32_t width, height;
        memcpy(&width, buf + 0, 4);
        memcpy(&height, buf + 4, 4);

        if (width == 0 || height == 0 || width > 8192 || height > 8192) {
            [gReadBuffer setLength:0];
            return;
        }

        size_t frameSize = kFrameHeaderSize + (size_t)width * height * 4;
        if (gReadBuffer.length < frameSize) break;

        NSData *pixelData = [NSData dataWithBytes:buf + kFrameHeaderSize
                                           length:(size_t)width * height * 4];

        dispatch_sync(gFrameQueue, ^{
            gLatestFrameData = pixelData;
            gLatestWidth = width;
            gLatestHeight = height;
        });

        [gReadBuffer replaceBytesInRange:NSMakeRange(0, frameSize) withBytes:NULL length:0];
    }
}

static void startTcpClient(void) {
    if (!gNetQueue) {
        gNetQueue = dispatch_queue_create("com.kalidokit.dal.net", DISPATCH_QUEUE_SERIAL);
    }
    if (!gReadBuffer) {
        gReadBuffer = [NSMutableData data];
    }
    if (!gFrameQueue) {
        gFrameQueue = dispatch_queue_create("com.kalidokit.dal.frame", DISPATCH_QUEUE_SERIAL);
    }

    tryConnect();

    if (!gConnectTimer) {
        gConnectTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, gNetQueue);
        dispatch_source_set_timer(gConnectTimer, dispatch_time(DISPATCH_TIME_NOW, 0),
                                  (uint64_t)(1.0 * NSEC_PER_SEC), NSEC_PER_MSEC);
        dispatch_source_set_event_handler(gConnectTimer, ^{
            if (gClientFd < 0) tryConnect();
        });
        dispatch_resume(gConnectTimer);
    }
}

static void stopTcpClient(void) {
    if (gConnectTimer) { dispatch_source_cancel(gConnectTimer); gConnectTimer = nil; }
    if (gReadSource) { dispatch_source_cancel(gReadSource); gReadSource = nil; }
    if (gClientFd >= 0) { close(gClientFd); gClientFd = -1; }
}

// ---------- Frame Timer ----------

static void startFrameTimer(void) {
    if (gFrameTimer) return;

    gTimerQueue = dispatch_queue_create("com.kalidokit.dal.timer", DISPATCH_QUEUE_SERIAL);
    gFrameTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, gTimerQueue);
    dispatch_source_set_timer(gFrameTimer, dispatch_time(DISPATCH_TIME_NOW, 0),
                              (uint64_t)(1.0 / kFrameRate * NSEC_PER_SEC), NSEC_PER_MSEC);
    dispatch_source_set_event_handler(gFrameTimer, ^{ pushFrame(); });
    dispatch_resume(gFrameTimer);
}

static void stopFrameTimer(void) {
    if (gFrameTimer) { dispatch_source_cancel(gFrameTimer); gFrameTimer = nil; }
}

static void pushFrame(void) {
    if (!gQueue) return;

    __block NSData *frameData = nil;
    __block uint32_t width = 0, height = 0;

    dispatch_sync(gFrameQueue, ^{
        frameData = gLatestFrameData;
        width = gLatestWidth;
        height = gLatestHeight;
    });

    // Generate test pattern if no TCP data
    if (!frameData || width == 0 || height == 0) {
        width = kFrameWidth;
        height = kFrameHeight;
        size_t dataSize = width * height * 4;
        NSMutableData *testData = [NSMutableData dataWithLength:dataSize];
        uint8_t *p = testData.mutableBytes;
        int phase = (gFrameCounter / 60) % 3;
        uint8_t r = (phase == 0) ? 0 : 0, g = (phase == 1) ? 200 : 0, b = (phase == 2) ? 200 : 80;
        for (uint32_t i = 0; i < width * height; i++) {
            p[i*4+0] = b; p[i*4+1] = g; p[i*4+2] = r; p[i*4+3] = 255;
        }
        frameData = testData;
    }

    CVPixelBufferRef pixelBuffer = NULL;
    NSDictionary *attrs = @{ (NSString *)kCVPixelBufferIOSurfacePropertiesKey: @{} };
    CVReturn cvStatus = CVPixelBufferCreate(kCFAllocatorDefault, width, height,
                                            kCVPixelFormatType_32BGRA,
                                            (__bridge CFDictionaryRef)attrs, &pixelBuffer);
    if (cvStatus != kCVReturnSuccess || !pixelBuffer) return;

    CVPixelBufferLockBaseAddress(pixelBuffer, 0);
    uint8_t *dst = CVPixelBufferGetBaseAddress(pixelBuffer);
    size_t dstBytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer);
    size_t srcBytesPerRow = (size_t)width * 4;
    const uint8_t *src = (const uint8_t *)frameData.bytes;
    for (uint32_t y = 0; y < height; y++) {
        memcpy(dst + y * dstBytesPerRow, src + y * srcBytesPerRow, srcBytesPerRow);
    }
    CVPixelBufferUnlockBaseAddress(pixelBuffer, 0);

    CMVideoFormatDescriptionRef formatDesc = NULL;
    CMVideoFormatDescriptionCreateForImageBuffer(kCFAllocatorDefault, pixelBuffer, &formatDesc);
    if (!formatDesc) { CVPixelBufferRelease(pixelBuffer); return; }

    uint64_t hostTimeNs = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
    CMTime pts = CMTimeMake((int64_t)hostTimeNs, 1000000000);
    CMSampleTimingInfo timing = {
        .duration = CMTimeMake(1, kFrameRate),
        .presentationTimeStamp = pts,
        .decodeTimeStamp = kCMTimeInvalid
    };
    CMSampleBufferRef sampleBuffer = NULL;
    OSStatus sbStatus = CMSampleBufferCreateReadyWithImageBuffer(
        kCFAllocatorDefault, pixelBuffer, formatDesc, &timing, &sampleBuffer);
    CFRelease(formatDesc);
    CVPixelBufferRelease(pixelBuffer);

    if (sampleBuffer && sbStatus == noErr) {
        OSStatus enqStatus = CMSimpleQueueEnqueue(gQueue, sampleBuffer);
        if (enqStatus == noErr) {
            // Notify consumer that queue changed
            if (gQueueAlteredProc) {
                gQueueAlteredProc(gStreamID, sampleBuffer, gQueueAlteredRefCon);
            }
        } else {
            CFRelease(sampleBuffer);
        }
    }

    gFrameCounter++;
    if (gFrameCounter % 300 == 0) {
        NSLog(@"[KalidoKit DAL] Frame %llu (%ux%u) connected=%d",
              gFrameCounter, width, height, gClientFd >= 0);
    }
}

// ---------- Plugin Interface Implementation ----------

static HRESULT DAL_QueryInterface(void *self, REFIID uuid, LPVOID *interface) {
    CFUUIDRef requested = CFUUIDCreateFromUUIDBytes(kCFAllocatorDefault, uuid);
    CFUUIDRef plugInInterfaceID = CFUUIDGetConstantUUIDWithBytes(
        NULL, 0xB8, 0x9D, 0xFA, 0xBA, 0x93, 0xBF, 0x11, 0xD8,
        0x8E, 0xA6, 0x00, 0x0A, 0x95, 0xAF, 0x9C, 0x6A);
    CFUUIDRef iUnknownID = CFUUIDGetConstantUUIDWithBytes(
        NULL, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46);

    if (CFEqual(requested, plugInInterfaceID) || CFEqual(requested, iUnknownID)) {
        CFRelease(requested);
        *interface = gPlugInRef;
        return S_OK;
    }

    CFRelease(requested);
    *interface = NULL;
    return E_NOINTERFACE;
}

static ULONG DAL_AddRef(void *self) { return 1; }
static ULONG DAL_Release(void *self) { return 1; }

static OSStatus DAL_Initialize(CMIOHardwarePlugInRef self) {
    return kCMIOHardwareIllegalOperationError;
}

static OSStatus DAL_InitializeWithObjectID(CMIOHardwarePlugInRef self, CMIOObjectID objectID) {
    gPlugInID = objectID;

    NSLog(@"[KalidoKit DAL] Initialize (pluginID=%u)", objectID);

    // Create format description
    CMVideoFormatDescriptionCreate(kCFAllocatorDefault,
                                   kCMVideoCodecType_422YpCbCr8,
                                   kFrameWidth, kFrameHeight,
                                   NULL, &gFormatDesc);

    // Create device
    OSStatus err = CMIOObjectCreate(self, kCMIOObjectSystemObject, kCMIODeviceClassID, &gDeviceID);
    if (err != noErr) {
        NSLog(@"[KalidoKit DAL] Failed to create device: %d", (int)err);
        return err;
    }

    // Create stream
    err = CMIOObjectCreate(self, gDeviceID, kCMIOStreamClassID, &gStreamID);
    if (err != noErr) {
        NSLog(@"[KalidoKit DAL] Failed to create stream: %d", (int)err);
        return err;
    }

    // Publish device + stream
    CMIOObjectID devPub[] = { gDeviceID };
    err = CMIOObjectsPublishedAndDied(self, kCMIOObjectSystemObject, 1, devPub, 0, NULL);
    if (err != noErr) {
        NSLog(@"[KalidoKit DAL] Failed to publish device: %d", (int)err);
        return err;
    }

    CMIOObjectID strmPub[] = { gStreamID };
    err = CMIOObjectsPublishedAndDied(self, gDeviceID, 1, strmPub, 0, NULL);
    if (err != noErr) {
        NSLog(@"[KalidoKit DAL] Failed to publish stream: %d", (int)err);
        return err;
    }

    NSLog(@"[KalidoKit DAL] Published device=%u stream=%u", gDeviceID, gStreamID);

    // Start TCP client
    startTcpClient();

    return noErr;
}

static OSStatus DAL_Teardown(CMIOHardwarePlugInRef self) {
    NSLog(@"[KalidoKit DAL] Teardown");

    stopFrameTimer();
    stopTcpClient();

    if (gStreamID) {
        CMIOObjectID dead[] = { gStreamID };
        CMIOObjectsPublishedAndDied(self, gDeviceID, 0, NULL, 1, dead);
        gStreamID = 0;
    }
    if (gDeviceID) {
        CMIOObjectID dead[] = { gDeviceID };
        CMIOObjectsPublishedAndDied(self, kCMIOObjectSystemObject, 0, NULL, 1, dead);
        gDeviceID = 0;
    }

    if (gQueue) { CFRelease(gQueue); gQueue = NULL; }
    if (gFormatDesc) { CFRelease(gFormatDesc); gFormatDesc = NULL; }

    return noErr;
}

static void DAL_ObjectShow(CMIOHardwarePlugInRef self, CMIOObjectID objectID) {}

// ---------- Property Handling ----------

static Boolean DAL_ObjectHasProperty(CMIOHardwarePlugInRef self, CMIOObjectID objectID,
                                     const CMIOObjectPropertyAddress *address) {
    UInt32 sel = address->mSelector;

    if (objectID == gPlugInID) {
        return (sel == kCMIOObjectPropertyName ||
                sel == kCMIOObjectPropertyOwnedObjects);
    }

    if (objectID == gDeviceID) {
        return (sel == kCMIOObjectPropertyName ||
                sel == kCMIOObjectPropertyOwnedObjects ||
                sel == kCMIODevicePropertyDeviceUID ||
                sel == kCMIODevicePropertyModelUID ||
                sel == kCMIODevicePropertyTransportType ||
                sel == kCMIODevicePropertyDeviceIsAlive ||
                sel == kCMIODevicePropertyDeviceHasChanged ||
                sel == kCMIODevicePropertyDeviceIsRunning ||
                sel == kCMIODevicePropertyDeviceIsRunningSomewhere ||
                sel == kCMIODevicePropertyStreams ||
                sel == kCMIOObjectPropertyManufacturer);
    }

    if (objectID == gStreamID) {
        return (sel == kCMIOObjectPropertyName ||
                sel == kCMIOStreamPropertyDirection ||
                sel == kCMIOStreamPropertyTerminalType ||
                sel == kCMIOStreamPropertyStartingChannel ||
                sel == kCMIOStreamPropertyFormatDescription ||
                sel == kCMIOStreamPropertyFormatDescriptions ||
                sel == kCMIOStreamPropertyFrameRate ||
                sel == kCMIOStreamPropertyFrameRates);
    }

    return false;
}

static OSStatus DAL_ObjectIsPropertySettable(CMIOHardwarePlugInRef self, CMIOObjectID objectID,
                                             const CMIOObjectPropertyAddress *address,
                                             Boolean *isSettable) {
    *isSettable = false;
    return noErr;
}

static OSStatus DAL_ObjectGetPropertyDataSize(CMIOHardwarePlugInRef self, CMIOObjectID objectID,
                                               const CMIOObjectPropertyAddress *address,
                                               UInt32 qualifierDataSize, const void *qualifierData,
                                               UInt32 *dataSize) {
    UInt32 sel = address->mSelector;

    // CFString properties
    if (sel == kCMIOObjectPropertyName || sel == kCMIOObjectPropertyManufacturer ||
        sel == kCMIODevicePropertyDeviceUID || sel == kCMIODevicePropertyModelUID) {
        *dataSize = sizeof(CFStringRef);
        return noErr;
    }

    // UInt32 properties
    if (sel == kCMIODevicePropertyTransportType ||
        sel == kCMIOStreamPropertyDirection ||
        sel == kCMIOStreamPropertyTerminalType ||
        sel == kCMIOStreamPropertyStartingChannel ||
        sel == kCMIODevicePropertyDeviceIsAlive ||
        sel == kCMIODevicePropertyDeviceHasChanged ||
        sel == kCMIODevicePropertyDeviceIsRunning ||
        sel == kCMIODevicePropertyDeviceIsRunningSomewhere) {
        *dataSize = sizeof(UInt32);
        return noErr;
    }

    // Owned objects / streams (array of CMIOObjectID)
    if (sel == kCMIOObjectPropertyOwnedObjects || sel == kCMIODevicePropertyStreams) {
        if (objectID == gPlugInID) {
            *dataSize = sizeof(CMIOObjectID);  // 1 device
        } else if (objectID == gDeviceID) {
            *dataSize = sizeof(CMIOObjectID);  // 1 stream
        } else {
            *dataSize = 0;
        }
        return noErr;
    }

    // Format descriptions
    if (sel == kCMIOStreamPropertyFormatDescription) {
        *dataSize = sizeof(CMFormatDescriptionRef);
        return noErr;
    }
    if (sel == kCMIOStreamPropertyFormatDescriptions) {
        *dataSize = sizeof(CFArrayRef);
        return noErr;
    }

    // Frame rate
    if (sel == kCMIOStreamPropertyFrameRate) {
        *dataSize = sizeof(Float64);
        return noErr;
    }
    if (sel == kCMIOStreamPropertyFrameRates) {
        *dataSize = sizeof(CFArrayRef);
        return noErr;
    }

    *dataSize = 0;
    return kCMIOHardwareUnknownPropertyError;
}

static OSStatus DAL_ObjectGetPropertyData(CMIOHardwarePlugInRef self, CMIOObjectID objectID,
                                           const CMIOObjectPropertyAddress *address,
                                           UInt32 qualifierDataSize, const void *qualifierData,
                                           UInt32 dataSize, UInt32 *dataUsed, void *data) {
    UInt32 sel = address->mSelector;

    // Name
    if (sel == kCMIOObjectPropertyName) {
        CFStringRef name;
        if (objectID == gPlugInID)  name = CFSTR("KalidoKit DAL Plugin");
        else if (objectID == gDeviceID)  name = CFSTR("KalidoKit Virtual Camera");
        else if (objectID == gStreamID)  name = CFSTR("KalidoKit Output");
        else return kCMIOHardwareUnknownPropertyError;
        CFRetain(name);
        *(CFStringRef *)data = name;
        *dataUsed = sizeof(CFStringRef);
        return noErr;
    }

    // Manufacturer
    if (sel == kCMIOObjectPropertyManufacturer) {
        CFStringRef mfr = CFSTR("KalidoKit");
        CFRetain(mfr);
        *(CFStringRef *)data = mfr;
        *dataUsed = sizeof(CFStringRef);
        return noErr;
    }

    // Device UID
    if (sel == kCMIODevicePropertyDeviceUID) {
        CFStringRef uid = CFSTR("kalidokit-dal-vcam");
        CFRetain(uid);
        *(CFStringRef *)data = uid;
        *dataUsed = sizeof(CFStringRef);
        return noErr;
    }

    // Model UID
    if (sel == kCMIODevicePropertyModelUID) {
        CFStringRef model = CFSTR("kalidokit-dal-vcam-model");
        CFRetain(model);
        *(CFStringRef *)data = model;
        *dataUsed = sizeof(CFStringRef);
        return noErr;
    }

    // Transport type (built-in)
    if (sel == kCMIODevicePropertyTransportType) {
        *(UInt32 *)data = 0; // unknown/virtual
        *dataUsed = sizeof(UInt32);
        return noErr;
    }

    // Device alive / changed / running
    if (sel == kCMIODevicePropertyDeviceIsAlive) {
        *(UInt32 *)data = 1;
        *dataUsed = sizeof(UInt32);
        return noErr;
    }
    if (sel == kCMIODevicePropertyDeviceHasChanged) {
        *(UInt32 *)data = 0;
        *dataUsed = sizeof(UInt32);
        return noErr;
    }
    if (sel == kCMIODevicePropertyDeviceIsRunning ||
        sel == kCMIODevicePropertyDeviceIsRunningSomewhere) {
        *(UInt32 *)data = gStreamRunning ? 1 : 0;
        *dataUsed = sizeof(UInt32);
        return noErr;
    }

    // Owned objects
    if (sel == kCMIOObjectPropertyOwnedObjects) {
        if (objectID == gPlugInID) {
            *(CMIOObjectID *)data = gDeviceID;
            *dataUsed = sizeof(CMIOObjectID);
        } else if (objectID == gDeviceID) {
            *(CMIOObjectID *)data = gStreamID;
            *dataUsed = sizeof(CMIOObjectID);
        }
        return noErr;
    }

    // Streams
    if (sel == kCMIODevicePropertyStreams) {
        *(CMIOObjectID *)data = gStreamID;
        *dataUsed = sizeof(CMIOObjectID);
        return noErr;
    }

    // Stream direction (0 = output/source)
    if (sel == kCMIOStreamPropertyDirection) {
        *(UInt32 *)data = 0;
        *dataUsed = sizeof(UInt32);
        return noErr;
    }

    // Terminal type
    if (sel == kCMIOStreamPropertyTerminalType) {
        *(UInt32 *)data = kIOAudioOutputPortSubTypeInternalSpeaker; // generic camera
        *dataUsed = sizeof(UInt32);
        return noErr;
    }

    // Starting channel
    if (sel == kCMIOStreamPropertyStartingChannel) {
        *(UInt32 *)data = 0;
        *dataUsed = sizeof(UInt32);
        return noErr;
    }

    // Format description
    if (sel == kCMIOStreamPropertyFormatDescription) {
        if (gFormatDesc) CFRetain(gFormatDesc);
        *(CMFormatDescriptionRef *)data = gFormatDesc;
        *dataUsed = sizeof(CMFormatDescriptionRef);
        return noErr;
    }

    // Format descriptions array
    if (sel == kCMIOStreamPropertyFormatDescriptions) {
        CFArrayRef arr = NULL;
        if (gFormatDesc) {
            arr = CFArrayCreate(kCFAllocatorDefault,
                                (const void **)&gFormatDesc, 1,
                                &kCFTypeArrayCallBacks);
        }
        *(CFArrayRef *)data = arr;
        *dataUsed = sizeof(CFArrayRef);
        return noErr;
    }

    // Frame rate
    if (sel == kCMIOStreamPropertyFrameRate) {
        *(Float64 *)data = (Float64)kFrameRate;
        *dataUsed = sizeof(Float64);
        return noErr;
    }

    // Frame rates array
    if (sel == kCMIOStreamPropertyFrameRates) {
        Float64 rate = (Float64)kFrameRate;
        CFNumberRef num = CFNumberCreate(kCFAllocatorDefault, kCFNumberFloat64Type, &rate);
        CFArrayRef arr = CFArrayCreate(kCFAllocatorDefault, (const void **)&num, 1,
                                       &kCFTypeArrayCallBacks);
        CFRelease(num);
        *(CFArrayRef *)data = arr;
        *dataUsed = sizeof(CFArrayRef);
        return noErr;
    }

    return kCMIOHardwareUnknownPropertyError;
}

static OSStatus DAL_ObjectSetPropertyData(CMIOHardwarePlugInRef self, CMIOObjectID objectID,
                                           const CMIOObjectPropertyAddress *address,
                                           UInt32 qualifierDataSize, const void *qualifierData,
                                           UInt32 dataSize, const void *data) {
    return noErr;
}

// ---------- Device / Stream Control ----------

static OSStatus DAL_DeviceSuspend(CMIOHardwarePlugInRef self, CMIODeviceID device) {
    return noErr;
}

static OSStatus DAL_DeviceResume(CMIOHardwarePlugInRef self, CMIODeviceID device) {
    return noErr;
}

static OSStatus DAL_DeviceStartStream(CMIOHardwarePlugInRef self, CMIODeviceID device,
                                       CMIOStreamID stream) {
    NSLog(@"[KalidoKit DAL] StartStream (device=%u stream=%u)", device, stream);
    gStreamRunning = true;
    startFrameTimer();
    return noErr;
}

static OSStatus DAL_DeviceStopStream(CMIOHardwarePlugInRef self, CMIODeviceID device,
                                      CMIOStreamID stream) {
    NSLog(@"[KalidoKit DAL] StopStream (device=%u stream=%u)", device, stream);
    gStreamRunning = false;
    stopFrameTimer();
    return noErr;
}

static OSStatus DAL_DeviceProcessAVCCommand(CMIOHardwarePlugInRef self, CMIODeviceID device,
                                             CMIODeviceAVCCommand *cmd) {
    return kCMIOHardwareIllegalOperationError;
}

static OSStatus DAL_DeviceProcessRS422Command(CMIOHardwarePlugInRef self, CMIODeviceID device,
                                               CMIODeviceRS422Command *cmd) {
    return kCMIOHardwareIllegalOperationError;
}

static OSStatus DAL_StreamCopyBufferQueue(CMIOHardwarePlugInRef self, CMIOStreamID stream,
                                           CMIODeviceStreamQueueAlteredProc queueAlteredProc,
                                           void *queueAlteredRefCon,
                                           CMSimpleQueueRef *queue) {
    NSLog(@"[KalidoKit DAL] StreamCopyBufferQueue (stream=%u)", stream);

    // Release old queue
    if (gQueue) { CFRelease(gQueue); gQueue = NULL; }

    // Create new queue (capacity = 8 frames)
    OSStatus err = CMSimpleQueueCreate(kCFAllocatorDefault, 8, &gQueue);
    if (err != noErr) return err;

    gQueueAlteredProc = queueAlteredProc;
    gQueueAlteredRefCon = queueAlteredRefCon;

    CFRetain(gQueue);
    *queue = gQueue;

    return noErr;
}

static OSStatus DAL_StreamDeckPlay(CMIOHardwarePlugInRef self, CMIOStreamID stream) {
    return kCMIOHardwareIllegalOperationError;
}
static OSStatus DAL_StreamDeckStop(CMIOHardwarePlugInRef self, CMIOStreamID stream) {
    return kCMIOHardwareIllegalOperationError;
}
static OSStatus DAL_StreamDeckJog(CMIOHardwarePlugInRef self, CMIOStreamID stream, SInt32 speed) {
    return kCMIOHardwareIllegalOperationError;
}
static OSStatus DAL_StreamDeckCueTo(CMIOHardwarePlugInRef self, CMIOStreamID stream,
                                     Float64 frameNumber, Boolean playOnCue) {
    return kCMIOHardwareIllegalOperationError;
}

// ---------- Plugin Interface Table ----------

static CMIOHardwarePlugInInterface gPlugInInterface = {
    // _reserved (IUnknown)
    ._reserved = NULL,
    // IUnknown
    .QueryInterface = DAL_QueryInterface,
    .AddRef = DAL_AddRef,
    .Release = DAL_Release,
    // Plugin
    .Initialize = DAL_Initialize,
    .InitializeWithObjectID = DAL_InitializeWithObjectID,
    .Teardown = DAL_Teardown,
    // Object
    .ObjectShow = DAL_ObjectShow,
    .ObjectHasProperty = DAL_ObjectHasProperty,
    .ObjectIsPropertySettable = DAL_ObjectIsPropertySettable,
    .ObjectGetPropertyDataSize = DAL_ObjectGetPropertyDataSize,
    .ObjectGetPropertyData = DAL_ObjectGetPropertyData,
    .ObjectSetPropertyData = DAL_ObjectSetPropertyData,
    // Device
    .DeviceSuspend = DAL_DeviceSuspend,
    .DeviceResume = DAL_DeviceResume,
    .DeviceStartStream = DAL_DeviceStartStream,
    .DeviceStopStream = DAL_DeviceStopStream,
    .DeviceProcessAVCCommand = DAL_DeviceProcessAVCCommand,
    .DeviceProcessRS422Command = DAL_DeviceProcessRS422Command,
    // Stream
    .StreamCopyBufferQueue = DAL_StreamCopyBufferQueue,
    .StreamDeckPlay = DAL_StreamDeckPlay,
    .StreamDeckStop = DAL_StreamDeckStop,
    .StreamDeckJog = DAL_StreamDeckJog,
    .StreamDeckCueTo = DAL_StreamDeckCueTo,
};

// ---------- Plugin Entry Point ----------

/// CFPlugIn factory function — name must match Info.plist CFPlugInFactories value.
void* KalidoKitDALPlugInMain(CFAllocatorRef allocator, CFUUIDRef requestedTypeUUID) {
    CFUUIDRef plugInTypeID = CFUUIDGetConstantUUIDWithBytes(
        NULL, 0x30, 0x01, 0x0C, 0x1C, 0x93, 0xBF, 0x11, 0xD8,
        0x8B, 0x5B, 0x00, 0x0A, 0x95, 0xAF, 0x9C, 0x6A);

    if (CFEqual(requestedTypeUUID, plugInTypeID)) {
        NSLog(@"[KalidoKit DAL] Factory called — returning plugin interface");
        return gPlugInRef;
    }

    return NULL;
}
