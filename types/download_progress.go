package types

type DownloadProgress func(downloaded, total int64, percent float64)
