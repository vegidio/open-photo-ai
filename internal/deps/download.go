package deps

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"hash"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
)

// stallTimeout is how long a transfer may deliver nothing before the attempt is abandoned and
// retried.
//
// It exists because body reads have no deadline and cannot have one: a legitimate artifact is two
// gigabytes over whatever link the user has, so any bound on the transfer as a whole is a bound on
// how large a file the app can install. Bounding the *silence* instead costs nothing on a slow link
// and turns a server that accepts the connection and then goes quiet - which used to hold
// Initialize open forever, with nothing to report and nothing to cancel it - into one more retry.
//
// A var rather than a const so tests can shrink it, the same seam un7zip uses.
var stallTimeout = 60 * time.Second

// downloadBufferSize is the unit a body is moved in. io.Copy's default is 32 KB; at the ~35 MB/s the
// slower of the two hosts delivers, that is around a thousand read/write pairs a second, which is
// not where the time goes. The larger buffer is here because the loop needs one of its own anyway -
// io.Copy has nowhere to reset a watchdog or to fold in the hash - and a megabyte is small enough to
// keep one per concurrent download.
const downloadBufferSize = 1 << 20

var bufferPool = sync.Pool{New: func() any {
	buf := make([]byte, downloadBufferSize)
	return &buf
}}

// errRestart reports that the part file on disk is a prefix of something the server is no longer
// serving. It is separate from a plain failure because the recovery is different: the bytes have to
// go before the next attempt, or every attempt resumes onto the same dead prefix.
var errRestart = errors.New("the artifact changed under its URL")

// acquire brings one source's file into dir and returns what it put there, resuming and retrying for
// as long as the transfer is still converging on the artifact.
//
// It owns the part file's whole life, which is what lets the cleanup rules live in one place. The
// old code deleted the partial download on every error, so a connection dropped at ninety percent of
// a seven-gigabyte model threw away everything it had; here the partial download is *kept* for
// anything a retry could fix, and deleted only when its bytes are known to be wrong.
func acquire(ctx context.Context, dir string, src Source, skipVerify bool, agg *aggregate) (File, error) {
	name := src.FileName()
	part, state := partPaths(dir, name)
	prog := &sourceProgress{agg: agg}

	// Two passes at most. The second only happens when a resumed transfer failed verification, where
	// the prefix on disk is the likeliest suspect and a clean run is the cheapest way to be sure -
	// see the mismatch handling below.
	for attempt := range 2 {
		sum, size, resumed, err := fill(ctx, src, part, state, prog)
		if err != nil {
			return File{}, err
		}

		if src.Sha256 == "" || sum == src.Sha256 {
			if err = os.Rename(part, filepath.Join(dir, name)); err != nil {
				return File{}, errors.Wrapf(err, "failed to place %s", name)
			}

			os.Remove(state)

			return File{Path: name, Size: size, Sha256: sum}, nil
		}

		// The bytes are known bad, so they must not survive as a resume point for the next install.
		removePart(part, state)
		prog.place(0)

		if skipVerify {
			internal.Log().Warn("hash mismatch ignored", "artifact", name, "expected", src.Sha256, "got", sum)
			return File{Path: name, Size: size, Sha256: sum}, nil
		}

		if !resumed || attempt == 1 {
			return File{}, errors.Newf("hash mismatch for %s: expected %s, got %s", name, src.Sha256, sum)
		}

		internal.Log().Warn("hash mismatch after a resumed transfer; downloading it again from the start",
			"artifact", name)
	}

	return File{}, errors.Newf("failed to download %s", name)
}

