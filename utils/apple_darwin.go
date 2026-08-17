package utils

/*
#cgo CFLAGS: -x objective-c
#cgo LDFLAGS: -framework Foundation
#import <Foundation/Foundation.h>

static int macos_major_version(void) {
    NSOperatingSystemVersion v = [[NSProcessInfo processInfo] operatingSystemVersion];
    return (int)v.majorVersion;
}
*/
import "C"

// IsCoreMLSupported reports whether this Mac is on macOS 12 or later, which is what the MLProgram model format the
// CoreML provider is configured with requires.
func IsCoreMLSupported() bool {
	return int(C.macos_major_version()) >= 12
}
