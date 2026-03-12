#import <Foundation/Foundation.h>
#import <CoreMediaIO/CoreMediaIO.h>
#import <CoreMedia/CoreMedia.h>

@class StreamSource;

NS_ASSUME_NONNULL_BEGIN

@interface SinkStreamSource : NSObject <CMIOExtensionStreamSource>

@property (nonatomic, weak, nullable) CMIOExtensionStream *sinkStream;

- (instancetype)initWithFormats:(NSArray<CMIOExtensionStreamFormat *> *)formats
             outputStreamSource:(StreamSource *)outputStreamSource;

@end

NS_ASSUME_NONNULL_END
