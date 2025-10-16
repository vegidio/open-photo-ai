package openphotoai

import _ "embed"

//go:embed libs/darwin_amd64/libonnxruntime.dylib
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.dylib"
