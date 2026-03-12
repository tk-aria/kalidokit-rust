// install-extension.m
// Minimal tool to activate the KalidoKit Camera Extension via OSSystemExtensionManager.
// Build: clang -fobjc-arc -fmodules -framework SystemExtensions -framework Foundation -o install-extension install-extension.m
// Run:   ./install-extension

#import <Foundation/Foundation.h>
#import <SystemExtensions/SystemExtensions.h>

static NSString *const kExtensionBundleID = @"com.kalidokit.rust.camera-extension";

@interface ExtensionDelegate : NSObject <OSSystemExtensionRequestDelegate>
@property (nonatomic, assign) BOOL finished;
@property (nonatomic, assign) BOOL success;
@end

@implementation ExtensionDelegate

- (OSSystemExtensionReplacementAction)request:(OSSystemExtensionRequest *)request
                  actionForReplacingExtension:(OSSystemExtensionProperties *)existing
                                withExtension:(OSSystemExtensionProperties *)ext {
    NSLog(@"[KalidoKit] Replacing existing extension");
    return OSSystemExtensionReplacementActionReplace;
}

- (void)request:(OSSystemExtensionRequest *)request didFinishWithResult:(OSSystemExtensionRequestResult)result {
    NSLog(@"[KalidoKit] Extension activation finished: result=%ld", (long)result);
    self.success = (result == OSSystemExtensionRequestCompleted ||
                    result == OSSystemExtensionRequestWillCompleteAfterReboot);
    self.finished = YES;
    CFRunLoopStop(CFRunLoopGetMain());
}

- (void)request:(OSSystemExtensionRequest *)request didFailWithError:(NSError *)error {
    NSLog(@"[KalidoKit] Extension activation failed: %@", error);
    self.success = NO;
    self.finished = YES;
    CFRunLoopStop(CFRunLoopGetMain());
}

- (void)requestNeedsUserApproval:(OSSystemExtensionRequest *)request {
    NSLog(@"[KalidoKit] User approval needed — check System Settings > Privacy & Security");
}

@end

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSLog(@"[KalidoKit] Activating Camera Extension: %@", kExtensionBundleID);

        ExtensionDelegate *delegate = [[ExtensionDelegate alloc] init];
        OSSystemExtensionRequest *request =
            [OSSystemExtensionRequest activationRequestForExtension:kExtensionBundleID
                                                              queue:dispatch_get_main_queue()];
        request.delegate = delegate;

        [[OSSystemExtensionManager sharedManager] submitRequest:request];

        // Run until delegate callback fires
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 30.0, false);

        if (delegate.success) {
            NSLog(@"[KalidoKit] Extension activated successfully!");
            return 0;
        } else {
            NSLog(@"[KalidoKit] Extension activation failed or timed out.");
            return 1;
        }
    }
}