// fill runs the retry loop for one source and returns the hash and size of the completed part file,
// along with whether any attempt picked up where an earlier one left off.
//
// The size is deliberately read back with Stat rather than taken from the bytes this run copied.
// Once transfers resume those are different numbers, and it is the file's length that goes into the
// manifest - where Manifest.intact compares it against Stat on every launch. Recording the bytes
// moved would make a resumed download reinstall itself forever, always looking short.
func fill(ctx context.Context, src Source, part, state string, prog *sourceProgress) (string, int64, bool, error) {
	var inline string
	var resumed bool

	err := withRetry(ctx, func(int) (int64, error) {
		moved, startedAt, sum, err := transfer(ctx, src, part, state, prog)

		inline = sum
		resumed = resumed || startedAt > 0

		if errors.Is(err, errRestart) {
			removePart(part, state)
			prog.place(0)
		}

		// How far the file got, not how much this attempt carried - see withRetry.
		return startedAt + moved, err
	})
	if err != nil {
		return "", 0, resumed, err
	}

	info, err := os.Stat(part)
	if err != nil {
		return "", 0, resumed, errors.Wrapf(err, "failed to measure %s", filepath.Base(part))
	}

	// An empty hash means the transfer could not keep one as it went, because it started partway
	// through the file. Reading it back is the price of resuming, and it is only paid on a resume:
	// six or seven gigabytes is a few seconds against the minutes the transfer itself took, and
	// against re-downloading all of it, nothing.
	if inline == "" {
		if inline, err = hashFile(part); err != nil {
			return "", 0, resumed, err
		}
	}

	return inline, info.Size(), resumed, nil
}

// transfer is one attempt: it asks for whatever the part file is still missing, and appends what
// arrives. It reports the bytes it moved even when it fails, because that is what tells withRetry
// whether the link is bad but converging or going nowhere at all.
func transfer(
	ctx context.Context,
	src Source,
	part, state string,
	prog *sourceProgress,
) (moved, startedAt int64, sum string, err error) {
	have, resume := resumePoint(part, state, src)

	// The watchdog's only way to interrupt a silent read is to cancel the request underneath it.
	attemptCtx, cancel := context.WithCancel(ctx)
	defer cancel()

	req, err := http.NewRequestWithContext(attemptCtx, http.MethodGet, src.URL, nil)
	if err != nil {
		return 0, 0, "", errors.Wrap(err, "failed to build the request")
	}

	if resume {
		req.Header.Set("Range", "bytes="+strconv.FormatInt(have, 10)+"-")

		// Go's transport otherwise offers gzip and unwraps the response itself, which would leave
		// the offsets in the Range header describing the compressed stream and the bytes arriving
		// from the decompressed one.
		req.Header.Set("Accept-Encoding", "identity")
	}

	resp, err := downloadClient.Do(req)
	if err != nil {
		return 0, 0, "", errors.Wrap(err, "failed to send the request")
	}
	defer resp.Body.Close()

	// A response with no length of its own reports -1, which must not be recorded as a size: it
	// would make every later run compare it against the source's real size and decline to resume.
	total := max(resp.ContentLength, 0)

	switch {
	case resp.StatusCode == http.StatusPartialContent && resume:
		if !readPartStateOf(state).unchanged(resp) {
			return 0, 0, "", errRestart
		}

		startedAt = have
		if size, ok := rangeTotal(resp.Header.Get("Content-Range")); ok {
			total = size
		}

	case resp.StatusCode == http.StatusOK:
		// Either nothing was resumed, or the server ignored the Range header - which it is entitled
		// to do. Starting over is the correct reading of a 200, not a failure.
		startedAt = 0

		// A source that declares its size is more trustworthy than a response that omitted one.
		if total == 0 {
			total = src.Size
		}

	default:
		return 0, 0, "", &httpStatusError{
			StatusCode: resp.StatusCode,
			Status:     resp.Status,
			RetryAfter: retryAfter(resp),
		}
	}

	file, err := os.OpenFile(part, os.O_WRONLY|os.O_CREATE, 0o644)
	if err != nil {
		return 0, 0, "", errors.Wrap(err, "failed to create the destination file")
	}
	defer file.Close()

	// Truncating to the resume point stands in for the O_TRUNC this used to open with: it drops any
	// tail past where the server is about to continue from, so a shorter artifact overwriting a
	// longer leftover cannot keep the old ending.
	if err = file.Truncate(startedAt); err != nil {
		return 0, 0, "", errors.Wrap(err, "failed to trim the destination file")
	}

	if _, err = file.Seek(startedAt, io.SeekStart); err != nil {
		return 0, 0, "", errors.Wrap(err, "failed to seek the destination file")
	}

	// Written before the first byte, so a crash mid-transfer still leaves a record of what these
	// bytes were meant to become. Without it they are unresumable and get thrown away.
	if err = writePartState(state, partState{
		URL:      src.URL,
		Sha256:   src.Sha256,
		Size:     total,
		ETag:     resp.Header.Get("ETag"),
		Modified: resp.Header.Get("Last-Modified"),
	}); err != nil {
		return 0, 0, "", err
	}

	prog.agg.setTotal(total)
	prog.place(startedAt)

	// The hash can only be kept as the bytes go past when they start at the beginning. A resumed
	// transfer leaves it empty and fill reads the finished file back instead.
	var digest hash.Hash
	if startedAt == 0 {
		digest = sha256.New()
	}

	moved, err = copyBody(file, digest, resp.Body, prog, cancel)
	if err != nil {
		return moved, startedAt, "", err
	}

	// A body that ends early with no length to check it against would otherwise be accepted, written
	// to the manifest at its truncated size, and agreed with by every later launch.
	if total > 0 && startedAt+moved != total {
		return moved, startedAt, "", errors.Wrapf(io.ErrUnexpectedEOF,
			"%s ended at %d of %d bytes", filepath.Base(part), startedAt+moved, total)
	}

	if digest == nil {
		return moved, startedAt, "", nil
	}

	return moved, startedAt, hex.EncodeToString(digest.Sum(nil)), nil
}

