package internal

// OnnxRuntimeName is the shared library inside the release archive for this platform. The archive itself is verified
// against a pinned hash (see Artifacts), so this only has to name the file the loader is pointed at.
const OnnxRuntimeName = "onnxruntime-1.26.0.dll"
