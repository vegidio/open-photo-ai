package deps

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"io"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	"golang.org/x/sync/errgroup"
)

// downloadClient has bounded connect and header timeouts so a stalled server can't hang Initialize indefinitely. Body
// reads are left unbounded because a dependency can be a couple of gigabytes on a slow link.
var downloadClient = &http.Client{
	Transport: &http.Transport{
		DialContext: (&net.Dialer{
			Timeout:   30 * time.Second,
			KeepAlive: 30 * time.Second,
		}).DialContext,
		TLSHandshakeTimeout:   30 * time.Second,
		ResponseHeaderTimeout: 30 * time.Second,
		IdleConnTimeout:       90 * time.Second,
		ExpectContinueTimeout: 5 * time.Second,
	},
}

// un7zip is a variable so tests can install a stub and exercise the manifest around extraction without carrying a
// binary fixture for every case.
var un7zip = fs.Un7zip

// locks serialises installs that share a manifest. Two different models install into the same directory concurrently,
// and an install is a read-modify-write of the directory as much as of its record.
//
// This is in-process only. Two copies of the app racing would both write the same bytes, so the worst outcome is a
// duplicated download, which is not worth a lock file and its staleness rules.
var locks sync.Map

// Install brings a dependency on disk up to date, downloading and expanding whatever is missing or stale, and records
// what it put there.
//
// Nothing is trusted that isn't recorded: the manifest is removed before the first byte is written and only written
// again once every file is in place, so an interruption at any point leaves a directory that is explicitly mid-install
// and gets reinstalled on the next call. That is also why there is no staging directory - it would double peak disk
// for a multi-gigabyte archive to close a window this ordering already closes.
func Install(ctx context.Context, dep Dependency, onProgress types.DownloadProgress) error {
	mu, _ := locks.LoadOrStore(dep.Destination+"/"+dep.manifestName(), &sync.Mutex{})
	mu.(*sync.Mutex).Lock()
	defer mu.(*sync.Mutex).Unlock()

	if err := validate(dep); err != nil {
		return err
	}

	dir, err := configDir(dep.Destination)
	if err != nil {
		return err
	}

	// The debug override: a model dropped in by hand has no manifest at all and must still be used as it is.
	if dep.SkipVerify && sourcesPresent(dir, dep) {
		internal.Log().Warn("model verification skipped; using the files on disk", "dep", dep.Name)
		return nil
	}

	want := fingerprint(dep)

	old, hasOld := readManifest(dir, dep.manifestName())
	if hasOld && old.Fingerprint == want && old.intact(dir) {
		internal.Log().Debug("dependency present", "dep", dep.Name, "dir", dep.Destination)
		return nil
	}

	internal.Log().Info("installing dependency",
		"dep", dep.Name, "version", dep.Version, "dir", dep.Destination, "sources", len(dep.Sources))

	// Only now that an install is certain: a `.old` file is the leftover of a previous one, so there is nothing to
	// sweep on the steady-state path above, where this would have listed the whole shared models directory on every
	// model acquisition.
	sweepRenamed(dir)

	if err = os.Remove(filepath.Join(dir, dep.manifestName())); err != nil && !os.IsNotExist(err) {
		return errors.Wrap(err, "failed to drop the previous manifest")
	}

	switch {
	case hasOld:
		if err = old.remove(dir); err != nil {
			return err
		}

	case dep.Exclusive:
		// An exclusive directory with no manifest was populated by a version of the app that didn't keep one. Its
		// contents can't be described, so extracting over them would silently merge two versions - the previous
		// release's shared libraries would stay behind for the loader to find.
		if err = EmptyDir(dir); err != nil {
			return err
		}
	}

	// Before any download, so an interrupted reinstall can never leave new weights paired with an engine compiled from
	// the old ones.
	if err = removeDerived(dep); err != nil {
		return err
	}

	installed, err := fetch(ctx, dir, dep, onProgress)
	if err != nil {
		return err
	}

	if dep.Exclusive {
		if installed, err = recordTree(dir, dep.manifestName()); err != nil {
			return err
		}
	}

	if err = writeManifest(dir, dep.manifestName(), Manifest{
		Schema:      manifestSchema,
		Name:        dep.Name,
		Version:     dep.Version,
		Fingerprint: want,
		Files:       installed,
	}); err != nil {
		return err
	}

	internal.Log().Info("dependency ready", "dep", dep.Name, "files", len(installed))
	return nil
}

