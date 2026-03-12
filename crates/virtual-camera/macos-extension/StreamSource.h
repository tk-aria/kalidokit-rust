#import <Foundation/Foundation.h>
#import <CoreMediaIO/CoreMediaIO.h>
#import <CoreMedia/CoreMedia.h>

NS_ASSUME_NONNULL_BEGIN

@interface StreamSource : NSObject <CMIOExtensionStreamSource>

@property (atomic, readonly) NSArray<CMIOExtensionStreamFormat *> *formats;
@property (nonatomic, weak, nullable) CMIOExtensionStream *stream;

- (instancetype)initWithFormats:(NSArray<CMIOExtensionStreamFormat *> *)formats;
- (void)enqueueBuffer:(CMSampleBufferRef)buffer;

@end

NS_ASSUME_NONNULL_END
