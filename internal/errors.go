package internal

import "github.com/cockroachdb/errors"

// ErrCreateSession marks every error returned when building an ONNX session, so callers can tell a session that
// couldn't be built apart from the failures that happen around it - a model that couldn't be downloaded, an operation
// with no model behind it. Test for it with errors.Is, it survives wrapping.
//
// It's what makes the CPU fallback in AcquireModel possible: a session that can't be built is usually the
// execution provider's fault (a broken GPU driver, no free VRAM, an unsupported model/EP combination) and is worth
// retrying elsewhere, while a missing model file would fail the same way on any provider.
//
// It lives here rather than next to CreateSession in internal/utils so that AcquireModel can test for it without
// internal importing its own subpackage.
var ErrCreateSession = errors.New("failed to create ONNX session")
