package internal

import "sync/atomic"

// skipModelVerification, when true, makes the library reuse any model file that already exists on
// disk instead of re-downloading it on a hash mismatch or empty/corrupt file. Debug-only; off by
// default. Safe for concurrent use.
var skipModelVerification atomic.Bool

// SetSkipModelVerification toggles the debug behavior. Safe for concurrent use.
func SetSkipModelVerification(skip bool) {
	skipModelVerification.Store(skip)
}

// SkipModelVerification reports whether model hash verification is currently disabled.
func SkipModelVerification() bool {
	return skipModelVerification.Load()
}
