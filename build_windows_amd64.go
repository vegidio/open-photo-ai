package openphotoai

import _ "embed"

//go:embed libs/windows_amd64/libonnxruntime.dll
var onnxRuntimeBinary []byte
var onnxRuntimeName = "libonnxruntime.dll"
