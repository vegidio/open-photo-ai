//go:build windows

package shared

import (
	"os"
	"runtime"
	"unsafe"

	"github.com/cockroachdb/errors"
	"golang.org/x/sys/windows"
)

// The C runtime's descriptor calls, resolved from ucrtbase.dll by name.
//
// This is the whole trick. libonnxruntime.dll is an MSVC /MD build, so its std::cerr resolves stderr through the
// Universal CRT - and the UCRT lives in exactly one place, ucrtbase.dll, shared by every module in the process that
// links it dynamically, this binary's own cgo included. There is one descriptor table, so a _dup2 issued here is the
// same _dup2 the DLL's stdio observes. Calling it through a lazy DLL rather than cgo keeps the package free of a C
// dependency it would otherwise need for three one-line calls.
var (
	ucrtbase = windows.NewLazySystemDLL("ucrtbase.dll")

	procOpenOSFHandle = ucrtbase.NewProc("_open_osfhandle")
	procDup           = ucrtbase.NewProc("_dup")
	procDup2          = ucrtbase.NewProc("_dup2")
	procClose         = ucrtbase.NewProc("_close")
	procFileNo        = ucrtbase.NewProc("_fileno")
	procFreopen       = ucrtbase.NewProc("freopen")
	procIOB           = ucrtbase.NewProc("__acrt_iob_func")
)

const (
	// stderrFd is descriptor 2 in the C runtime's table - not os.Stderr, which on Windows is a raw handle Go keeps for
	// itself and which the calls below deliberately leave alone.
	stderrFd = 2

	// stderrIOB indexes the stderr stream in the C runtime's table of standard FILE handles.
	stderrIOB = 2

	// nulDevice is the Windows bit bucket, and the only writable path guaranteed to exist. It is what a detached
	// stderr stream is reopened on, to give it a descriptor that can then be pointed somewhere useful.
	nulDevice = "NUL"
)

// redirectStderr points the C runtime's descriptor 2 at w, and the process's standard error handle with it.
//
// Both are needed and neither is enough on its own. SetStdHandle covers anything that asks Windows for the standard
// error handle, but not a C runtime that already seeded its descriptor table from it - which ucrtbase has by the time
// any Go code runs. _dup2 covers that table, which is where ORT's writes actually land.
//
// The saved file handed back is a duplicate of the console, and the caller has to install it: a descriptor owns its
// handle, so pointing descriptor 2 somewhere else closes the one it had - the very handle Go captured for os.Stderr at
// startup. Left alone, os.Stderr would name a closed handle, and a closed handle on Windows is worse than a dead one:
// the value is reused, and a later print could land in whatever object inherited it.
func redirectStderr(w *os.File) (*os.File, func() error, error) {
	// A descriptor owns the handle it was opened on: _close(2), and the _dup2 below when it restores, would close
	// this one. Go owns w, so the runtime gets a duplicate of its own to destroy.
	proc := windows.CurrentProcess()

	prev, err := windows.GetStdHandle(windows.STD_ERROR_HANDLE)
	if err != nil {
		prev = 0
	}

	// A GUI-subsystem process has no console to preserve, and nothing to hand back.
	saved := duplicateStderr(proc, prev)

	// Nothing below is installed yet, so a failure gives the duplicate straight back rather than leaking it.
	var pipe windows.Handle
	if err := windows.DuplicateHandle(proc, windows.Handle(w.Fd()), proc, &pipe, 0, false,
		windows.DUPLICATE_SAME_ACCESS); err != nil {
		closeSaved(saved)
		return nil, nil, errors.Wrap(err, "failed to duplicate the pipe handle")
	}

	fd, err := openOSFHandle(pipe)
	if err != nil {
		_ = windows.CloseHandle(pipe)
		closeSaved(saved)

		return nil, nil, err
	}

	// A GUI-subsystem process starts with no standard error at all, so there is genuinely nothing to save and _dup
	// fails; that is a restore with nothing to put back, not an error.
	savedFd := crtDup(stderrFd)

	if err = crtDup2(fd, stderrFd); err != nil {
		_ = crtClose(fd)

		if savedFd >= 0 {
			_ = crtClose(savedFd)
		}

		closeSaved(saved)

		return nil, nil, err
	}

	// The descriptor is only half of it: the runtime's stderr stream has its own, and in a GUI process it has none.
	reopened := redirectStderrStream(fd)

	// Descriptor 2 carries its own duplicate now; this one was only ever the vehicle.
	_ = crtClose(fd)

	// Best effort, and the reason its failure is not fatal: the descriptors above are what ORT writes through, and
	// they are already redirected. This only catches writers that go to the Win32 handle instead.
	_ = windows.SetStdHandle(windows.STD_ERROR_HANDLE, windows.Handle(w.Fd()))

	return saved, func() error { return restoreStderr(savedFd, prev, reopened) }, nil
}

// closeSaved drops a duplicate the caller never got to install. A GUI process has none to drop.
func closeSaved(saved *os.File) {
	if saved != nil {
		_ = saved.Close()
	}
}

