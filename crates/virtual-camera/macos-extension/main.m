#import <Foundation/Foundation.h>
#import <CoreMediaIO/CoreMediaIO.h>
#import "ProviderSource.h"

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSLog(@"[KalidoKit] Camera Extension starting...");
        NSLog(@"[KalidoKit] Bundle: %@", [[NSBundle mainBundle] bundlePath]);
        NSLog(@"[KalidoKit] BundleID: %@", [[NSBundle mainBundle] bundleIdentifier]);

        ProviderSource *source = [[ProviderSource alloc] initWithClientQueue:nil];
        NSLog(@"[KalidoKit] Provider created, starting service...");

        [CMIOExtensionProvider startServiceWithProvider:source.provider];
        NSLog(@"[KalidoKit] Service started, entering run loop");

        CFRunLoopRun();
    }
    return 0;
}