// region - Private functions

// validate rejects a dependency whose record could not describe what it installs.
//
// The case worth catching is an archive in a shared destination: extraction produces a file list nobody declared, and
// only an exclusive destination can be read back to find out what it was. Left unchecked it would install correctly and
// then record nothing, so every start would find an empty record and download it all again.
func validate(dep Dependency) error {
	if len(dep.Sources) == 0 {
		return errors.Newf("dependency %s has no sources", dep.Name)
	}

	if !dep.Exclusive {
		for _, src := range dep.Sources {
			if isArchive(src.FileName()) {
				return errors.Newf("dependency %s installs an archive into the shared directory %s",
					dep.Name, dep.Destination)
			}
		}
	}

	return nil
}

// isArchive reports whether a downloaded file has to be expanded rather than kept as it is.
func isArchive(name string) bool {
	return strings.EqualFold(filepath.Ext(name), ".7z")
}

// maxParallelDownloads is how many sources are fetched at once. Osaka's three graphs and the NVIDIA libraries are
// several gigabytes over separate connections, and a single stream rarely saturates the link - but past a handful the
// per-connection share drops without the total improving, and every extra stream is another partial file to clean up
// if the install fails.
const maxParallelDownloads = 3

// fetch downloads every source into dir, expanding archives, and returns what it wrote. The return value only covers
// files downloaded directly; an archive's contents are read back off the disk by recordTree, since extraction is the one
// step that produces files nobody declared.
//
// Downloading and expanding are two phases on purpose. The transfers run concurrently, because they are independent and
// network-bound; the extraction that follows runs in declared order, one at a time, because two archives expanding into
// one directory would be writing the same paths with nothing to arbitrate. Keeping the disk mutation serial is what
// makes the concurrency above it safe to reason about.
func fetch(ctx context.Context, dir string, dep Dependency, onProgress types.DownloadProgress) ([]File, error) {
	prog := newAggregate(dep, onProgress)

	// Indexed rather than appended: the manifest should list the sources in the order the dependency declares them,
	// which is not the order concurrent downloads finish in.
	fetched := make([]File, len(dep.Sources))

	group, groupCtx := errgroup.WithContext(ctx)
	group.SetLimit(maxParallelDownloads)

	for i, src := range dep.Sources {
		group.Go(func() error {
			name := src.FileName()
			part := filepath.Join(dir, "."+name+partSuffix)

			sum, size, err := downloadTo(groupCtx, src.URL, part, prog)
			if err != nil {
				os.Remove(part)
				return errors.Wrapf(err, "failed to download %s", name)
			}

			if src.Sha256 != "" && sum != src.Sha256 {
				os.Remove(part)

				if !dep.SkipVerify {
					return errors.Newf("hash mismatch for %s: expected %s, got %s", name, src.Sha256, sum)
				}

				internal.Log().Warn("hash mismatch ignored", "artifact", name, "expected", src.Sha256, "got", sum)
			}

			final := filepath.Join(dir, name)
			if err = os.Rename(part, final); err != nil {
				os.Remove(part)
				return errors.Wrapf(err, "failed to place %s", name)
			}

			fetched[i] = File{Path: name, Size: size, Sha256: sum}

			return nil
		})
	}

	if err := group.Wait(); err != nil {
		return nil, err
	}

	installed := make([]File, 0, len(fetched))

	for _, file := range fetched {
		if !isArchive(file.Path) {
			installed = append(installed, file)
			continue
		}

		internal.Log().Info("extracting archive", "file", file.Path)

		final := filepath.Join(dir, file.Path)
		if err := un7zip(final, dir); err != nil {
			return nil, errors.Wrapf(err, "failed to extract %s", file.Path)
		}

		os.Remove(final)
	}

	prog.finish()

	return installed, nil
}

