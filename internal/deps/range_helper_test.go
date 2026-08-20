package deps

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/vegidio/open-photo-ai/internal"
)

// rangeServer serves one payload the way the real hosts do - honouring Range with a 206 and a
// Content-Range - and records enough about what was asked of it to assert that a resume actually
// resumed rather than merely ending up with the right bytes.
//
// The switches cover the failure shapes the download path is supposed to survive: a body that stops
// early, a run of 5xx before success, and a connection that goes silent without closing.
type rangeServer struct {
	*httptest.Server

	body string
	etag string

	// The switches are atomic because a test may flip one while a request is in flight - that is the
	// point of several of them - and the handler runs on the server's own goroutine.

	// noRanges answers every request with a 200, as a server that does not implement Range would.
	noRanges atomic.Bool

	// failAfter, when positive, writes that many bytes of each response and then hangs up.
	failAfter atomic.Int64

	// failTimes counts the leading requests to answer with a 500 before serving anything.
	failTimes atomic.Int32

	// stall holds the body open, after the headers, until the client gives up on it.
	stall atomic.Bool

	mu      sync.Mutex
	ranges  []string
	hits    atomic.Int32
	served  atomic.Int64
	current atomic.Int32
	peak    atomic.Int32
}

// fastRetries collapses the backoff so a test exercising the retry path costs milliseconds rather
// than the tens of seconds the real schedule is spread over.
func fastRetries(t *testing.T) {
	t.Helper()

	base, cap, jit := retryBaseDelay, retryMaxDelay, jitter

	retryBaseDelay = time.Millisecond
	retryMaxDelay = 5 * time.Millisecond
	jitter = func() float64 { return 1 }

	t.Cleanup(func() { retryBaseDelay, retryMaxDelay, jitter = base, cap, jit })
}

func newRangeServer(t *testing.T, body string) *rangeServer {
	t.Helper()

	fastRetries(t)

	s := &rangeServer{body: body, etag: `"v1"`}
	s.Server = httptest.NewServer(http.HandlerFunc(s.handle))

	t.Cleanup(s.Close)

	return s
}

func (s *rangeServer) handle(w http.ResponseWriter, r *http.Request) {
	s.hits.Add(1)

	if depth := s.current.Add(1); depth > s.peak.Load() {
		s.peak.Store(depth)
	}
	defer s.current.Add(-1)

	if remaining := s.failTimes.Add(-1); remaining >= 0 {
		w.Header().Set("Retry-After", "0")
		w.WriteHeader(http.StatusInternalServerError)

		return
	}

	requested := r.Header.Get("Range")

	s.mu.Lock()
	s.ranges = append(s.ranges, requested)
	s.mu.Unlock()

	start := 0
	if offset, ok := parseRangeStart(requested); ok && !s.noRanges.Load() {
		start = offset
	}

	if start > len(s.body) {
		w.WriteHeader(http.StatusRequestedRangeNotSatisfiable)
		return
	}

	chunk := s.body[start:]

	w.Header().Set("ETag", s.etag)
	w.Header().Set("Content-Length", strconv.Itoa(len(chunk)))

	if start > 0 {
		w.Header().Set("Content-Range", fmt.Sprintf("bytes %d-%d/%d", start, len(s.body)-1, len(s.body)))
		w.WriteHeader(http.StatusPartialContent)
	} else {
		w.WriteHeader(http.StatusOK)
	}

	if s.stall.Load() {
		w.(http.Flusher).Flush()

		// Waiting on the request's own context rather than sleeping is what lets the handler end
		// when the client's stall watchdog cancels it. A plain sleep would hold the connection open
		// past the end of the test, and httptest.Server.Close waits for its handlers.
		<-r.Context().Done()

		return
	}

	if cut := int(s.failAfter.Load()); cut > 0 && cut < len(chunk) {
		chunk = chunk[:cut]
		w.Write([]byte(chunk))
		s.served.Add(int64(len(chunk)))
		w.(http.Flusher).Flush()

		// Hijack and drop the connection, so the client sees a body that stopped short of the length
		// it was promised rather than a clean end.
		if conn, _, err := w.(http.Hijacker).Hijack(); err == nil {
			conn.Close()
		}

		return
	}

	w.Write([]byte(chunk))
	s.served.Add(int64(len(chunk)))
}

// requestedRanges returns the Range header of every request in order, "" for a request that had none.
func (s *rangeServer) requestedRanges() []string {
	s.mu.Lock()
	defer s.mu.Unlock()

	return append([]string(nil), s.ranges...)
}

func (s *rangeServer) source(t *testing.T, name string) Source {
	t.Helper()

	return Source{
		URL:    s.URL + "/" + name,
		Sha256: sha256Of(t, s.body),
		Size:   int64(len(s.body)),
	}
}

func parseRangeStart(header string) (int, bool) {
	value, found := strings.CutPrefix(header, "bytes=")
	if !found {
		return 0, false
	}

	start, _, _ := strings.Cut(value, "-")

	offset, err := strconv.Atoi(start)
	if err != nil {
		return 0, false
	}

	return offset, true
}

// dependencyFor wraps one source as an installable dependency in the shared models directory, which
// is the destination that does not require the archive handling.
func dependencyFor(name string, src Source) Dependency {
	return Dependency{
		Name:        name,
		Destination: internal.ModelsDir,
		Sources:     []Source{src},
	}
}

// partFilesFor names the two bookkeeping files for a source, for tests that seed or inspect them.
func partFilesFor(t *testing.T, dir, name string) (string, string) {
	t.Helper()

	return partPaths(filepath.Join(dir, internal.ModelsDir), name)
}

// seedPart writes a partial download and the record describing it, standing in for a previous run
// of the app that was interrupted mid-transfer.
func seedPart(t *testing.T, root, name, body string, state partState) {
	t.Helper()

	dir := filepath.Join(root, internal.ModelsDir)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("failed to create %s: %v", dir, err)
	}

	part, statePath := partPaths(dir, name)

	if err := os.WriteFile(part, []byte(body), 0o644); err != nil {
		t.Fatalf("failed to seed the part file: %v", err)
	}

	if err := writePartState(statePath, state); err != nil {
		t.Fatalf("failed to seed the part file's record: %v", err)
	}
}

// assertInstalled checks that a source ended up on disk under its own name with the bytes expected.
func assertInstalled(t *testing.T, root, name, want string) {
	t.Helper()

	got, err := os.ReadFile(filepath.Join(root, internal.ModelsDir, name))
	if err != nil {
		t.Fatalf("%s was not installed: %v", name, err)
	}

	if string(got) != want {
		t.Errorf("%s holds %d bytes, want %d", name, len(got), len(want))
	}
}
