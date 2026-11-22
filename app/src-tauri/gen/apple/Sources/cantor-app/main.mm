#include "bindings/bindings.h"
#import <AVFoundation/AVFoundation.h>

int main(int argc, char * argv[]) {
	// Rehearsal playback must sound with the silent switch on and keep
	// playing in the background (Info.plist UIBackgroundModes = audio).
	@autoreleasepool {
		NSError *err = nil;
		AVAudioSession *session = [AVAudioSession sharedInstance];
		[session setCategory:AVAudioSessionCategoryPlayback error:&err];
		[session setActive:YES error:&err];
	}
	ffi::start_app();
	return 0;
}
