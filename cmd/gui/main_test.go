package main

import (
	"fmt"
	stdos "os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/wailsapp/wails/v3/pkg/application"
)

// machineHasRealSessionBus reports whether the well-known sockets exist, so a test that wants to assert "no bus" can
// step aside on a developer machine that genuinely has one rather than asserting something about the machine.
func machineHasRealSessionBus() (string, bool) {
	runtimeDir := fmt.Sprintf("/run/user/%d", stdos.Getuid())

	for _, name := range []string{"bus", "dbus-session"} {
		path := filepath.Join(runtimeDir, name)
		if _, err := stdos.Stat(path); err == nil {
			return path, true
		}
	}

	return "", false
}

func TestHasSessionBusAcceptsAnExplicitAddress(t *testing.T) {
	t.Setenv("DBUS_SESSION_BUS_ADDRESS", "unix:path=/tmp/opai-test-bus")

	if !hasSessionBus() {
		t.Error("hasSessionBus() = false while an address is set; an explicit one must be accepted")
	}
}

// "autolaunch:" asks for a private bus to be spawned rather than naming one that exists, and a private bus is shared
// with nobody - so the lock built on it would guard nothing. godbus skips the value for the same reason.
func TestHasSessionBusIgnoresAutolaunch(t *testing.T) {
	if path, ok := machineHasRealSessionBus(); ok {
		t.Skipf("this machine has a real session bus at %s, which is a legitimate true", path)
	}

	t.Setenv("DBUS_SESSION_BUS_ADDRESS", "autolaunch:")

	if hasSessionBus() {
		t.Error(`hasSessionBus() = true for "autolaunch:"; that names a bus to spawn, not one that exists`)
	}
}

// The lock is what keeps a second copy of the app off the same cache directory, so the fields Wails needs to build it
// have to be present. ExitCode matters on its own: a non-zero one would report an ordinary second launch as a crash.
func TestSingleInstanceOptionsAreComplete(t *testing.T) {
	if runtime.GOOS == "linux" && !hasSessionBus() {
		t.Skip("no session bus reachable here, so nil is the correct result and the fields do not exist to check")
	}

	var raised bool
	opts := singleInstanceOptions(func() { raised = true })

	if opts == nil {
		t.Fatal("singleInstanceOptions() = nil while a session bus is reachable")
	}
	if opts.UniqueID != singleInstanceUniqueID {
		t.Errorf("UniqueID = %q, want %q", opts.UniqueID, singleInstanceUniqueID)
	}
	if opts.ExitCode != 0 {
		t.Errorf("ExitCode = %d, want 0 - a second launch is not a failure the user caused", opts.ExitCode)
	}
	if opts.OnSecondInstanceLaunch == nil {
		t.Fatal("OnSecondInstanceLaunch is nil, so a second launch would raise nothing")
	}

	opts.OnSecondInstanceLaunch(application.SecondInstanceData{})
	if !raised {
		t.Error("OnSecondInstanceLaunch did not call through to the handler it was given")
	}
}

// The ID is baked into a Windows mutex name, a macOS lock file and a D-Bus name, so it has to stay put across
// releases: a value that moved would let two versions run side by side, both opening the same cache directory.
func TestSingleInstanceUniqueIDIsStable(t *testing.T) {
	if singleInstanceUniqueID != "io.vinicius.opai" {
		t.Errorf("singleInstanceUniqueID = %q; changing it lets an older running copy go undetected",
			singleInstanceUniqueID)
	}
}
