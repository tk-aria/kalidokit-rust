#import <Foundation/Foundation.h>
#import <CoreMediaIO/CoreMediaIO.h>

NS_ASSUME_NONNULL_BEGIN

@class SinkStreamSource;

@interface DeviceSource : NSObject <CMIOExtensionDeviceSource>

@property (nonatomic, readonly, nullable) SinkStreamSource *sinkStreamSource;

- (instancetype)initWithLocalizedName:(NSString *)localizedName;
- (void)addStreamsToDevice:(CMIOExtensionDevice *)device;

@end

NS_ASSUME_NONNULL_END
