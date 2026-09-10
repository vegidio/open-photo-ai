package shared

import (
	"bytes"
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
	// pollInterval is how often the tailer looks for new bytes. Native output is occasional - a handful of provider
	// warnings per session - so a poll costing one read(2) at the end of a file is cheaper than any of the ways to be
	// notified of a write, and Close reads to the end regardless, so no line waits on this interval to be logged.
	pollInterval = 150 * time.Millisecond

	// readChunk is how much the tailer takes per read. A buffer size, not a limit: readAvailable loops until the file
	// yields nothing.
	readChunk = 64 << 10

	// maxPendingLine bounds the partial line held across polls, so a native writer that emits megabytes without a
	// newline cannot grow it without limit. It is mebibytes rather than kilobytes because ORT's node-assignment
	// warning names every node that fell back to another provider and runs past 512 KiB on a large model - splitting
	// that into arbitrary chunks would read worse than logging it whole and letting truncate cap the record.
	maxPendingLine = 4 << 20

	// maxLoggedLine caps how much of a single line makes it into a log record; see truncate.
	maxLoggedLine = 16 << 10
)

// stderrCapture points the process's stderr at a file and turns everything written to it into log records.
//
// It exists for ONNX Runtime. ORT logs from C++ through std::cerr - std::wcerr on Windows - and the Go binding creates
// the environment with plain CreateEnv rather than CreateEnvWithCustomLogger, so there is no callback to install and no
// sink to swap. Its log level can be changed; its destination cannot. Redirecting what the process calls stderr is the
// only interception point there is, which is also why this catches the CoreML, CUDA and TensorRT providers, and
// anything else in the process writing to stderr.
//
// A file rather than a pipe, and that choice is the whole point of this type. A pipe exists only in memory, and its
// contents reach the log through a goroutine; when the process dies from a signal every goroutine stops at once, so
// whatever had not been read yet is gone. That is not hypothetical - issue #40 is a native crash inside a CUDA
// session, and both logs the reporter attached simply stop mid-session, because the C++ account of the failure was
// sitting unread in the pipe at the moment of death. Bytes written to a file are already the kernel's before write(2)
// returns, with no goroutine in the path, so they survive an abort. The runtime's own crash trace lands there too,
// since it is written to descriptor 2 directly.
//
// The cost is that the last lines before a crash are on disk but not yet in opai.log, because the tailer never got to
// them. foldNativeLog replays them on the next run.
//
// What "stderr" means is where the platforms part company, and redirectStderr owns that difference: a descriptor on
// Unix, and on Windows a C runtime descriptor, the stream sitting on it and the Win32 handle behind both.
//
// It stays unexported on purpose. Its preconditions - a logger that does not write to stderr, and a start that beats
// every goroutine in the process - cannot be enforced by the type, so SetupLogging is the only call site there is.
type stderrCapture struct {
	logger *slog.Logger

	// write is what descriptor 2 now names; read is an independent handle the tailer walks forward over the same
	// file. Two handles rather than one because the writer is C++ and owns its own offset.
	write *os.File
	read  *os.File
	path  string

	// pending holds the bytes since the last newline. A native writer can be caught mid-line by the poll interval,
	// and half a line is not a record.
	pending []byte
	buf     []byte

	restore func() error

	stop chan struct{}
	done chan struct{}

	closeOnce sync.Once
	closeErr  error
}

// startStderrCapture redirects stderr into the file at path, logging each completed line through logger.
//
// The logger must not write to stderr: it would feed its own input. Go's own stderr is preserved - os.Stderr and the
// runtime's crash output are pointed at a duplicate of the original, so prints and panic traces still reach the
// terminal, and keep reaching it after the capture is closed. That reassignment of the package-level os.Stderr is a
// process-wide write, safe only because SetupLogging runs at the top of main before any goroutine exists; do not move
// the call later.
func startStderrCapture(logger *slog.Logger, path string) (*stderrCapture, error) {
	// O_TRUNC because foldNativeLog has already replayed whatever a previous run left here. Starting empty is what
	// lets a non-empty file at startup mean "the last run did not shut down cleanly" rather than "these may be old".
	write, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o600)
	if err != nil {
		return nil, err
	}

	read, err := os.Open(path)
	if err != nil {
		_ = write.Close()
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
		write:   write,
		read:    read,
		path:    path,
		buf:     make([]byte, readChunk),
		restore: restore,
		stop:    make(chan struct{}),
		done:    make(chan struct{}),
	}

	if saved != nil {
		os.Stderr = saved

		// The runtime writes an unhandled panic or a fatal error to descriptor 2, which now names the capture file,
		// so the trace reaches disk on its own. This additionally keeps it on the terminal for anyone running the
		// binary from one. SetCrashOutput duplicates the descriptor internally, so saved's lifetime is not a concern.
		_ = debug.SetCrashOutput(saved, debug.CrashOptions{})
	}

	go c.tail()

	return c, nil
}

