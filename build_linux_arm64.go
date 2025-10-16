package openphotoai

import _ "embed"

//go:embed libs/linux_arm64/libonnxruntime.so
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.so"
