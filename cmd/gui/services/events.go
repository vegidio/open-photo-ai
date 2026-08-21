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
	EventAppFallback     = "app:fallback"

	EventAppUnsupportedFiles = "app:unsupportedFiles"
)

// DownloadProgress is the payload of EventAppDownload. Emitted while a required runtime dependency
// (ONNX Runtime, CUDA, cuDNN, TensorRT) is being fetched.
type DownloadProgress struct {
	Dependency string  `json:"dependency"`
	Percent    float64 `json:"percent"`
}

// InferenceProgress is the payload of EventAppProgress. Emitted as each model step within a
// processing pipeline advances; Name identifies the sub-operation.
//
// Progress covers the whole pipeline, so the bar fills once however many operations were asked for. Fraction is
// local to whatever Phase is currently running - during "download" it is that download's own percentage, which is
// what the label needs while the bar itself is still near the start of the operation's slice.
type InferenceProgress struct {
	Name     string  `json:"name"`
	Phase    string  `json:"phase"`
	Progress float64 `json:"progress"`
	Fraction float64 `json:"fraction"`
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

// ProviderFallback is the payload of EventAppFallback. Emitted when the selected AI processor couldn't be used - a
// GPU driver that is broken or too old, a GPU without enough free memory - and inference was downgraded to the CPU.
// Only the first downgrade since the models were last loaded is emitted, so the user isn't told the same thing on
// every operation, but a different processor that also fails is reported again.
type ProviderFallback struct {
	Provider string `json:"provider"`
}

// UnsupportedFiles is the payload of EventAppUnsupportedFiles. Emitted when a drag-and-drop included files the
// decoder can't read. Only the base names are sent: the frontend renders the message, so the wording - and its plural
// form, which the "len == 1" test here could only ever get right for English - lives in the i18n catalog instead.
type UnsupportedFiles struct {
	Names []string `json:"names"`
}

func init() {
	application.RegisterEvent[DownloadProgress](EventAppDownload)
	application.RegisterEvent[ProviderFallback](EventAppFallback)
	application.RegisterEvent[InferenceProgress](EventAppProgress)
	application.RegisterEvent[ExportUpdate](EventAppExport)
	application.RegisterEvent[[]guitypes.File](EventAppFilesDropped)
	application.RegisterEvent[UnsupportedFiles](EventAppUnsupportedFiles)
}
