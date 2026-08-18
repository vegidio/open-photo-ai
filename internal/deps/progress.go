package deps

import (
	"io"
	"time"

	"github.com/vegidio/open-photo-ai/types"
)

// progressInterval is the shortest gap between two progress callbacks. io.Copy reads in 32 KB chunks, so reporting on
// every Read means ~16,000 callbacks for a 500 MB dependency - each one crossing into the GUI's event bus and on to the
// frontend. Throttling keeps the reporting smooth without making the download drive the UI.
const progressInterval = 100 * time.Millisecond

// aggregate spreads one 0-100% report across every source of a dependency. Without it a model split into a graph and a
// weights blob would run to 100% for the 7 MB graph and then start over for the 6.8 GB of weights.
type aggregate struct {
	onProgress types.DownloadProgress
	total      int64 // 0 when the size isn't known ahead of time
	sources    int
	base       int64 // bytes finished in earlier sources
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
	if a.total == 0 && a.sources == 1 && contentLength > 0 {
		a.total = contentLength
	}

	return &progressReader{agg: a, reader: reader}
}

// advance closes off a finished source, so the next one's bytes are counted on top of it rather than from zero.
func (a *aggregate) advance(size int64) {
	a.base += size
}

// finish lands the report on 100%, rather than wherever the last throttled tick happened to fall or wherever a total
// assembled from declared sizes happened to end.
func (a *aggregate) finish() {
	if a.onProgress == nil {
		return
	}

	total := a.total
	if total <= 0 {
		total = a.base
	}

	a.onProgress(a.base, total, 1.0)
}

func (a *aggregate) report(current int64, done bool) {
	if a.onProgress == nil {
		return
	}

	if !done && time.Since(a.lastReport) < progressInterval {
		return
	}
	a.lastReport = time.Now()

	downloaded := a.base + current

	percent := 0.0
	if a.total > 0 {
		percent = float64(downloaded) / float64(a.total)
		if percent > 1 {
			percent = 1
		}
	}

	a.onProgress(downloaded, a.total, percent)
}

type progressReader struct {
	agg    *aggregate
	reader io.Reader
	read   int64
}

func (pr *progressReader) Read(p []byte) (int, error) {
	n, err := pr.reader.Read(p)
	pr.read += int64(n)

	// The last callback of a source is always emitted, so a failed transfer still reports the bytes that did arrive.
	pr.agg.report(pr.read, err != nil)

	return n, err
}
