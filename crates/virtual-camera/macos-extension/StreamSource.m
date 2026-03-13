#import "StreamSource.h"
#import <mach/mach_time.h>
#import <sys/socket.h>
#import <netinet/in.h>
#import <netinet/tcp.h>
#import <arpa/inet.h>
#import <unistd.h>
#import <fcntl.h>
#import <string.h>
#import <errno.h>

/// TCP port for host → extension frame transfer on localhost.
/// Host runs the server, Extension connects as client.
/// Must match Rust host side.
static const uint16_t kTcpPort = 19876;

/// Frame header: [width: u32 LE][height: u32 LE] = 8 bytes
static const size_t kFrameHeaderSize = 8;

@implementation StreamSource {
    NSArray<CMIOExtensionStreamFormat *> *_formats;
    dispatch_source_t _timer;
    uint64_t _frameCounter;
    dispatch_queue_t _timerQueue;

    // TCP client connection to host
    int _clientFd;
    dispatch_source_t _readSource;
    dispatch_queue_t _netQueue;
    dispatch_source_t _connectTimer;

    // Latest frame received from host
    NSData *_latestFrameData;
    uint32_t _latestWidth;
    uint32_t _latestHeight;
    dispatch_queue_t _frameQueue;

    // Read buffer for TCP stream reassembly
    NSMutableData *_readBuffer;
    BOOL _streaming;
}

- (instancetype)initWithFormats:(NSArray<CMIOExtensionStreamFormat *> *)formats {
    self = [super init];
    if (self) {
        _formats = formats;
        _clientFd = -1;
        _frameQueue = dispatch_queue_create("com.kalidokit.vcam.frame", DISPATCH_QUEUE_SERIAL);
        _netQueue = dispatch_queue_create("com.kalidokit.vcam.net", DISPATCH_QUEUE_SERIAL);
        _readBuffer = [NSMutableData data];
        _streaming = YES;

        // Start TCP connection immediately (don't wait for startStreamAndReturnError
        // which CMIO may never call on proxy processes)
        [self tryConnect];
        [self startConnectTimer];

        // Start frame timer after a short delay to allow self.stream to be set
        __weak typeof(self) weakSelf = self;
        dispatch_queue_t delayQueue = dispatch_queue_create("com.kalidokit.vcam.delay", DISPATCH_QUEUE_SERIAL);
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.5 * NSEC_PER_SEC)),
                       delayQueue, ^{
            __strong typeof(weakSelf) strongSelf = weakSelf;
            if (!strongSelf) return;
            [strongSelf startFrameTimer];
        });
    }
    return self;
}

- (void)startFrameTimer {
    if (_timer) return;
    NSLog(@"[KalidoKit] Starting frame timer (stream=%@)", self.stream ? @"set" : @"nil");
    _timerQueue = dispatch_queue_create("com.kalidokit.vcam.timer", DISPATCH_QUEUE_SERIAL);
    _timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, _timerQueue);
    dispatch_source_set_timer(_timer, dispatch_time(DISPATCH_TIME_NOW, 0),
                              (uint64_t)(1.0 / 30.0 * NSEC_PER_SEC), NSEC_PER_MSEC);
    __weak typeof(self) weakSelf = self;
    dispatch_source_set_event_handler(_timer, ^{
        __strong typeof(weakSelf) strongSelf = weakSelf;
        if (!strongSelf) return;
        [strongSelf pollAndSendFrame];
    });
    dispatch_resume(_timer);
}

#pragma mark - TCP Client

- (BOOL)tryConnect {
    if (_clientFd >= 0) return YES;

    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return NO;

    struct sockaddr_in addr = {0};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(kTcpPort);
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);

    if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        close(fd);
        return NO;
    }

    // Set non-blocking and TCP_NODELAY
    int flags = fcntl(fd, F_GETFL, 0);
    fcntl(fd, F_SETFL, flags | O_NONBLOCK);
    int flag = 1;
    setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &flag, sizeof(flag));
    int rcvbuf = 16 * 1024 * 1024;
    setsockopt(fd, SOL_SOCKET, SO_RCVBUF, &rcvbuf, sizeof(rcvbuf));

    _clientFd = fd;
    [_readBuffer setLength:0];

    NSLog(@"[KalidoKit] Connected to host TCP server on 127.0.0.1:%u", kTcpPort);

    // Setup read dispatch source
    _readSource = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ, fd, 0, _netQueue);
    __weak typeof(self) weakSelf = self;
    dispatch_source_set_event_handler(_readSource, ^{
        __strong typeof(weakSelf) strongSelf = weakSelf;
        if (!strongSelf) return;
        [strongSelf readFromHost];
    });
    dispatch_source_set_cancel_handler(_readSource, ^{
        __strong typeof(weakSelf) strongSelf = weakSelf;
        if (strongSelf && strongSelf->_clientFd == fd) {
            close(fd);
            strongSelf->_clientFd = -1;
            NSLog(@"[KalidoKit] Disconnected from host");
        }
    });
    dispatch_resume(_readSource);

    return YES;
}

