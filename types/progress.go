package types

// DownloadProgress is a function type for reporting progress during file downloads.
//
// The downloaded parameter indicates the number of bytes downloaded so far, total represents the total file size in
// bytes, and percent represents the completion percentage as a value between 0.0 and 1.0.
type DownloadProgress func(downloaded, total int64, percent float64)

// InferenceProgress is a function type for reporting progress during model operations. It is what a Model reports
// against, so it describes one model's own run and nothing beyond it: a model has no way to know what else the call
// that invoked it is going to do. ProgressHandler is what the caller of Process or Execute receives.
//
// The operation parameter describes the current processing step, and progress represents the completion percentage as a
// value between 0.0 and 1.0.
type InferenceProgress func(operation string, progress float64)

// Phase distinguishes the two things that take time while an operation is being carried out.
type Phase string

const (
	// PhaseDownload is the model's files being fetched, which happens only the first time it is used.
	PhaseDownload Phase = "download"

	// PhaseInference is the model actually running.
	PhaseInference Phase = "inference"
)

// Progress is a single progress report from Process or Execute.
//
// It carries two different numbers on purpose. Total drives a progress bar; Fraction labels it. During a download
// those two diverge - a download that is 80% done is only a small step of the whole call - and a UI that wants to say
// "Downloading 80%" while the bar sits near the start needs both.
type Progress struct {
	// Operation is the ID of the operation being worked on, e.g. "fr_athens_1_fp32". Unlike the operation name a
	// Model reports, this is always the full ID, and it is filled in from the operation itself rather than by the
	// model, so it is consistent across every phase and every model.
	Operation string

	// Phase says whether the model is being downloaded or being run.
	Phase Phase

	// Total is how far the whole Process or Execute call has got, from 0.0 to 1.0. In a Process chain each operation
	// owns an equal slice of it, so the bar fills exactly once however many operations there are.
	Total float64

	// Fraction is how far the current Phase has got on its own terms, from 0.0 to 1.0: the download's own percentage
	// during PhaseDownload, the model's own during PhaseInference.
	Fraction float64
}

// ProgressHandler receives progress reports from Process and Execute.
type ProgressHandler func(progress Progress)

// FallbackHandler reports that a model couldn't be created with the requested execution provider (ep) and was built
// on the CPU instead, err being the reason why.
type FallbackHandler func(ep ExecutionProvider, err error)