// copyBody moves the body to disk, folding in the hash and the progress report, and gives up on a
// transfer that has stopped delivering. See stallTimeout for why the silence is what gets bounded.
func copyBody(
	file io.Writer,
	digest hash.Hash,
	body io.Reader,
	prog *sourceProgress,
	cancel context.CancelFunc,
) (int64, error) {
	buf := bufferPool.Get().(*[]byte)
	defer bufferPool.Put(buf)

	var stalled atomic.Bool

	watchdog := time.AfterFunc(stallTimeout, func() {
		stalled.Store(true)
		cancel()
	})
	defer watchdog.Stop()

	var moved int64

	for {
		n, err := body.Read(*buf)

		if n > 0 {
			watchdog.Reset(stallTimeout)

			if _, werr := file.Write((*buf)[:n]); werr != nil {
				return moved, errors.Wrap(werr, "failed to write the file")
			}

			if digest != nil {
				digest.Write((*buf)[:n])
			}

			moved += int64(n)
			prog.advance(int64(n))
		}

		if err != nil {
			if errors.Is(err, io.EOF) {
				return moved, nil
			}

			// The cancellation below is ours, so it has to be reported as the stall it was rather
			// than as the caller changing their mind - which would end the install instead of
			// retrying it.
			if stalled.Load() {
				return moved, errStalled
			}

			return moved, err
		}
	}
}

// resumePoint reports how many bytes on disk can be continued towards src, and whether they can be
// continued at all.
func resumePoint(part, state string, src Source) (int64, bool) {
	info, err := os.Stat(part)
	if err != nil {
		return 0, false
	}

	recorded, found := readPartState(state)
	if !found || !recorded.resumable(src, info.Size()) {
		return 0, false
	}

	return info.Size(), true
}

// readPartStateOf returns the record beside a part file, or a zero one. The zero value answers
// "unchanged" for any response, which is the right default: it is only reached when there was no
// record to disagree with.
func readPartStateOf(state string) partState {
	recorded, _ := readPartState(state)

	return recorded
}

// rangeTotal pulls the artifact's full length out of a Content-Range header. The header's own length
// describes the slice being sent, so this is the only place the whole size is stated on a resumed
// response.
func rangeTotal(header string) (int64, bool) {
	_, size, found := strings.Cut(header, "/")
	if !found || size == "*" {
		return 0, false
	}

	total, err := strconv.ParseInt(size, 10, 64)
	if err != nil || total <= 0 {
		return 0, false
	}

	return total, true
}

// hashFile reads a finished download back through SHA-256. It is the fallback for an artifact whose
// hash could not be kept in stream - see fill.
func hashFile(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", errors.Wrapf(err, "failed to reopen %s", filepath.Base(path))
	}
	defer file.Close()

	buf := bufferPool.Get().(*[]byte)
	defer bufferPool.Put(buf)

	digest := sha256.New()
	if _, err = io.CopyBuffer(digest, file, *buf); err != nil {
		return "", errors.Wrapf(err, "failed to read %s back", filepath.Base(path))
	}

	return hex.EncodeToString(digest.Sum(nil)), nil
}
