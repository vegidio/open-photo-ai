package openphotoai

import _ "embed"

//go:embed libs/linux_amd64/libonnxruntime.so
var libOnnxBinary []byte
var libOnnxName = "libonnxruntime.so"