// Close restores stderr, logs whatever is left in the file and stops the tailer. It is idempotent.
func (c *stderrCapture) Close() error {
	c.closeOnce.Do(func() {
		// Stderr first: from here on native writes go back to the terminal rather than to a file nobody is reading.
		if err := c.restore(); err != nil {
			c.closeErr = err
		}

		// The tailer reads to the end once more before returning, so everything written up to the restore above is
		// logged. Unlike a pipe this cannot block - a read at the end of a file returns immediately - so there is no
		// drain timeout to get wrong, and no way for a stray duplicate of a write end to hang shutdown.
		close(c.stop)
		<-c.done

		_ = c.read.Close()
		_ = c.write.Close()

		// A clean shutdown has just logged every line, so leaving the file behind would have the next run replay all
		// of it. Removing it is also what gives a file present at startup its meaning.
		if err := os.Remove(c.path); err != nil && !os.IsNotExist(err) {
			c.logger.Warn("could not remove the native output file", "path", c.path, "err", err)
		}

		// os.Stderr keeps the saved duplicate rather than being put back: it names the same terminal the original did,
		// it stays open for the life of the process, and on Windows the original it would be put back to is a handle
		// that pointing descriptor 2 at the file has already closed.
	})

	return c.closeErr
}

// foldNativeLog replays into logger whatever native output a previous run left behind, then removes it.
//
// A file here means the last run never closed its capture - it crashed, or was killed - so these are lines that
// reached disk but not opai.log. They are also the ones that matter most: a native crash writes its account of itself
// immediately before the process dies, which is precisely the window that cannot be logged live. See stderrCapture.
//
// It must run before startStderrCapture, which truncates the file.
func foldNativeLog(logger *slog.Logger, path string) {
	data, err := os.ReadFile(path)
	if err != nil {
		if !os.IsNotExist(err) {
			logger.Warn("could not read the previous run's native output", "path", path, "err", err)
		}

		return
	}

	if len(bytes.TrimSpace(data)) == 0 {
		remove(logger, path)
		return
	}

	logger.Warn("the previous run left native output behind, so it did not exit cleanly; the records below are "+
		"from that run, not this one", "path", path, "bytes", len(data))

	for _, line := range strings.Split(string(data), "\n") {
		emitLine(logger, line, "previous_run", true)
	}

	logger.Warn("end of the previous run's native output")

	remove(logger, path)
}

func remove(logger *slog.Logger, path string) {
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		logger.Warn("could not remove the previous run's native output", "path", path, "err", err)
	}
}

// tail follows the capture file, logging each line as it is completed.
func (c *stderrCapture) tail() {
	defer close(c.done)

	ticker := time.NewTicker(pollInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			c.readAvailable()

		case <-c.stop:
			// One last pass, then flush a trailing line that never got its newline - the usual shape for a writer
			// that was interrupted, and still worth a record.
			c.readAvailable()
			c.flushPending()

			return
		}
	}
}

// readAvailable consumes everything the file holds beyond what has already been read.
func (c *stderrCapture) readAvailable() {
	for {
		n, err := c.read.Read(c.buf)
		if n > 0 {
			c.pending = append(c.pending, c.buf[:n]...)
			c.emitCompleteLines()
		}

		// io.EOF here means "nothing more for now" rather than "never again": the file grows under the reader, and
		// the next poll resumes from exactly this offset.
		if err != nil || n == 0 {
			return
		}
	}
}

// emitCompleteLines logs every whole line in pending and keeps the remainder for the next read.
func (c *stderrCapture) emitCompleteLines() {
	for {
		i := bytes.IndexByte(c.pending, '\n')
		if i < 0 {
			break
		}

		c.emit(string(c.pending[:i+1]))
		c.pending = c.pending[i+1:]
	}

	// Dropping the slice lets the backing array go rather than sliding its base forward for the life of the process.
	if len(c.pending) == 0 {
		c.pending = nil
		return
	}

	// A line this long means a native writer that has not emitted a newline in megabytes. Logging what there is beats
	// growing until the process runs out of memory, and truncate caps the record itself anyway.
	if len(c.pending) > maxPendingLine {
		c.flushPending()
	}
}

// flushPending logs a line that has no newline yet, and forgets it.
func (c *stderrCapture) flushPending() {
	if len(c.pending) == 0 {
		return
	}

	c.emit(string(c.pending))
	c.pending = nil
}

// emit turns one line into a log record.
func (c *stderrCapture) emit(line string) {
	emitLine(c.logger, line)
}

// emitLine turns one raw line of native output into a record on logger, with extra appended to whatever attributes
// the line itself carries.
//
// A package function rather than a method so foldNativeLog can replay a previous run's file through exactly the same
// parsing. A second copy of this would be a second place for ORT's log format to be understood.
func emitLine(logger *slog.Logger, line string, extra ...any) {
	line = normalizeLine(line)
	if strings.TrimSpace(line) == "" {
		return
	}

	if rec, ok := parseOrtLine(line); ok {
		logger.Log(context.Background(), rec.level, truncate(rec.msg), append(rec.attrs, extra...)...)
		return
	}

	// Not ORT's format: another library on the same descriptor, or a continuation line of a multi-line message. It is
	// exactly the output that used to reach the terminal, so it is kept - but it carries no severity of its own, and
	// INFO is the level that neither hides it nor overstates it.
	logger.Log(context.Background(), slog.LevelInfo, truncate(line), append([]any{"source", "stderr"}, extra...)...)
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
