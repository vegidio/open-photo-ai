package opai

import "github.com/vegidio/open-photo-ai/internal"

// SetSkipModelVerification enables a debug mode where models already present on disk are used as-is.
//
// Normally a locally-present model whose SHA-256 doesn't match the expected Hugging Face hash (or an
// empty/corrupt file) is re-downloaded. With this enabled, a model is downloaded ONLY when it is
// missing — a different hash or an empty file no longer triggers a re-download. Intended for
// debugging with hand-built/experimental models; leave it off in production. Can be called any time
// after Initialize.
func SetSkipModelVerification(skip bool) {
	internal.SetSkipModelVerification(skip)
}
