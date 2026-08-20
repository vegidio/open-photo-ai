package deps

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/vegidio/open-photo-ai/internal"
)

// payload is long enough that a truncated response leaves a prefix worth resuming, and that the
// stall watchdog has something to wait on.
var payload = strings.Repeat("open-photo-ai/", 4096)

// TestDownloadResumesAfterATruncatedBody is the headline behaviour: a connection that drops
// partway must cost the bytes it did not deliver, not the ones it did.
func TestDownloadResumesAfterATruncatedBody(t *testing.T) {
	setup(t)

	srv := newRangeServer(t, payload)
	srv.failAfter.Store(int64(len(payload) / 3))

	dep := dependencyFor("resume", srv.source(t, "model.onnx"))

	// Every response stops a third of the way through what it was asked for, so the transfer can
	// only finish by resuming: three attempts, each picking up where the last was cut off.
	if err := Install(t.Context(), dep, nil); err != nil {
		t.Fatalf("install failed: %v", err)
	}

	ranges := srv.requestedRanges()
	if len(ranges) < 2 {
		t.Fatalf("expected the transfer to be retried, got %d request(s)", len(ranges))
	}

	if !strings.HasPrefix(ranges[len(ranges)-1], "bytes=") {
		t.Errorf("the retry did not ask to resume; ranges were %q", ranges)
	}

	if served := srv.served.Load(); served >= int64(2*len(payload)) {
		t.Errorf("served %d bytes for a %d byte payload; the prefix was not reused", served, len(payload))
	}
}

// TestDownloadResumesFromAnExistingPartFile covers the restart case: the bytes and the record
// outlive the process that wrote them, so a new run continues rather than starts over.
func TestDownloadResumesFromAnExistingPartFile(t *testing.T) {
	dir := setup(t)

	srv := newRangeServer(t, payload)
	src := srv.source(t, "model.onnx")

	have := len(payload) / 2
	seedPart(t, dir, "model.onnx", payload[:have], partState{
		URL:    src.URL,
		Sha256: src.Sha256,
		Size:   int64(len(payload)),
		ETag:   srv.etag,
	})

	if err := Install(t.Context(), dependencyFor("resume", src), nil); err != nil {
		t.Fatalf("install failed: %v", err)
	}

	ranges := srv.requestedRanges()
	if len(ranges) != 1 || !strings.HasPrefix(ranges[0], "bytes=") {
		t.Fatalf("expected a single ranged request, got %q", ranges)
	}

	if served := srv.served.Load(); served != int64(len(payload)-have) {
		t.Errorf("served %d bytes; expected only the %d byte tail", served, len(payload)-have)
	}
}

// TestDownloadIgnoresAStalePartFile is what stops a part file left by one release being appended to
// for the next. The names collide on disk; the recorded URL and hash do not.
func TestDownloadIgnoresAStalePartFile(t *testing.T) {
	dir := setup(t)

	srv := newRangeServer(t, payload)
	src := srv.source(t, "model.onnx")

	seedPart(t, dir, "model.onnx", "bytes of some previous release", partState{
		URL:    "http://example.invalid/another/model.onnx",
		Sha256: "0000000000000000000000000000000000000000000000000000000000000000",
		Size:   30,
	})

	if err := Install(t.Context(), dependencyFor("stale", src), nil); err != nil {
		t.Fatalf("install failed: %v", err)
	}

	if ranges := srv.requestedRanges(); len(ranges) != 1 || ranges[0] != "" {
		t.Errorf("expected one unranged request, got %q", ranges)
	}
}

// TestDownloadRestartsWhenTheServerCannotResume covers a server answering a ranged request with a
// 200. Ignoring the header is its right, and starting over is the correct reading of the reply.
func TestDownloadRestartsWhenTheServerCannotResume(t *testing.T) {
	dir := setup(t)

	srv := newRangeServer(t, payload)
	srv.noRanges.Store(true)

	src := srv.source(t, "model.onnx")

	seedPart(t, dir, "model.onnx", payload[:len(payload)/2], partState{
		URL:    src.URL,
		Sha256: src.Sha256,
		Size:   int64(len(payload)),
		ETag:   srv.etag,
	})

	if err := Install(t.Context(), dependencyFor("restart", src), nil); err != nil {
		t.Fatalf("install failed: %v", err)
	}

	assertInstalled(t, dir, "model.onnx", payload)
}

// TestDownloadRetriesTransientErrors covers the 5xx run, and that Retry-After is obeyed rather than
// the backoff being taken on faith.
func TestDownloadRetriesTransientErrors(t *testing.T) {
	dir := setup(t)

	srv := newRangeServer(t, payload)
	srv.failTimes.Store(3)

	if err := Install(t.Context(), dependencyFor("flaky", srv.source(t, "model.onnx")), nil); err != nil {
		t.Fatalf("install failed: %v", err)
	}

	assertInstalled(t, dir, "model.onnx", payload)

	if hits := srv.hits.Load(); hits != 4 {
		t.Errorf("expected 3 failures and 1 success, got %d requests", hits)
	}
}

