package internal

// AppName is the name of the application using Open Photo AI's library.
//
// This name is used to create a dedicated config directory for the application, where the ONNX runtime, model files and
// their dependencies are stored, under the user's configuration path. This variable is set by the Initialize() function
// and should never be changed directly.
var AppName = "open-photo-ai"

// Registry is where all loaded models are stored.
//
// This variable is set by the `selectModel` function and should never be changed directly.
var Registry = make(map[string]interface{})
