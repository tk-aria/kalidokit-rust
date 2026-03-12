#import "ExtensionInstaller.h"
#import <SystemExtensions/SystemExtensions.h>

static NSString *const kExtensionBundleID = @"com.kalidokit.rust.camera-extension";

@interface KalidoKitExtensionDelegate : NSObject <OSSystemExtensionRequestDelegate>
@end

@implementation KalidoKitExtensionDelegate

- (OSSystemExtensionReplacementAction)request:(OSSystemExtensionRequest *)request
                  actionForReplacingExtension:(OSSystemExtensionProperties *)existing
                                withExtension:(OSSystemExtensionProperties *)ext {
    NSLog(@"[KalidoKit] Replacing existing camera extension");
    return OSSystemExtensionReplacementActionReplace;
}

- (void)request:(OSSystemExtensionRequest *)request didFinishWithResult:(OSSystemExtensionRequestResult)result {
    if (result == OSSystemExtensionRequestCompleted) {
        NSLog(@"[KalidoKit] Camera extension activated successfully!");
    } else {
        NSLog(@"[KalidoKit] Camera extension activation result: %ld (may require reboot)", (long)result);
    }
}

- (void)request:(OSSystemExtensionRequest *)request didFailWithError:(NSError *)error {
    NSLog(@"[KalidoKit] Camera extension activation failed: %@", error);
}

- (void)requestNeedsUserApproval:(OSSystemExtensionRequest *)request {
    NSLog(@"[KalidoKit] User approval needed — check System Settings > Privacy & Security");
}

@end

// Static reference to keep delegate alive
static KalidoKitExtensionDelegate *_delegate = nil;

void KalidoKitInstallCameraExtension(void) {
    NSLog(@"[KalidoKit] Requesting camera extension activation: %@", kExtensionBundleID);
    _delegate = [[KalidoKitExtensionDelegate alloc] init];
    OSSystemExtensionRequest *request =
        [OSSystemExtensionRequest activationRequestForExtension:kExtensionBundleID
                                                          queue:dispatch_get_main_queue()];
    request.delegate = _delegate;
    [[OSSystemExtensionManager sharedManager] submitRequest:request];
}
