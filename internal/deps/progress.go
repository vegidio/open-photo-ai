package deps

import (
	"io"
	"sync"
	"sync/atomic"
	"time"

	"github.com/vegidio/open-photo-ai/types"
)

// progressInterval is the shortest gap between two progress callbacks. io.Copy reads in 32 KB chunks, so reporting on
// every Read means ~16,000 callbacks for a 500 MB dependency - each one crossing into the GUI's event bus and on to the
// frontend. Throttling keeps the reporting smooth without making the download drive the UI.
const progressInterval = 100 * time.Millisecond

// aggregate spreads one 0-100% report across every source of a dependency. Without it a model split into a graph and a
// weights blob would run to 100% for the 7 MB graph and then start over for the 6.8 GB of weights.
//
// Sources are downloaded concurrently, so several progressReaders report into one aggregate at once. Rather than track
// "bytes finished in earlier sources" - which only has a meaning when they run in order - it counts every byte that
// arrives, from any source, into one atomic total.
type aggregate struct {
	onProgress types.DownloadProgress
	sources    int
	downloaded atomic.Int64

	// mu guards total and lastReport, and is held across the onProgress call itself. Serialising the callback keeps
	// the contract every caller was written against - one report at a time, in non-decreasing order - so concurrent
	// downloads did not become something each of them had to synchronise for. The throttle bounds it to ten calls a
	// second, so the readers never contend for long.
	mu         sync.Mutex
	total      int64 // 0 when the size isn't known ahead of time
	lastReport time.Time
}

// newAggregate sizes the report from what the dependency declares. A single missing size makes the total unknown
// rather than wrong, since a percentage against a partial total would run past 100%.
func newAggregate(dep Dependency, onProgress types.DownloadProgress) *aggregate {
	var total int64

	for _, src := range dep.Sources {
		if src.Size <= 0 {
			total = 0
			break
		}

		total += src.Size
	}

	return &aggregate{onProgress: onProgress, total: total, sources: len(dep.Sources)}
}

// wrap counts one source's transfer towards the whole.
func (a *aggregate) wrap(reader io.Reader, contentLength int64) io.Reader {
	// With a single source there is nothing to spread, so the response's own length is as good as a declared size.
	if contentLength > 0 && a.sources == 1 {
		a.mu.Lock()
		if a.total == 0 {
			a.total = contentLength
		}
		a.mu.Unlock()
	}

	return &progressReader{agg: a, reader: reader}
}

// finish lands the report on 100%, rather than wherever the last throttled tick happened to fall or wherever a total
// assembled from declared sizes happened to end.
func (a *aggregate) finish() {
	if a.onProgress == nil {
		return
	}

	downloaded := a.downloaded.Load()

	a.mu.Lock()
	defer a.mu.Unlock()

	total := a.total
	if total <= 0 {
		total = downloaded
	}

	a.onProgress(downloaded, total, 1.0)
}

// add counts n newly arrived bytes and reports, subject to the throttle. done forces a report through it.
func (a *aggregate) add(n int64, done bool) {
	downloaded := a.downloaded.Add(n)

	if a.onProgress == nil {
		return
	}

	a.mu.Lock()
	defer a.mu.Unlock()

	if !done && time.Since(a.lastReport) < progressInterval {
		return
	}
	a.lastReport = time.Now()

	percent := 0.0
	if a.total > 0 {
		percent = min(float64(downloaded)/float64(a.total), 1)
	}

	a.onProgress(downloaded, a.total, percent)
}

type progressReader struct {
	agg    *aggregate
	reader io.Reader
}

func (pr *progressReader) Read(p []byte) (int, error) {
	n, err := pr.reader.Read(p)

	// The last callback of a source is always emitted, so a failed transfer still reports the bytes that did arrive.
	pr.agg.add(int64(n), err != nil)

	return n, err
}
