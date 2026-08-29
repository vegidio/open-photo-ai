package shared

import (
	"bufio"
	"context"
	"fmt"
	"log/slog"
	"os"
	"regexp"
	"runtime/debug"
	"strings"
	"sync"
	"time"
	"unicode/utf16"
	"unicode/utf8"
)

const (
	// readerBufSize is the reader's starting buffer, not a limit - see drain for why there must not be one.
	readerBufSize = 64 << 10

	// drainTimeout bounds how long Close waits for the reader to reach EOF. Shutdown must not hang because some other
	// part of the process kept a duplicate of the write end alive.
	drainTimeout = 2 * time.Second

	// maxLoggedLine caps how much of a single stderr line makes it into a log record. ONNX Runtime's node-assignment
	// warnings name every node that fell back to another provider and can run to hundreds of kilobytes; the cap is
	// applied at formatting time, and never to the read itself - see drain for why that distinction matters.
	maxLoggedLine = 16 << 10
)

// stderrCapture points the process's stderr at a pipe and turns everything written to it into log records.
//
// It exists for ONNX Runtime. ORT logs from C++ through std::cerr - std::wcerr on Windows - and the Go binding creates
// the environment with plain CreateEnv rather than CreateEnvWithCustomLogger, so there is no callback to install and no
// sink to swap. Its log level can be changed; its destination cannot. Redirecting what the process calls stderr is the
// only interception point there is, which is also why this catches the CoreML, CUDA and TensorRT providers, and
// anything else in the process writing to stderr.
//
// What "stderr" means is where the platforms part company, and redirectStderr owns that difference: a descriptor on
// Unix, and on Windows a C runtime descriptor, the stream sitting on it and the Win32 handle behind both.
//
// It stays unexported on purpose. Its preconditions - a logger that does not write to stderr, and a start that beats
// every goroutine in the process - cannot be enforced by the type, so SetupLogging is the only call site there is.
type stderrCapture struct {
	logger *slog.Logger

	read  *os.File
	write *os.File

	restore func() error

	done chan struct{}

	closeOnce sync.Once
	closeErr  error
}

// startStderrCapture redirects stderr into logger, one record per line.
//
// The logger must not write to stderr: it would feed its own input. Go's own stderr is preserved - os.Stderr and the
// runtime's crash output are pointed at a duplicate of the original, so prints and panic traces still reach the
// terminal, and keep reaching it after the capture is closed. That reassignment of the package-level os.Stderr is a
// process-wide write, safe only because SetupLogging runs at the top of main before any goroutine exists; do not move
// the call later.
func startStderrCapture(logger *slog.Logger) (*stderrCapture, error) {
	read, write, err := os.Pipe()
	if err != nil {
		return nil, err
	}

	saved, restore, err := redirectStderr(write)
	if err != nil {
		_ = read.Close()
		_ = write.Close()

		return nil, err
	}

	c := &stderrCapture{
		logger:  logger,
		read:    read,
		write:   write,
		restore: restore,
		done:    make(chan struct{}),
	}

	if saved != nil {
		os.Stderr = saved

		// An unhandled panic or a runtime fatal error is written to descriptor 2 - now the pipe - with every other
		// goroutine already frozen, so the reader will never drain it and the trace would vanish with the process.
		// SetCrashOutput duplicates the descriptor internally, so saved's lifetime is not a concern here.
		_ = debug.SetCrashOutput(saved, debug.CrashOptions{})
	}

	go c.drain()

	return c, nil
}

// Close restores stderr, drains what is left in the pipe and stops the reader. It is idempotent.
func (c *stderrCapture) Close() error {
	c.closeOnce.Do(func() {
		// Stderr first: from here on native writes go back to the terminal rather than into a pipe whose reader is
		// about to go away.
		if err := c.restore(); err != nil {
			c.closeErr = err
		}

		// Only now does dropping our own handle produce an EOF - before the restore, descriptor 2 was still a
		// duplicate of the same write end, and closing this one would have changed nothing.
		_ = c.write.Close()

		select {
		case <-c.done:
		case <-time.After(drainTimeout):
			// A stray duplicate of the write end is keeping the pipe open. Closing the read end below fails the
			// pending read, which is what lets the goroutine exit.
		}

		_ = c.read.Close()

		// os.Stderr keeps the saved duplicate rather than being put back: it names the same terminal the original did,
		// it stays open for the life of the process, and on Windows the original it would be put back to is a handle
		// that pointing descriptor 2 at the pipe has already closed.
	})

	return c.closeErr
}

