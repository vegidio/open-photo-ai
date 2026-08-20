package deps

import (
	"context"
	"errors"
	"io"
	"net"
	"net/http"
	"syscall"
	"testing"
	"time"
)

func TestRetryableClassifiesErrors(t *testing.T) {
	cases := []struct {
		name string
		err  error
		want bool
	}{
		{"nil", nil, false},
		{"cancelled", context.Canceled, false},
		{"deadline", context.DeadlineExceeded, false},
		{"disk full", syscall.ENOSPC, false},
		{"not found", &httpStatusError{StatusCode: http.StatusNotFound}, false},
		{"forbidden", &httpStatusError{StatusCode: http.StatusForbidden}, false},
		{"range not satisfiable", &httpStatusError{StatusCode: http.StatusRequestedRangeNotSatisfiable}, false},
		{"server error", &httpStatusError{StatusCode: http.StatusInternalServerError}, true},
		{"unavailable", &httpStatusError{StatusCode: http.StatusServiceUnavailable}, true},
		{"too many requests", &httpStatusError{StatusCode: http.StatusTooManyRequests}, true},
		{"request timeout", &httpStatusError{StatusCode: http.StatusRequestTimeout}, true},
		{"truncated body", io.ErrUnexpectedEOF, true},
		{"connection reset", syscall.ECONNRESET, true},
		{"broken pipe", syscall.EPIPE, true},
		{"stalled", errStalled, true},
		{"artifact moved", errRestart, true},
		{"network", &net.OpError{Err: errors.New("dial")}, true},
		{"anything else", errors.New("some other failure"), false},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if got := retryable(c.err); got != c.want {
				t.Errorf("retryable(%v) = %v, want %v", c.err, got, c.want)
			}
		})
	}
}

// TestBackoffHonoursRetryAfter pins the one case where the server, not the client, sets the pace.
func TestBackoffHonoursRetryAfter(t *testing.T) {
	fastRetries(t)

	err := &httpStatusError{StatusCode: http.StatusServiceUnavailable, RetryAfter: 2 * time.Millisecond}

	if got := backoff(1, err); got != 2*time.Millisecond {
		t.Errorf("backoff = %v, want the server's 2ms", got)
	}

	// Even a server asking for an hour is capped, or one bad header parks the install.
	tooLong := &httpStatusError{StatusCode: http.StatusServiceUnavailable, RetryAfter: time.Hour}
	if got := backoff(1, tooLong); got != retryMaxDelay {
		t.Errorf("backoff = %v, want it capped at %v", got, retryMaxDelay)
	}
}

// TestBackoffGrowsAndIsCapped covers the schedule itself, with the jitter pinned so the bounds are
// exact rather than probabilistic.
func TestBackoffGrowsAndIsCapped(t *testing.T) {
	fastRetries(t)

	retryBaseDelay = time.Second
	retryMaxDelay = 4 * time.Second

	for _, c := range []struct {
		fruitless int
		want      time.Duration
	}{
		{0, time.Second},
		{1, time.Second},
		{2, 2 * time.Second},
		{3, 4 * time.Second},
		{9, 4 * time.Second},
	} {
		if got := backoff(c.fruitless, errStalled); got != c.want {
			t.Errorf("backoff(%d) = %v, want %v", c.fruitless, got, c.want)
		}
	}
}

// TestWithRetryStopsWhenNothingAdvances is the budget: attempts that keep reaching the same offset
// are not progress, however many bytes they moved getting there.
func TestWithRetryStopsWhenNothingAdvances(t *testing.T) {
	fastRetries(t)

	var attempts int

	err := withRetry(t.Context(), func(int) (int64, error) {
		attempts++
		return 0, errStalled
	})

	if err == nil {
		t.Fatal("expected withRetry to give up")
	}

	if attempts != maxFruitlessAttempts {
		t.Errorf("made %d attempts, want %d", attempts, maxFruitlessAttempts)
	}
}

// TestWithRetryStopsWhenProgressStops is the same budget one step along: a transfer that got
// somewhere and then stuck gets its attempts counted from where it stopped advancing, not from the
// beginning. Reaching a new offset once buys one more round, and no more.
func TestWithRetryStopsWhenProgressStops(t *testing.T) {
	fastRetries(t)

	var attempts int

	err := withRetry(t.Context(), func(int) (int64, error) {
		attempts++
		return 100, errStalled
	})

	if err == nil {
		t.Fatal("expected withRetry to give up")
	}

	if want := maxFruitlessAttempts + 1; attempts != want {
		t.Errorf("made %d attempts, want %d - one that advanced, then the budget", attempts, want)
	}
}

// TestWithRetryKeepsGoingWhileAdvancing is the other half: a bad link that is still delivering must
// not be abandoned just because it has reconnected more times than a fixed cap would allow.
func TestWithRetryKeepsGoingWhileAdvancing(t *testing.T) {
	fastRetries(t)

	var reached int64

	err := withRetry(t.Context(), func(int) (int64, error) {
		reached += 10
		if reached >= 200 {
			return reached, nil
		}

		return reached, errStalled
	})
	if err != nil {
		t.Fatalf("withRetry gave up on a transfer that was still advancing: %v", err)
	}

	if reached != 200 {
		t.Errorf("stopped at %d, want 200", reached)
	}
}

// TestWithRetryStopsOnCancellation checks that a cancelled install does not sit through its backoff.
func TestWithRetryStopsOnCancellation(t *testing.T) {
	retryBaseDelay, retryMaxDelay = time.Hour, time.Hour
	jitter = func() float64 { return 1 }

	t.Cleanup(func() {
		retryBaseDelay, retryMaxDelay, jitter = time.Second, 30*time.Second, jitterDefault
	})

	ctx, cancel := context.WithCancel(t.Context())

	go func() {
		time.Sleep(50 * time.Millisecond)
		cancel()
	}()

	done := make(chan error, 1)
	go func() { done <- withRetry(ctx, func(int) (int64, error) { return 0, errStalled }) }()

	select {
	case err := <-done:
		if !errors.Is(err, context.Canceled) {
			t.Errorf("got %v, want a cancellation", err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("withRetry sat through its backoff instead of noticing the cancellation")
	}
}
