package openphotoai

import _ "embed"

//go:embed libs/darwin_amd64/libonnxruntime.dylib
var libOnnxBinary []byte
var libOnnxName = "libonnxruntime.dylib"
