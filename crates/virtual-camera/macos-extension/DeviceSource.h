#import <Foundation/Foundation.h>
#import <CoreMediaIO/CoreMediaIO.h>

NS_ASSUME_NONNULL_BEGIN

@interface DeviceSource : NSObject <CMIOExtensionDeviceSource>

- (instancetype)initWithLocalizedName:(NSString *)localizedName;
- (void)addStreamsToDevice:(CMIOExtensionDevice *)device;

@end

NS_ASSUME_NONNULL_END
