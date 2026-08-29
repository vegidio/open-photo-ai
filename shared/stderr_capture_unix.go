//go:build darwin || linux

package shared

import (
	"os"

	"golang.org/x/sys/unix"
)

// redirectStderr points file descriptor 2 at w, returning the original descriptor as a file the caller installs as
// os.Stderr, plus the call that puts descriptor 2 back.
func redirectStderr(w *os.File) (*os.File, func() error, error) {
	// Literally descriptor 2, not os.Stderr.Fd(): what matters is the one libonnxruntime's std::cerr resolves to,
	// which is 2 regardless of what os.Stderr happens to point at.
	savedFd, err := unix.Dup(unix.Stderr)
	if err != nil {
		return nil, nil, err
	}

	// The GUI re-execs itself on Linux to set LD_LIBRARY_PATH; a stray duplicate of the terminal surviving into the
	// new image is at best confusing.
	unix.CloseOnExec(savedFd)

	// unix.Dup2 rather than syscall.Dup2: linux/arm64 has no dup2 syscall at all and the standard library omits the
	// function there, so this would not compile for one of the published targets. x/sys routes it through dup3.
	if err = unix.Dup2(int(w.Fd()), unix.Stderr); err != nil {
		_ = unix.Close(savedFd)
		return nil, nil, err
	}

	saved := os.NewFile(uintptr(savedFd), "/dev/stderr")

	return saved, func() error { return unix.Dup2(savedFd, unix.Stderr) }, nil
}
