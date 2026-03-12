#import "DeviceSource.h"
#import "StreamSource.h"
#import "SinkStreamSource.h"

static NSUUID *kOutputStreamID;
static NSUUID *kSinkStreamID;

@implementation DeviceSource {
    NSString *_localizedName;
    StreamSource *_outputStreamSource;
    SinkStreamSource *_sinkStreamSource;
    CMIOExtensionStream *_outputStream;
    CMIOExtensionStream *_sinkStream;
}

+ (void)initialize {
    kOutputStreamID = [[NSUUID alloc] initWithUUIDString:@"A8D7B8AA-2001-4001-B001-123456789ABC"];
    kSinkStreamID = [[NSUUID alloc] initWithUUIDString:@"A8D7B8AA-3001-4001-B001-123456789ABC"];
}

- (instancetype)initWithLocalizedName:(NSString *)localizedName {
    self = [super init];
    if (self) {
        _localizedName = localizedName;
    }
    return self;
}

- (void)addStreamsToDevice:(CMIOExtensionDevice *)device {
    // Video format: 1280x720 BGRA 30fps
    CMVideoDimensions dims = { .width = 1280, .height = 720 };
    CMFormatDescriptionRef formatDesc = [self createFormatDescriptionWithDimensions:dims];
    CMIOExtensionStreamFormat *format =
        [[CMIOExtensionStreamFormat alloc] initWithFormatDescription:formatDesc
                                                    maxFrameDuration:CMTimeMake(1, 30)
                                                    minFrameDuration:CMTimeMake(1, 30)
                                                  validFrameDurations:nil];
    CFRelease(formatDesc);

    // Output stream (consumed by FaceTime, Zoom, etc.)
    _outputStreamSource = [[StreamSource alloc] initWithFormats:@[format]];
    _outputStream = [[CMIOExtensionStream alloc] initWithLocalizedName:@"KalidoKit Output"
                                                             streamID:kOutputStreamID
                                                            direction:CMIOExtensionStreamDirectionSource
                                                            clockType:CMIOExtensionStreamClockTypeHostTime
                                                               source:_outputStreamSource];
    _outputStreamSource.stream = _outputStream;

    NSError *error = nil;
    [device addStream:_outputStream error:&error];
    if (error) {
        NSLog(@"[KalidoKit] Failed to add output stream: %@", error);
    }

    // Sink stream (receives frames from host app)
    _sinkStreamSource = [[SinkStreamSource alloc] initWithFormats:@[format]
                                               outputStreamSource:_outputStreamSource];
    _sinkStream = [[CMIOExtensionStream alloc] initWithLocalizedName:@"KalidoKit Sink"
                                                            streamID:kSinkStreamID
                                                           direction:CMIOExtensionStreamDirectionSink
                                                           clockType:CMIOExtensionStreamClockTypeHostTime
                                                              source:_sinkStreamSource];
    _sinkStreamSource.sinkStream = _sinkStream;

    [device addStream:_sinkStream error:&error];
    if (error) {
        NSLog(@"[KalidoKit] Failed to add sink stream: %@", error);
    }
}

- (CMFormatDescriptionRef)createFormatDescriptionWithDimensions:(CMVideoDimensions)dims {
    CMFormatDescriptionRef formatDesc = NULL;
    CMVideoFormatDescriptionCreate(kCFAllocatorDefault,
                                   kCVPixelFormatType_32BGRA,
                                   dims.width, dims.height,
                                   NULL, &formatDesc);
    return formatDesc;
}

#pragma mark - CMIOExtensionDeviceSource

- (NSSet<CMIOExtensionProperty> *)availableProperties {
    return [NSSet setWithObjects:
            CMIOExtensionPropertyDeviceTransportType,
            CMIOExtensionPropertyDeviceModel,
            nil];
}

- (nullable CMIOExtensionDeviceProperties *)devicePropertiesForProperties:(NSSet<CMIOExtensionProperty> *)properties
                                                                    error:(NSError **)outError {
    CMIOExtensionDeviceProperties *props =
        [CMIOExtensionDeviceProperties devicePropertiesWithDictionary:@{}];
    if ([properties containsObject:CMIOExtensionPropertyDeviceTransportType]) {
        // 'bltn' = kIOAudioDeviceTransportTypeBuiltIn = 0x626C746E
        props.transportType = @(0x626C746E);
    }
    if ([properties containsObject:CMIOExtensionPropertyDeviceModel]) {
        props.model = @"KalidoKit Virtual Camera";
    }
    return props;
}

- (BOOL)setDeviceProperties:(CMIOExtensionDeviceProperties *)deviceProperties
                       error:(NSError **)outError {
    return YES;
}

@end
