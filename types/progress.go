package types

// DownloadProgress is a function type for reporting progress during file downloads.
//
// The downloaded parameter indicates the number of bytes downloaded so far, total represents the total file size in
// bytes, and percent represents the completion percentage as a value between 0.0 and 1.0.
type DownloadProgress func(downloaded, total int64, percent float64)

// InferenceProgress is a function type for reporting progress during model operations.
//
// The operation parameter describes the current processing step, and progress represents the completion percentage as a
// value between 0.0 and 1.0.
type InferenceProgress func(operation string, progress float64)
