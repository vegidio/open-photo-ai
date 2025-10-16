package openphotoai

import _ "embed"

//go:embed libs/windows_arm64/libonnxruntime.dll
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.dll"
