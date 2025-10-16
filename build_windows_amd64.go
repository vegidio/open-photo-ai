package openphotoai

import _ "embed"

//go:embed libs/windows_amd64/libonnxruntime.dll
var libOnnxBinary []byte
var libOnnxName = "libonnxruntime.dll"
