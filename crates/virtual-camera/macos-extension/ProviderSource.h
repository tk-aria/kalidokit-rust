#import <Foundation/Foundation.h>
#import <CoreMediaIO/CoreMediaIO.h>

NS_ASSUME_NONNULL_BEGIN

@interface ProviderSource : NSObject <CMIOExtensionProviderSource>

@property (nonatomic, strong, readonly) CMIOExtensionProvider *provider;

- (instancetype)initWithClientQueue:(nullable dispatch_queue_t)clientQueue;

@end

NS_ASSUME_NONNULL_END
