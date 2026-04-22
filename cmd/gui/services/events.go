package services

import (
	guitypes "gui/types"

	"github.com/wailsapp/wails/v3/pkg/application"
)

// Event names emitted between the Go backend and the frontend.
//
// Each custom event is registered with application.RegisterEvent[T] in init() below. The Wails
// binding generator picks up these registrations and augments the TypeScript Events.CustomEvents
// interface in bindings/…/eventdata.d.ts, giving frontend callers fully typed event.data and
// key-level autocomplete. Do not emit or listen on a raw string outside this file — use these
// constants so backend and frontend stay in sync.
const (
	EventAppDownload     = "app:download"
	EventAppProgress     = "app:progress"
	EventAppExport       = "app:export"
	EventAppFilesDropped = "app:FilesDropped"
)

// DownloadProgress is the payload of EventAppDownload. Emitted while a required runtime dependency
// (ONNX Runtime, CUDA, cuDNN, TensorRT) is being fetched.
type DownloadProgress struct {
	Dependency string  `json:"dependency"`
	Percent    float64 `json:"percent"`
}

// InferenceProgress is the payload of EventAppProgress. Emitted as each model step within a
// processing pipeline advances; Name identifies the sub-operation.
type InferenceProgress struct {
	Name     string  `json:"name"`
	Progress float64 `json:"progress"`
}

// ExportUpdate is the payload of EventAppExport. One event stream serves every file in the export
// queue; subscribers filter by Hash.
//
// Value is overloaded by State: while RUNNING it is a 0.0–1.0 progress ratio; on COMPLETED it is
// the final file size in bytes. The frontend export row formats it accordingly.
type ExportUpdate struct {
	Hash  string  `json:"hash"`
	State string  `json:"state"`
	Value float64 `json:"value"`
}

func init() {
	application.RegisterEvent[DownloadProgress](EventAppDownload)
	application.RegisterEvent[InferenceProgress](EventAppProgress)
	application.RegisterEvent[ExportUpdate](EventAppExport)
	application.RegisterEvent[[]guitypes.File](EventAppFilesDropped)
}
