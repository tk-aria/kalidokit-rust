// test-extension-install.m
// Minimal test: activate Camera Extension from within a .app bundle.
// This gets compiled and placed as the CFBundleExecutable of a test .app.
//
// Build & run via: scripts/test-extension-activation.sh

#import <Foundation/Foundation.h>
#import <SystemExtensions/SystemExtensions.h>

static NSString *const kExtensionBundleID = @"com.kalidokit.rust.camera-extension";

@interface ExtDelegate : NSObject <OSSystemExtensionRequestDelegate>
@property (nonatomic, assign) BOOL finished;
@property (nonatomic, assign) BOOL success;
@end

@implementation ExtDelegate

- (OSSystemExtensionReplacementAction)request:(OSSystemExtensionRequest *)request
                  actionForReplacingExtension:(OSSystemExtensionProperties *)existing
                                withExtension:(OSSystemExtensionProperties *)ext {
    NSLog(@"[Test] Replacing existing extension");
    return OSSystemExtensionReplacementActionReplace;
}

- (void)request:(OSSystemExtensionRequest *)request didFinishWithResult:(OSSystemExtensionRequestResult)result {
    NSLog(@"[Test] Extension activation finished: result=%ld", (long)result);
    self.success = (result == OSSystemExtensionRequestCompleted ||
                    result == OSSystemExtensionRequestWillCompleteAfterReboot);
    self.finished = YES;
    CFRunLoopStop(CFRunLoopGetMain());
}

- (void)request:(OSSystemExtensionRequest *)request didFailWithError:(NSError *)error {
    NSLog(@"[Test] Extension activation FAILED: %@", error);
    self.success = NO;
    self.finished = YES;
    CFRunLoopStop(CFRunLoopGetMain());
}

- (void)requestNeedsUserApproval:(OSSystemExtensionRequest *)request {
    NSLog(@"[Test] User approval needed — check System Settings > Privacy & Security");
}

@end

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSLog(@"[Test] Bundle: %@", [[NSBundle mainBundle] bundlePath]);
        NSLog(@"[Test] BundleID: %@", [[NSBundle mainBundle] bundleIdentifier]);
        NSLog(@"[Test] Activating extension: %@", kExtensionBundleID);

        ExtDelegate *delegate = [[ExtDelegate alloc] init];
        OSSystemExtensionRequest *request =
            [OSSystemExtensionRequest activationRequestForExtension:kExtensionBundleID
                                                              queue:dispatch_get_main_queue()];
        request.delegate = delegate;
        [[OSSystemExtensionManager sharedManager] submitRequest:request];

        // Run loop for up to 30 seconds
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 30.0, false);

        if (delegate.success) {
            NSLog(@"[Test] SUCCESS — Extension activated!");
            return 0;
        } else if (!delegate.finished) {
            NSLog(@"[Test] TIMEOUT — may need user approval in System Settings");
            return 2;
        } else {
            NSLog(@"[Test] FAILED");
            return 1;
        }
    }
}