- (void)disconnect {
    if (_readSource) {
        dispatch_source_cancel(_readSource);
        _readSource = nil;
    }
    if (_clientFd >= 0) {
        close(_clientFd);
        _clientFd = -1;
    }
}

- (void)startConnectTimer {
    if (_connectTimer) return;

    _connectTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, _netQueue);
    dispatch_source_set_timer(_connectTimer, dispatch_time(DISPATCH_TIME_NOW, 0),
                              (uint64_t)(1.0 * NSEC_PER_SEC), NSEC_PER_MSEC);  // retry every 1s

    __weak typeof(self) weakSelf = self;
    dispatch_source_set_event_handler(_connectTimer, ^{
        __strong typeof(weakSelf) strongSelf = weakSelf;
        if (!strongSelf || !strongSelf->_streaming) return;
        if (strongSelf->_clientFd < 0) {
            [strongSelf tryConnect];
        }
    });
    dispatch_resume(_connectTimer);
}

- (void)stopConnectTimer {
    if (_connectTimer) {
        dispatch_source_cancel(_connectTimer);
        _connectTimer = nil;
    }
}

- (void)readFromHost {
    uint8_t tmp[64 * 1024];  // 64KB read chunks (must fit in GCD 512KB stack)

    // Drain all available data (non-blocking socket)
    while (1) {
        ssize_t n = read(_clientFd, tmp, sizeof(tmp));
        if (n < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) break;
            // Real error — close connection
            if (_readSource) { dispatch_source_cancel(_readSource); _readSource = nil; }
            return;
        }
        if (n == 0) {
            // EOF — host disconnected
            if (_readSource) { dispatch_source_cancel(_readSource); _readSource = nil; }
            return;
        }
        [_readBuffer appendBytes:tmp length:n];
    }

    // Process complete frames from buffer
    while (_readBuffer.length >= kFrameHeaderSize) {
        const uint8_t *buf = (const uint8_t *)_readBuffer.bytes;
        uint32_t width, height;
        memcpy(&width, buf + 0, 4);
        memcpy(&height, buf + 4, 4);

        if (width == 0 || height == 0 || width > 8192 || height > 8192) {
            NSLog(@"[KalidoKit] Invalid frame header: %ux%u — resetting buffer", width, height);
            [_readBuffer setLength:0];
            return;
        }

        size_t frameSize = kFrameHeaderSize + (size_t)width * height * 4;
        if (_readBuffer.length < frameSize) break;  // incomplete frame

        NSData *pixelData = [NSData dataWithBytes:buf + kFrameHeaderSize
                                           length:(size_t)width * height * 4];

        dispatch_sync(_frameQueue, ^{
            self->_latestFrameData = pixelData;
            self->_latestWidth = width;
            self->_latestHeight = height;
        });

        [_readBuffer replaceBytesInRange:NSMakeRange(0, frameSize) withBytes:NULL length:0];
    }
}

#pragma mark - CMIOExtensionStreamSource

- (NSArray<CMIOExtensionStreamFormat *> *)formats { return _formats; }

- (NSSet<CMIOExtensionProperty> *)availableProperties {
    return [NSSet setWithObjects:
            CMIOExtensionPropertyStreamActiveFormatIndex,
            CMIOExtensionPropertyStreamFrameDuration, nil];
}

- (nullable CMIOExtensionStreamProperties *)streamPropertiesForProperties:(NSSet<CMIOExtensionProperty> *)properties
                                                                    error:(NSError **)outError {
    CMIOExtensionStreamProperties *props = [CMIOExtensionStreamProperties streamPropertiesWithDictionary:@{}];
    if ([properties containsObject:CMIOExtensionPropertyStreamActiveFormatIndex]) {
        props.activeFormatIndex = @0;
    }
    if ([properties containsObject:CMIOExtensionPropertyStreamFrameDuration]) {
        CMTime dur = CMTimeMake(1, 30);
        props.frameDuration = @{ @"value": @(dur.value), @"timescale": @(dur.timescale),
                                 @"flags": @(dur.flags), @"epoch": @(dur.epoch) };
    }
    return props;
}

