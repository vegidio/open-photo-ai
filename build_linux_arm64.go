package openphotoai

import _ "embed"

//go:embed libs/linux_arm64/libonnxruntime-1.22.0.so
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.so"
