package openphotoai

import _ "embed"

//go:embed libs/linux_arm64/libonnxruntime.so
var libOnnxBinary []byte
var libOnnxName = "libonnxruntime.so"
