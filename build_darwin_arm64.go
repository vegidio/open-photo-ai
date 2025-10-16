package openphotoai

import _ "embed"

//go:embed libs/darwin_arm64/libonnxruntime.dylib
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.dylib"
