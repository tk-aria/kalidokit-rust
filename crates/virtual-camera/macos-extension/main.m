#import <Foundation/Foundation.h>
#import <CoreMediaIO/CoreMediaIO.h>
#import "ProviderSource.h"

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        ProviderSource *source = [[ProviderSource alloc] initWithClientQueue:nil];
        [CMIOExtensionProvider startServiceWithProvider:source.provider];
        CFRunLoopRun();
    }
    return 0;
}
