package types

// CacheMode says what is backing the image cache after Initialize. It is reported, not configured: the library opens
// the best store it can and says which one it got.
type CacheMode string

const (
	// CacheModeDisk is the normal outcome: results survive the run.
	CacheModeDisk CacheMode = "disk"

	// CacheModeMemory means the cache directory could not be opened - another copy of the app holds Badger's directory
	// lock, or the directory is unwritable - and the run is caching in RAM instead. Correct, smaller, not durable.
	CacheModeMemory CacheMode = "memory"

	// CacheModeNone means there is no cache at all and every operation is recomputed. Slower, never wrong.
	CacheModeNone CacheMode = "none"
)
