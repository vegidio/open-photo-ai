package openphotoai

import _ "embed"

//go:embed libs/linux_amd64/libonnxruntime.so
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.so"
