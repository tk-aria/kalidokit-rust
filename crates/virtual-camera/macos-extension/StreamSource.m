#import "StreamSource.h"

@implementation StreamSource {
    NSArray<CMIOExtensionStreamFormat *> *_formats;
}

- (instancetype)initWithFormats:(NSArray<CMIOExtensionStreamFormat *> *)formats {
    self = [super init];
    if (self) {
        _formats = formats;
    }
    return self;
}

- (void)enqueueBuffer:(CMSampleBufferRef)buffer {
    CMIOExtensionStream *stream = self.stream;
    if (stream) {
        CMTime pts = CMSampleBufferGetPresentationTimeStamp(buffer);
        uint64_t hostTimeNs = (uint64_t)(CMTimeGetSeconds(pts) * 1e9);
        [stream sendSampleBuffer:buffer
                   discontinuity:CMIOExtensionStreamDiscontinuityFlagNone
           hostTimeInNanoseconds:hostTimeNs];
    }
}

#pragma mark - CMIOExtensionStreamSource

- (NSArray<CMIOExtensionStreamFormat *> *)formats {
    return _formats;
}

- (NSSet<CMIOExtensionProperty> *)availableProperties {
    return [NSSet setWithObjects:
            CMIOExtensionPropertyStreamActiveFormatIndex,
            CMIOExtensionPropertyStreamFrameDuration,
            nil];
}

- (nullable CMIOExtensionStreamProperties *)streamPropertiesForProperties:(NSSet<CMIOExtensionProperty> *)properties
                                                                    error:(NSError **)outError {
    CMIOExtensionStreamProperties *props =
        [CMIOExtensionStreamProperties streamPropertiesWithDictionary:@{}];
    if ([properties containsObject:CMIOExtensionPropertyStreamActiveFormatIndex]) {
        props.activeFormatIndex = @0;
    }
    if ([properties containsObject:CMIOExtensionPropertyStreamFrameDuration]) {
        CMTime dur = CMTimeMake(1, 30);
        props.frameDuration = @{
            @"value": @(dur.value),
            @"timescale": @(dur.timescale),
            @"flags": @(dur.flags),
            @"epoch": @(dur.epoch),
        };
    }
    return props;
}

- (BOOL)setStreamProperties:(CMIOExtensionStreamProperties *)streamProperties
                       error:(NSError **)outError {
    return YES;
}

- (BOOL)authorizedToStartStreamForClient:(CMIOExtensionClient *)client {
    return YES;
}

- (BOOL)startStreamAndReturnError:(NSError **)outError {
    NSLog(@"[KalidoKit] Output stream started");
    return YES;
}

- (BOOL)stopStreamAndReturnError:(NSError **)outError {
    NSLog(@"[KalidoKit] Output stream stopped");
    return YES;
}

@end
