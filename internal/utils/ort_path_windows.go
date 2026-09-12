//go:build windows

package utils

import (
	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"golang.org/x/sys/windows"
)

// ortCachePath returns a spelling of dir that ONNX Runtime's narrow-string filesystem calls can open, reporting false
// when there is none.
//
// ORT hands trt_engine_cache_path and trt_timing_cache_path to std::filesystem as a narrow string, which on Windows is
// decoded with the process ANSI code page rather than UTF-8. A user whose profile directory is outside ASCII -
// "C:\Users\Роман\AppData\Roaming\open-photo-ai\engines\dt_newyork_fp32" - therefore has the path mojibaked before it
// reaches CreateDirectory, which fails with ERROR_PATH_NOT_FOUND and takes the session build down with it. That is
// microsoft/onnxruntime#8975, open since 2021 and still present in the 1.26 runtime pinned here.
//
// The failure is not visible as a path problem from the outside: the session build fails, the model falls back to the
// CPU, and a machine with a 5070 Ti quietly runs everything on its processor for no reason the user can see. It cost
// the two accounts it hit 46 fallbacks over 30 days.
//
// The 8.3 short name is the way out, because it is ASCII by construction and so survives the round trip. It exists
// only where the volume has 8.3 name creation enabled, which is why this reports failure rather than returning
// something hopeful: the caller turns that cache off instead, which costs a rebuild per session rather than the
// session itself.
//
// dir must already exist - GetShortPathName resolves against the filesystem, not lexically. Every caller comes through
// internal.ConfigDir, which creates it.
func ortCachePath(dir string) (string, bool) {
	if isASCIIPath(dir) {
		return dir, true
	}

	short, err := shortPathName(dir)
	if err != nil {
		internal.Log().Warn("cannot resolve a short path for the provider cache directory", "dir", dir, "err", err)
		return "", false
	}

	// 8.3 generation is per-volume and can be turned off, in which case the call succeeds and hands back the long
	// name unchanged - so the ASCII check has to be repeated on the result rather than assumed from the success.
	if !isASCIIPath(short) {
		internal.Log().Warn("the provider cache directory has no ASCII spelling; 8.3 names look disabled on this "+
			"volume, so the cache is off for this session", "dir", dir)
		return "", false
	}

	return short, true
}

func shortPathName(dir string) (string, error) {
	long, err := windows.UTF16PtrFromString(dir)
	if err != nil {
		return "", errors.Wrap(err, "failed to convert the path to UTF-16")
	}

	// Called twice on purpose: the first call sizes the buffer, the second fills it. n counts characters including
	// the terminating NUL on the sizing call, which is exactly the length the second call wants.
	n, err := windows.GetShortPathName(long, nil, 0)
	if err != nil {
		return "", errors.Wrap(err, "failed to size the short path")
	}

	buf := make([]uint16, n)
	if _, err = windows.GetShortPathName(long, &buf[0], n); err != nil {
		return "", errors.Wrap(err, "failed to read the short path")
	}

	return windows.UTF16ToString(buf), nil
}
