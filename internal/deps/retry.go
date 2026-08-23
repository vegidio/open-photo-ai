package deps

import (
	"context"
	"io"
	"math/rand/v2"
	"net"
	"net/http"
	"strconv"
	"syscall"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
)

// The retry budget. maxFruitlessAttempts counts attempts that moved *no* bytes rather than attempts
// in total, which is the only bound that makes sense once downloads resume: a seven-gigabyte model
// over a bad link may legitimately reconnect a dozen times and still be converging on the file,
// while a fixed cap would abandon exactly the transfer resuming was built to rescue. An attempt
// that moved even one byte resets the count.
const maxFruitlessAttempts = 5

// The backoff bounds, as vars so tests can collapse them. jitter is separate so a test can pin a
// delay exactly rather than merely shrink it.
var (
	retryBaseDelay = time.Second
	retryMaxDelay  = 30 * time.Second
	jitter         = jitterDefault
)

// jitterDefault is named so a test that swaps jitter out has something to put back.
func jitterDefault() float64 { return rand.Float64() }

// errStalled reports a transfer that stopped delivering bytes without failing. It is separate from
// the context cancellation that actually unblocks the read, because that cancellation is ours: left
// as context.Canceled it would be read as "the caller asked to stop" and end the install.
var errStalled = errors.New("the transfer stalled")

// httpStatusError carries a response status through the retry machinery, so retryable can decide on
// the code rather than on the shape of a message.
//
// RetryAfter is captured here rather than read back later because it is only available on the
// response that carried it, and by the time an error is inspected the body is long closed.
type httpStatusError struct {
	StatusCode int
	Status     string
	RetryAfter time.Duration
}

func (e *httpStatusError) Error() string {
	return "bad status: " + e.Status
}

// retryable reports whether an error is worth another attempt.
//
// It is deliberately an allowlist. Anything not named here - a 404 from a mis-generated URL, a hash
// mismatch, a full disk - fails the same way five times over and only delays the report, so the
// default is to give up and say why.
func retryable(err error) bool {
	if err == nil {
		return false
	}

	// A cancelled context is a decision, not a fault: something asked for this.
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		return false
	}

	// Retrying into a disk with no room just fails more slowly.
	if errors.Is(err, syscall.ENOSPC) {
		return false
	}

	if errors.Is(err, errStalled) || errors.Is(err, errRestart) {
		return true
	}

	var status *httpStatusError
	if errors.As(err, &status) {
		switch status.StatusCode {
		case http.StatusRequestTimeout, http.StatusTooManyRequests:
			return true
		default:
			return status.StatusCode >= 500
		}
	}

	// The usual shapes of a connection dropped mid-transfer, which is what resume exists for.
	if errors.Is(err, io.ErrUnexpectedEOF) || errors.Is(err, syscall.ECONNRESET) ||
		errors.Is(err, syscall.ECONNABORTED) || errors.Is(err, syscall.EPIPE) {
		return true
	}

	var netErr net.Error
	return errors.As(err, &netErr)
}

// withRetry runs op until it succeeds, until it fails for a reason another attempt cannot fix, or
// until enough consecutive attempts have got no further than the one before.
//
// op reports how far it got - the length of the artifact on disk, not the bytes it moved - because
// that is what separates "this link is bad but the file is still arriving" from "this is going
// nowhere". The distinction matters: a server that ignores Range and truncates every response would
// move bytes on every attempt while never advancing past the same offset, and counting bytes rather
// than position would retry it forever.
//
// artifact names what is being fetched, and exists only so the log lines below can say which of
// several concurrent downloads they are about. This is the slowest thing the app ever does and the
// likeliest to fail on a user's machine, so every way out of the loop says why: without it a stalled
// install is a frozen progress bar with nothing in the log to explain it. The success path stays
// silent - the caller already reports that.
func withRetry(ctx context.Context, artifact string, op func(attempt int) (int64, error)) error {
	var fruitless int
	var furthest int64

	for attempt := 1; ; attempt++ {
		reached, err := op(attempt)
		if err == nil {
			return nil
		}

		// Checked before retryable, so a cancellation that surfaced as some other error - a read on
		// a closed body, say - still ends the loop rather than being retried into a dead context.
		if ctx.Err() != nil {
			internal.Log().Info("transfer cancelled", "artifact", artifact, "attempt", attempt)
			return ctx.Err()
		}

		if !retryable(err) {
			internal.Log().Warn("transfer failed and will not be retried",
				"artifact", artifact, "attempt", attempt, "reached", reached, "err", err)
			return err
		}

		if reached > furthest {
			furthest = reached
			fruitless = 0
		} else {
			fruitless++
			if fruitless >= maxFruitlessAttempts {
				internal.Log().Warn("gave up on the transfer", "artifact", artifact,
					"attempts", attempt, "fruitless", fruitless, "furthest", furthest, "err", err)
				return errors.Wrapf(err, "gave up after %d attempts that got no further", fruitless)
			}
		}

		// Hoisted out of the sleepFor call below so the wait can be reported. `stalled` is the one
		// attribute that cannot be recovered from the error text at a glance, and it is exactly what
		// separates "the bar froze for a minute" from an ordinary dropped connection.
		delay := backoff(fruitless, err)

		internal.Log().Warn("transfer attempt failed; retrying",
			"artifact", artifact, "attempt", attempt, "reached", reached, "fruitless", fruitless,
			"backoff", delay, "stalled", errors.Is(err, errStalled), "err", err)

		if err = sleepFor(ctx, delay); err != nil {
			return err
		}
	}
}

// backoff spreads the wait before the next attempt, honouring the server's own instruction when it
// gave one. The jitter is full rather than partial so that a fleet of clients coming back from one
// outage doesn't arrive together and cause the next.
func backoff(fruitless int, err error) time.Duration {
	var status *httpStatusError
	if errors.As(err, &status) && status.RetryAfter > 0 {
		return min(status.RetryAfter, retryMaxDelay)
	}

	delay := retryBaseDelay << max(fruitless-1, 0)

	return time.Duration(jitter() * float64(min(delay, retryMaxDelay)))
}

// sleepFor waits, or gives up early if the context is done. A plain time.Sleep here would make a
// cancelled install sit through the whole backoff before noticing.
func sleepFor(ctx context.Context, d time.Duration) error {
	timer := time.NewTimer(d)
	defer timer.Stop()

	select {
	case <-ctx.Done():
		return ctx.Err()
	case <-timer.C:
		return nil
	}
}

// retryAfter reads the header of the same name, which is either a count of seconds or an HTTP date.
// An unparseable value is treated as absent, leaving the caller with its own backoff.
func retryAfter(resp *http.Response) time.Duration {
	value := resp.Header.Get("Retry-After")
	if value == "" {
		return 0
	}

	if seconds, err := strconv.Atoi(value); err == nil {
		return max(time.Duration(seconds)*time.Second, 0)
	}

	if when, err := http.ParseTime(value); err == nil {
		return max(time.Until(when), 0)
	}

	return 0
}
