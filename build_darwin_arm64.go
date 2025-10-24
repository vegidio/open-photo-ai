package openphotoai

import _ "embed"

//go:embed libs/darwin_arm64/libonnxruntime-1.22.0.dylib
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.dylib"