// downloadTo streams a URL to disk and returns the SHA-256 and size of what arrived.
//
// The hash is computed from the bytes as they pass through, not by reading the file back: the artifact is verified
// without ever being read twice, which on a multi-gigabyte archive is the difference between one pass over the data and
// three. O_TRUNC matters as much - without it a shorter artifact overwriting a longer leftover would keep the tail.
func downloadTo(ctx context.Context, url, dst string, prog *aggregate) (string, int64, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return "", 0, errors.Wrap(err, "failed to build the request")
	}

	resp, err := downloadClient.Do(req)
	if err != nil {
		return "", 0, errors.Wrap(err, "failed to send the request")
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return "", 0, errors.Newf("bad status: %s", resp.Status)
	}

	file, err := os.OpenFile(dst, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o644)
	if err != nil {
		return "", 0, errors.Wrap(err, "failed to create the destination file")
	}
	defer file.Close()

	hash := sha256.New()

	size, err := io.Copy(io.MultiWriter(file, hash), prog.wrap(resp.Body, resp.ContentLength))
	if err != nil {
		return "", 0, errors.Wrap(err, "failed to write the file")
	}

	return hex.EncodeToString(hash.Sum(nil)), size, nil
}

// sourcesPresent reports whether every source already has a file on disk under its own name. It backs the debug
// override only, where the question is "is there something here to use" rather than "is it the right thing".
func sourcesPresent(dir string, dep Dependency) bool {
	for _, src := range dep.Sources {
		if !fs.FileExists(filepath.Join(dir, src.FileName())) {
			return false
		}
	}

	// validate has already rejected a dependency with no sources, so reaching here means every one of them is present.
	return true
}

// EmptyDir removes every entry in dir without removing dir itself.
//
// Keeping the directory is the whole point: on Linux the config subdirectories are put on LD_LIBRARY_PATH before the
// process re-execs (see setLibPathAndRestart in cmd/gui) and the loader permanently skips a search path it finds
// missing, so a directory deleted here would stay unusable for the rest of the process lifetime. Manifest.remove keeps
// dir for the same reason.
func EmptyDir(dir string) error {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return errors.Wrapf(err, "failed to read %s", dir)
	}

	for _, e := range entries {
		if err = os.RemoveAll(filepath.Join(dir, e.Name())); err != nil {
			return errors.Wrapf(err, "failed to remove %s from %s", e.Name(), dir)
		}
	}

	return nil
}

// configDir resolves one of the slash-separated paths a Dependency names - Destination, or an entry in Derived - to an
// OS path under the user's config directory, creating it if it isn't there.
func configDir(rel string) (string, error) {
	dir, err := fs.MkUserConfigDir(internal.AppName, strings.Split(rel, "/")...)
	if err != nil {
		return "", errors.Wrapf(err, "failed to resolve the %s directory", rel)
	}

	return dir, nil
}

// removeDerived drops the caches built from a dependency. They are rebuilt by whoever owns them - for the execution
// providers, on the next session - so there is nothing to restore here.
func removeDerived(dep Dependency) error {
	for _, d := range dep.Derived {
		dir, err := configDir(d)
		if err != nil {
			return err
		}

		if err = EmptyDir(dir); err != nil {
			return err
		}

		internal.Log().Debug("cleared a derived cache", "dep", dep.Name, "dir", d)
	}

	return nil
}

// sweepRenamed deletes the files a previous install had to rename aside because Windows held them open. By the time
// another install runs, the process that had them mapped is long gone.
func sweepRenamed(dir string) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}

	for _, e := range entries {
		if strings.HasSuffix(e.Name(), oldSuffix) {
			os.Remove(filepath.Join(dir, e.Name()))
		}
	}
}

// endregion
