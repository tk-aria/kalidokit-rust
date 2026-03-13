#import "ProviderSource.h"
#import "DeviceSource.h"
#import "SinkStreamSource.h"

@implementation ProviderSource {
    CMIOExtensionProvider *_provider;
    DeviceSource *_deviceSource;
    CMIOExtensionDevice *_device;
}

- (instancetype)initWithClientQueue:(dispatch_queue_t)clientQueue {
    self = [super init];
    if (self) {
        _provider = [[CMIOExtensionProvider alloc] initWithSource:self
                                                     clientQueue:clientQueue];

        _deviceSource = [[DeviceSource alloc] initWithLocalizedName:@"KalidoKit Virtual Camera"];

        NSError *error = nil;
        _device = [[CMIOExtensionDevice alloc] initWithLocalizedName:@"KalidoKit Virtual Camera"
                                                           deviceID:[[NSUUID alloc] initWithUUIDString:@"A8D7B8AA-1001-4001-B001-123456789ABC"]
                                                     legacyDeviceID:nil
                                                             source:_deviceSource];

        // Add output stream + sink stream
        [_deviceSource addStreamsToDevice:_device];

        [_provider addDevice:_device error:&error];
        if (error) {
            NSLog(@"[KalidoKit] Failed to add device: %@", error);
        }
    }
    return self;
}

- (CMIOExtensionProvider *)provider {
    return _provider;
}

#pragma mark - CMIOExtensionProviderSource

- (BOOL)connectClient:(CMIOExtensionClient *)client error:(NSError **)outError {
    NSLog(@"[KalidoKit] Client connected: %@", client);
    // Do NOT subscribe to sink here — subscribe happens in SinkStreamSource's
    // startStreamAndReturnError: (triggered by CMIODeviceStartStream from host).
    return YES;
}

- (void)disconnectClient:(CMIOExtensionClient *)client {
    NSLog(@"[KalidoKit] Client disconnected: %@", client);
}

- (NSSet<CMIOExtensionProperty> *)availableProperties {
    return [NSSet setWithObjects:
            CMIOExtensionPropertyProviderManufacturer,
            CMIOExtensionPropertyProviderName,
            nil];
}

- (nullable CMIOExtensionProviderProperties *)providerPropertiesForProperties:(NSSet<CMIOExtensionProperty> *)properties
                                                                        error:(NSError **)outError {
    CMIOExtensionProviderProperties *props =
        [CMIOExtensionProviderProperties providerPropertiesWithDictionary:@{}];
    if ([properties containsObject:CMIOExtensionPropertyProviderManufacturer]) {
        props.manufacturer = @"KalidoKit";
    }
    if ([properties containsObject:CMIOExtensionPropertyProviderName]) {
        props.name = @"KalidoKit Virtual Camera Provider";
    }
    return props;
}

- (BOOL)setProviderProperties:(CMIOExtensionProviderProperties *)providerProperties
                         error:(NSError **)outError {
    return YES;
}

@end
