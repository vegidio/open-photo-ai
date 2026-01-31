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

// IsCoreMLSupported performs a simplified check whether the Mac supported CoreML with MLProgram engine.
//
// Returns false if CoreML is not supported.
func IsCoreMLSupported() bool {
	return int(C.macos_major_version()) >= 12
}
