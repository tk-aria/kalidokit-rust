#import "SinkStreamSource.h"
#import "StreamSource.h"

@implementation SinkStreamSource {
    NSArray<CMIOExtensionStreamFormat *> *_formats;
    StreamSource *_outputStreamSource;
    CMIOExtensionClient *_connectedClient;
}

- (instancetype)initWithFormats:(NSArray<CMIOExtensionStreamFormat *> *)formats
             outputStreamSource:(StreamSource *)outputStreamSource {
    self = [super init];
    if (self) {
        _formats = formats;
        _outputStreamSource = outputStreamSource;
    }
    return self;
}

- (void)subscribeWithClient:(CMIOExtensionClient *)client {
    __weak typeof(self) weakSelf = self;
    CMIOExtensionStream *stream = self.sinkStream;
    if (!stream || !client) return;

    [stream consumeSampleBufferFromClient:client
                        completionHandler:^(CMSampleBufferRef _Nullable buffer,
                                            uint64_t sequenceNumber,
                                            CMIOExtensionStreamDiscontinuityFlags flags,
                                            BOOL hasMoreSampleBuffers,
                                            NSError * _Nullable error) {
        __strong typeof(weakSelf) strongSelf = weakSelf;
        if (!strongSelf) return;

        if (buffer && !error) {
            // Forward received buffer to output stream
            [strongSelf->_outputStreamSource enqueueBuffer:buffer];
        }

        // Re-subscribe to continue receiving frames (UniCamEx pattern)
        dispatch_async(dispatch_get_main_queue(), ^{
            [strongSelf subscribeWithClient:client];
        });
    }];
}

#pragma mark - CMIOExtensionStreamSource

- (NSArray<CMIOExtensionStreamFormat *> *)formats {
    return _formats;
}

- (NSSet<CMIOExtensionProperty> *)availableProperties {
    return [NSSet setWithObjects:
            CMIOExtensionPropertyStreamActiveFormatIndex,
            CMIOExtensionPropertyStreamFrameDuration,
            CMIOExtensionPropertyStreamSinkBufferQueueSize,
            CMIOExtensionPropertyStreamSinkBuffersRequiredForStartup,
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
    if ([properties containsObject:CMIOExtensionPropertyStreamSinkBufferQueueSize]) {
        props.sinkBufferQueueSize = @1;
    }
    if ([properties containsObject:CMIOExtensionPropertyStreamSinkBuffersRequiredForStartup]) {
        props.sinkBuffersRequiredForStartup = @1;
    }
    return props;
}

- (BOOL)setStreamProperties:(CMIOExtensionStreamProperties *)streamProperties
                       error:(NSError **)outError {
    return YES;
}

- (BOOL)authorizedToStartStreamForClient:(CMIOExtensionClient *)client {
    _connectedClient = client;
    return YES;
}

- (BOOL)startStreamAndReturnError:(NSError **)outError {
    NSLog(@"[KalidoKit] Sink stream started, subscribing for buffers");
    if (_connectedClient) {
        [self subscribeWithClient:_connectedClient];
    }
    return YES;
}

- (BOOL)stopStreamAndReturnError:(NSError **)outError {
    NSLog(@"[KalidoKit] Sink stream stopped");
    _connectedClient = nil;
    return YES;
}

@end
