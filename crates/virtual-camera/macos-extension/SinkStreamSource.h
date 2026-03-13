#import <Foundation/Foundation.h>
#import <CoreMediaIO/CoreMediaIO.h>
#import <CoreMedia/CoreMedia.h>

@class StreamSource;

NS_ASSUME_NONNULL_BEGIN

@interface SinkStreamSource : NSObject <CMIOExtensionStreamSource>

@property (nonatomic, weak, nullable) CMIOExtensionStream *sinkStream;

- (instancetype)initWithFormats:(NSArray<CMIOExtensionStreamFormat *> *)formats
             outputStreamSource:(StreamSource *)outputStreamSource;

/// Start consuming sample buffers from the given client via the sink stream.
- (void)subscribeWithClient:(CMIOExtensionClient *)client;

@end

NS_ASSUME_NONNULL_END