- (BOOL)setStreamProperties:(CMIOExtensionStreamProperties *)streamProperties error:(NSError **)outError { return YES; }
- (BOOL)authorizedToStartStreamForClient:(CMIOExtensionClient *)client { return YES; }

- (BOOL)startStreamAndReturnError:(NSError **)outError {
    NSLog(@"[KalidoKit] Output stream started (CMIO called startStream)");
    _streaming = YES;
    // Ensure timer and TCP are running (idempotent)
    [self startConnectTimer];
    [self startFrameTimer];
    return YES;
}

- (void)pollAndSendFrame {
    CMIOExtensionStream *stream = self.stream;
    if (!stream) return;

    __block NSData *frameData = nil;
    __block uint32_t width = 0, height = 0;

    dispatch_sync(_frameQueue, ^{
        frameData = self->_latestFrameData;
        width = self->_latestWidth;
        height = self->_latestHeight;
    });

    if (!frameData || width == 0 || height == 0) {
        [self generateTestFrame];
        return;
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
        .duration = CMTimeMake(1, 30),
        .presentationTimeStamp = pts,
        .decodeTimeStamp = kCMTimeInvalid
    };
    CMSampleBufferRef sampleBuffer = NULL;
    OSStatus sbStatus = CMSampleBufferCreateReadyWithImageBuffer(kCFAllocatorDefault, pixelBuffer,
                                              formatDesc, &timing, &sampleBuffer);
    CFRelease(formatDesc);
    CVPixelBufferRelease(pixelBuffer);

    if (sampleBuffer && sbStatus == noErr) {
        [stream sendSampleBuffer:sampleBuffer
                   discontinuity:CMIOExtensionStreamDiscontinuityFlagNone
           hostTimeInNanoseconds:hostTimeNs];
        CFRelease(sampleBuffer);

        if (_frameCounter % 30 == 0) {
            NSLog(@"[KalidoKit] TCP frame %llu (%ux%u)", _frameCounter, width, height);
        }
    }
    _frameCounter++;
}

- (void)generateTestFrame {
    CMIOExtensionStream *stream = self.stream;
    if (!stream) return;

    int width = 1280, height = 720;
    CVPixelBufferRef pixelBuffer = NULL;
    NSDictionary *attrs = @{ (NSString *)kCVPixelBufferIOSurfacePropertiesKey: @{} };
    if (CVPixelBufferCreate(kCFAllocatorDefault, width, height, kCVPixelFormatType_32BGRA,
                            (__bridge CFDictionaryRef)attrs, &pixelBuffer) != kCVReturnSuccess) return;

    CVPixelBufferLockBaseAddress(pixelBuffer, 0);
    uint8_t *base = CVPixelBufferGetBaseAddress(pixelBuffer);
    size_t bpr = CVPixelBufferGetBytesPerRow(pixelBuffer);
    int phase = (_frameCounter / 60) % 3;
    uint8_t r = (phase == 0) ? 200 : 0, g = (phase == 1) ? 200 : 0, b = (phase == 2) ? 200 : 0;
    for (int y = 0; y < height; y++) {
        uint8_t *row = base + y * bpr;
        for (int x = 0; x < width; x++) {
            row[x*4+0] = b; row[x*4+1] = g; row[x*4+2] = r; row[x*4+3] = 255;
        }
    }
    CVPixelBufferUnlockBaseAddress(pixelBuffer, 0);

    CMVideoFormatDescriptionRef fd = NULL;
    CMVideoFormatDescriptionCreateForImageBuffer(kCFAllocatorDefault, pixelBuffer, &fd);
    if (!fd) { CVPixelBufferRelease(pixelBuffer); return; }

    uint64_t hostTimeNs = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
    CMSampleTimingInfo timing = { CMTimeMake(1,30), CMTimeMake((int64_t)hostTimeNs,1000000000), kCMTimeInvalid };
    CMSampleBufferRef sb = NULL;
    if (CMSampleBufferCreateReadyWithImageBuffer(kCFAllocatorDefault, pixelBuffer, fd, &timing, &sb) == noErr && sb) {
        [stream sendSampleBuffer:sb discontinuity:CMIOExtensionStreamDiscontinuityFlagNone hostTimeInNanoseconds:hostTimeNs];
        CFRelease(sb);
    }
    CFRelease(fd);
    CVPixelBufferRelease(pixelBuffer);
    _frameCounter++;
}

- (BOOL)stopStreamAndReturnError:(NSError **)outError {
    NSLog(@"[KalidoKit] Output stream stopped (CMIO called stopStream)");
    // Keep TCP and timer running — proxy may be restarted without startStream being called again
    return YES;
}

@end