// drain reads the pipe a line at a time until the write end is gone.
//
// It reads with bufio.Reader rather than bufio.Scanner deliberately. ORT's node-assignment warning names every node
// that fell back to another execution provider and routinely runs past 64 KiB; a Scanner returns ErrTooLong on such a
// token and then stops for good, leaving the pipe undrained. The pipe buffer would fill and the ORT thread would block
// inside write(2) - a hang in native code with no Go stack to show for it. ReadString grows its own buffer instead,
// and truncate applies the size cap once the record is formatted.
func (c *stderrCapture) drain() {
	defer close(c.done)

	reader := bufio.NewReaderSize(c.read, readerBufSize)

	for {
		line, err := reader.ReadString('\n')

		// A final line without a trailing newline still has to be logged, so the content is handled before the error.
		if line != "" {
			c.emit(line)
		}

		if err != nil {
			return
		}
	}
}

// emit turns one line into a log record.
func (c *stderrCapture) emit(line string) {
	line = normalizeLine(line)
	if strings.TrimSpace(line) == "" {
		return
	}

	if rec, ok := parseOrtLine(line); ok {
		c.logger.Log(context.Background(), rec.level, truncate(rec.msg), rec.attrs...)
		return
	}

	// Not ORT's format: another library on the same descriptor, or a continuation line of a multi-line message. It is
	// exactly the output that used to reach the terminal, so it is kept - but it carries no severity of its own, and
	// INFO is the level that neither hides it nor overstates it.
	c.logger.Log(context.Background(), slog.LevelInfo, truncate(line), "source", "stderr")
}

// region - Private functions

// ansiRe matches an ANSI CSI escape sequence. ONNX Runtime colours its output by severity, so on Windows the warnings
// that matter here arrive wrapped in "\x1b[0;93m" and "\x1b[m" - invisible on a terminal, but in a log file both
// unreadable and enough to stop ortLineRe matching at all.
var ansiRe = regexp.MustCompile("\x1b\\[[0-9;?]*[ -/]*[@-~]")

// normalizeLine turns one raw line off the pipe into text worth matching against.
//
// What it undoes is Windows-only in practice but harmless anywhere: one code path means every platform's tests cover
// it, and a line that needs none of it comes back unchanged.
func normalizeLine(line string) string {
	line = strings.TrimRight(line, "\r\n")

	// The low half of a wide newline is what ended the previous line; its high half opens this one.
	line = strings.TrimPrefix(line, "\x00")

	if decoded, ok := decodeUTF16(line); ok {
		line = decoded
	}

	// An escape byte is present in every sequence ansiRe can match, so this skips the substitution - two allocations
	// and two copies of the whole line, even when nothing matches - on the lines that carry no colour, which on the
	// platforms ORT does not colourize is all of them.
	if strings.IndexByte(line, 0x1b) >= 0 {
		line = ansiRe.ReplaceAllString(line, "")
	}

	return strings.TrimRight(line, "\r\n")
}

// decodeUTF16 converts a UTF-16LE line to UTF-8, reporting false for anything already narrow.
//
// ONNX Runtime logs through std::wcerr on Windows. Against a console that is invisible - the runtime hands the wide
// characters to WriteConsoleW - but a pipe receives the wchar_t buffer as it stands, two bytes per character. Setting
// the descriptor to _O_U8TEXT would have the runtime convert instead, at the cost of every narrow writer sharing it,
// whose bytes it would then read back as UTF-16; decoding here costs one scan of the line and leaves them alone.
//
// A NUL is the tell. It cannot appear in a line of narrow text - the reader splits on newlines, and nothing else puts
// one there - while a wide line carries one in every ASCII character.
func decodeUTF16(s string) (string, bool) {
	if !strings.ContainsRune(s, 0) {
		return "", false
	}

	// An odd length means the last character was cut in half; that is a reason to drop it, not the rest of the line.
	units := make([]uint16, 0, len(s)/2)
	for i := 0; i+1 < len(s); i += 2 {
		units = append(units, uint16(s[i])|uint16(s[i+1])<<8)
	}

	return string(utf16.Decode(units)), true
}

// truncate caps a message at maxLoggedLine bytes, cutting on a rune boundary and saying how much was dropped.
func truncate(s string) string {
	if len(s) <= maxLoggedLine {
		return s
	}

	cut := maxLoggedLine
	for cut > 0 && !utf8.RuneStart(s[cut]) {
		cut--
	}

	return fmt.Sprintf("%s… (truncated, %d bytes)", s[:cut], len(s))
}

// endregion