// TestDownloadDoesNotRetryA404 pins the allowlist: a permanent answer must fail on the first reply,
// not after five identical ones.
func TestDownloadDoesNotRetryA404(t *testing.T) {
	setup(t)

	srv := newServer(t, map[string]string{"other.onnx": payload})

	dep := dependencyFor("missing", Source{URL: srv.URL + "/model.onnx", Size: int64(len(payload))})

	if err := Install(t.Context(), dep, nil); err == nil {
		t.Fatal("expected the install to fail")
	}

	if hits := srv.hits.Load(); hits != 0 {
		t.Errorf("a 404 should not be retried, but the server was hit %d times", hits)
	}
}

// TestDownloadRecoversFromAStalledBody is the case that used to hang Initialize forever: headers
// arrive, then nothing, and there is no deadline on a body read to notice.
func TestDownloadRecoversFromAStalledBody(t *testing.T) {
	dir := setup(t)

	original := stallTimeout
	stallTimeout = 150 * time.Millisecond
	t.Cleanup(func() { stallTimeout = original })

	srv := newRangeServer(t, payload)
	srv.stall.Store(true)

	// The first attempt goes silent and is abandoned by the watchdog; from then on the server
	// behaves, so the retry is what completes the install.
	go func() {
		time.Sleep(300 * time.Millisecond)
		srv.stall.Store(false)
	}()

	if err := Install(t.Context(), dependencyFor("stalled", srv.source(t, "model.onnx")), nil); err != nil {
		t.Fatalf("install failed: %v", err)
	}

	assertInstalled(t, dir, "model.onnx", payload)
}

// TestInstallKeepsThePartFileOnATransientFailure is the change of policy the resume rests on. The
// old code deleted the partial download on every error, which is what made a blip cost the whole
// transfer.
func TestInstallKeepsThePartFileOnATransientFailure(t *testing.T) {
	dir := setup(t)

	// Truncating *and* refusing ranges is what makes this unrecoverable: every attempt starts over
	// and stops at the same offset, so the file never gets further and the budget runs out.
	srv := newRangeServer(t, payload)
	srv.failAfter.Store(int64(len(payload) / 4))
	srv.noRanges.Store(true)

	if err := Install(t.Context(), dependencyFor("kept", srv.source(t, "model.onnx")), nil); err == nil {
		t.Fatal("expected the install to give up")
	}

	part, state := partFilesFor(t, dir, "model.onnx")

	info, err := os.Stat(part)
	if err != nil {
		t.Fatalf("the part file was not kept: %v", err)
	}

	if info.Size() == 0 {
		t.Error("the part file was kept but is empty")
	}

	if _, err = os.Stat(state); err != nil {
		t.Errorf("the part file's record was not kept: %v", err)
	}
}

// TestInstallRecordsTheFullSizeAfterAResume guards the trap that would otherwise be invisible:
// recording the bytes moved this run rather than the file's length makes Manifest.intact disagree
// with the disk on every launch, and the dependency reinstalls itself forever.
func TestInstallRecordsTheFullSizeAfterAResume(t *testing.T) {
	dir := setup(t)

	srv := newRangeServer(t, payload)
	src := srv.source(t, "model.onnx")

	seedPart(t, dir, "model.onnx", payload[:len(payload)/2], partState{
		URL:    src.URL,
		Sha256: src.Sha256,
		Size:   int64(len(payload)),
		ETag:   srv.etag,
	})

	dep := dependencyFor("sized", src)

	if err := Install(t.Context(), dep, nil); err != nil {
		t.Fatalf("install failed: %v", err)
	}

	manifest := readManifestFor(t, filepath.Join(dir, internal.ModelsDir), dep.manifestName())
	if len(manifest.Files) != 1 {
		t.Fatalf("expected one recorded file, got %d", len(manifest.Files))
	}

	if manifest.Files[0].Size != int64(len(payload)) {
		t.Errorf("recorded size %d, want the full %d", manifest.Files[0].Size, len(payload))
	}

	before := srv.hits.Load()

	if err := Install(t.Context(), dep, nil); err != nil {
		t.Fatalf("second install failed: %v", err)
	}

	if srv.hits.Load() != before {
		t.Error("the second install re-downloaded; the recorded size did not match the file on disk")
	}
}

// TestInstallRejectsAResumedHashMismatch covers the poisoned prefix. The bytes must not survive as a
// resume point, or every later install would rebuild the same wrong file.
func TestInstallRejectsAResumedHashMismatch(t *testing.T) {
	dir := setup(t)

	srv := newRangeServer(t, payload)
	src := srv.source(t, "model.onnx")

	// A prefix of the right length but the wrong content, recorded as though it were genuine.
	seedPart(t, dir, "model.onnx", strings.Repeat("x", len(payload)/2), partState{
		URL:    src.URL,
		Sha256: src.Sha256,
		Size:   int64(len(payload)),
		ETag:   srv.etag,
	})

	// The server only ever serves the tail, so the resumed file cannot hash correctly and the clean
	// second pass is what rescues it.
	if err := Install(t.Context(), dependencyFor("poisoned", src), nil); err != nil {
		t.Fatalf("expected the clean retry to succeed: %v", err)
	}

	assertInstalled(t, dir, "model.onnx", payload)

	part, state := partFilesFor(t, dir, "model.onnx")
	for _, path := range []string{part, state} {
		if _, err := os.Stat(path); !os.IsNotExist(err) {
			t.Errorf("%s survived a successful install", path)
		}
	}
}