// duplicateStderr copies the console handle for Go's own use, or reports nil when there is nothing to copy.
func duplicateStderr(proc windows.Handle, stderr windows.Handle) *os.File {
	if stderr == 0 || stderr == windows.InvalidHandle {
		return nil
	}

	var dup windows.Handle
	if err := windows.DuplicateHandle(proc, stderr, proc, &dup, 0, false,
		windows.DUPLICATE_SAME_ACCESS); err != nil {
		return nil
	}

	return os.NewFile(uintptr(dup), "CONERR$")
}

// redirectStderrStream points the C runtime's stderr stream at fd, reporting whether it had to be reopened to get
// there.
//
// A console process needs none of this: its stream already sits on descriptor 2, which the caller has redirected. A
// GUI-subsystem process is the interesting one. It starts with no standard error at all, so the runtime marks the
// stream detached - _fileno returns -2, not a descriptor - and every write through it is dropped on the floor,
// whatever descriptor 2 happens to be. That is the state ORT logs into: std::wcerr sits on this very stream, so
// without this its warnings are discarded in exactly the build that has no terminal to print them to.
//
// Reopening on NUL is what breaks the deadlock. It costs a descriptor and gives the stream a real one, which _dup2
// then points at the pipe like any other.
func redirectStderrStream(fd int) bool {
	stream := stderrStream()
	if stream == 0 {
		return false
	}

	reopened := false

	if streamFd(stream) < 0 {
		if !freopenNUL(stream) {
			return false
		}

		reopened = true
	}

	// Nothing to do when the stream already sits on descriptor 2: the caller pointed that at the pipe.
	if target := streamFd(stream); target >= 0 && target != stderrFd {
		_ = crtDup2(fd, target)
	}

	return reopened
}

// restoreStderr puts descriptor 2, the stderr stream and the standard error handle back the way they were.
func restoreStderr(saved int, prev windows.Handle, reopened bool) error {
	if prev != 0 {
		_ = windows.SetStdHandle(windows.STD_ERROR_HANDLE, prev)
	}

	// A stream this package attached to the pipe goes back to the bit bucket rather than to a descriptor that is about
	// to close - which is where its writes went before any of this, in a process with no stderr to speak of.
	if reopened {
		if stream := stderrStream(); stream != 0 {
			_ = freopenNUL(stream)
		}
	}

	// Either way the pipe's descriptor is gone afterwards - _dup2 closes what it overwrites - which is what lets the
	// reader see EOF once the caller drops its own handle.
	if saved < 0 {
		return crtClose(stderrFd)
	}

	err := crtDup2(saved, stderrFd)
	_ = crtClose(saved)

	return err
}

// region - Private functions

// openOSFHandle wraps a Win32 handle in a C runtime descriptor. The flags are left at zero: they select append and
// text-mode behaviour, and this descriptor is only ever written to as a raw byte stream.
func openOSFHandle(h windows.Handle) (int, error) {
	r, _, _ := procOpenOSFHandle.Call(uintptr(h), 0)

	fd := int(int32(r))
	if fd < 0 {
		return -1, errors.New("failed to open a runtime descriptor on the pipe handle")
	}

	return fd, nil
}

// crtDup duplicates a C runtime descriptor, returning a negative value when there is none to duplicate. That is the
// ordinary case for a GUI-subsystem process rather than an error, which is why there is no error to return.
func crtDup(fd int) int {
	r, _, _ := procDup.Call(uintptr(fd))
	return int(int32(r))
}

func crtDup2(from, to int) error {
	r, _, _ := procDup2.Call(uintptr(from), uintptr(to))

	if int(int32(r)) < 0 {
		return errors.Newf("failed to point descriptor %d at %d", to, from)
	}

	return nil
}

// stderrStream returns the runtime's stderr FILE, or zero if it cannot be reached. The C "stderr" is a macro over
// this table lookup, so there is nothing else to import.
func stderrStream() uintptr {
	r, _, _ := procIOB.Call(uintptr(stderrIOB))
	return r
}

// streamFd is the descriptor behind a stream, negative for one that has none.
func streamFd(stream uintptr) int {
	r, _, _ := procFileNo.Call(stream)
	return int(int32(r))
}

// freopenNUL reopens a stream on the null device, reporting whether it worked.
func freopenNUL(stream uintptr) bool {
	path, err := windows.BytePtrFromString(nulDevice)
	if err != nil {
		return false
	}

	mode, err := windows.BytePtrFromString("w")
	if err != nil {
		return false
	}

	r, _, _ := procFreopen.Call(uintptr(unsafe.Pointer(path)), uintptr(unsafe.Pointer(mode)), stream)

	// The pointers have to outlive the call itself, which takes them as plain integers.
	runtime.KeepAlive(path)
	runtime.KeepAlive(mode)

	return r != 0
}

func crtClose(fd int) error {
	r, _, _ := procClose.Call(uintptr(fd))

	if int(int32(r)) < 0 {
		return errors.Newf("failed to close descriptor %d", fd)
	}

	return nil
}

// endregion
