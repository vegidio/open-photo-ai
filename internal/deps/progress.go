package deps

import (
	"sync"
	"sync/atomic"
	"time"

	"github.com/vegidio/open-photo-ai/types"
)

// progressInterval is the shortest gap between two progress callbacks. The transfer loop reads in
// megabyte chunks, so reporting on every read means hundreds of callbacks for a large dependency -
// each one crossing into the GUI's event bus and on to the frontend. Throttling keeps the reporting
// smooth without making the download drive the UI.
const progressInterval = 100 * time.Millisecond

// downloadShare is how much of a report a dependency's transfer is worth when it also has to be
// expanded afterwards.
//
// Extraction is not a pause. A couple of gigabytes of LZMA2 is a minute or more of single-threaded
// work with nothing arriving over the network to report, and it used to happen after the last
// throttled tick had already put the bar on 100% - so the app looked wedged for exactly as long as
// the slowest step took. Giving the expansion the last fifth of the bar is what makes that minute
// legible as work rather than as a hang.
const downloadShare = 0.8

// aggregate spreads one 0-100% report across every source of a dependency, and across the expansion
// that follows when one of them is an archive. Without it a model split into a graph and a weights
// blob would run to 100% for the 7 MB graph and then start over for the 6.8 GB of weights.
//
// Sources are downloaded concurrently, so several transfers report into one aggregate at once.
// Rather than track "bytes finished in earlier sources" - which only has a meaning when they run in
// order - it counts every byte that arrives, from any source, into one atomic total. That the count
// makes no assumption about ordering is also what lets a source correct itself when it resumes.
type aggregate struct {
	onProgress types.DownloadProgress
	sources    int
	expands    bool
	downloaded atomic.Int64

	// mu guards the totals and lastReport, and is held across the onProgress call itself.
	// Serialising the callback keeps the contract every caller was written against - one report at a
	// time, in non-decreasing order - so concurrent downloads did not become something each of them
	// had to synchronise for. The throttle bounds it to ten calls a second, so the readers never
	// contend for long.
	mu           sync.Mutex
	total        int64 // 0 when the size isn't known ahead of time
	extracted    int64
	extractTotal int64
	extracting   bool
	lastReport   time.Time
}

// newAggregate sizes the report from what the dependency declares. A single missing size makes the
// total unknown rather than wrong, since a percentage against a partial total would run past 100%.
func newAggregate(dep Dependency, onProgress types.DownloadProgress) *aggregate {
	var total int64
	var expands bool

	for _, src := range dep.Sources {
		if isArchive(src.FileName()) {
			expands = true
		}

		if src.Size <= 0 {
			total = 0
			break
		}

		total += src.Size
	}

	return &aggregate{onProgress: onProgress, total: total, sources: len(dep.Sources), expands: expands}
}

// setTotal adopts a length the transfer discovered for a dependency that could not declare one.
//
// It takes the artifact's *full* length, which on a resumed response is not the same as the body's:
// Content-Length there describes the remaining tail. Reading the tail as the whole would put the bar
// past 100% for every resumed download, so the size comes from Content-Range and this only ever
// fills a gap the dependency left.
func (a *aggregate) setTotal(total int64) {
	if total <= 0 || a.sources != 1 {
		return
	}

	a.mu.Lock()
	defer a.mu.Unlock()

	if a.total == 0 {
		a.total = total
	}
}

// extract moves the report into the expansion phase and positions it there. The phase occupies
// whatever share of the bar the transfer did not; the first call is forced through the throttle so
// the switch is visible immediately rather than up to a tick later.
func (a *aggregate) extract(done, total int64) {
	a.mu.Lock()
	first := !a.extracting
	a.extracting = true
	a.extractTotal = total
	a.extracted = done
	a.mu.Unlock()

	a.report(0, first)
}

// finish lands the report on 100%, rather than wherever the last throttled tick happened to fall or
// wherever a total assembled from declared sizes happened to end.
func (a *aggregate) finish() {
	if a.onProgress == nil {
		return
	}

	a.mu.Lock()
	defer a.mu.Unlock()

	downloaded := a.downloaded.Load()

	total := a.total
	if total <= 0 {
		total = downloaded
	}

	a.lastReport = time.Now()
	a.onProgress(downloaded, total, 1.0)
}

// add counts n newly arrived bytes and reports, subject to the throttle. n may be negative, which is
// how a source that had to start over gives back the bytes the bar was already told about.
func (a *aggregate) add(n int64, done bool) {
	a.downloaded.Add(n)
	a.report(n, done)
}

// report emits the current position, unless the last one was too recent. done forces it through.
func (a *aggregate) report(_ int64, done bool) {
	if a.onProgress == nil {
		return
	}

	a.mu.Lock()
	defer a.mu.Unlock()

	if !done && time.Since(a.lastReport) < progressInterval {
		return
	}
	a.lastReport = time.Now()

	downloaded := a.downloaded.Load()
	a.onProgress(downloaded, a.total, a.percent(downloaded))
}

// percent is the position of the bar, which is not simply the fraction downloaded once a dependency
// has to be expanded as well. Callers hold mu.
func (a *aggregate) percent(downloaded int64) float64 {
	share := 1.0
	if a.expands {
		share = downloadShare
	}

	transferred := 0.0
	if a.total > 0 {
		transferred = min(float64(downloaded)/float64(a.total), 1) * share
	}

	if !a.extracting {
		return transferred
	}

	expanded := 1.0
	if a.extractTotal > 0 {
		expanded = min(float64(a.extracted)/float64(a.extractTotal), 1)
	}

	return min(share+expanded*(1-share), 1)
}

// sourceProgress tracks one source's contribution to the aggregate, so a transfer that resumed or
// had to start over can correct the total without the aggregate needing to know that sources exist.
type sourceProgress struct {
	agg      *aggregate
	credited int64
}

// place declares how many bytes of this source are on disk right now, correcting whatever the
// aggregate was last told. It is what stops a resumed download from drawing its bar from zero while
// the file runs from ninety percent to full, and what gives the bytes back when a part file that had
// already been counted turns out to be unusable.
func (s *sourceProgress) place(n int64) {
	if s == nil || n == s.credited {
		return
	}

	s.agg.add(n-s.credited, false)
	s.credited = n
}

// advance records bytes that have just arrived.
func (s *sourceProgress) advance(n int64) {
	s.credited += n
	s.agg.add(n, false)
}
