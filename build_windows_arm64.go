package openphotoai

import _ "embed"

//go:embed libs/windows_arm64/libonnxruntime-1.22.0.dll
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.dll"
