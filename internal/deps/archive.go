package deps

import (
	"context"
	"io/fs"
	"path/filepath"
	"time"

	"github.com/bodgit/sevenzip"
	"github.com/cockroachdb/errors"
	sak "github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
)

// extractSampleInterval is how often the destination is measured while an archive expands. Four
// times a second is smooth enough for a bar and cheap enough for a tree of a few hundred files.
const extractSampleInterval = 250 * time.Millisecond

// extractArchive expands a 7z archive, reporting the uncompressed bytes on disk as it goes.
//
// The reporting is done by watching the destination rather than by counting bytes through the
// extraction itself, because the extraction is go-sak's: fs.Un7zip owns the loop and takes no hook.
// Reimplementing it here to get one would mean re-deriving its Zip-Slip and symlink-escape guards,
// which is the wrong thing to fork for a progress bar. Sampling the tree costs a walk every quarter
// second and keeps the security-relevant code in one place. If go-sak ever grows a progress variant,
// this collapses into a call to it.
//
// ctx stops the sampling, not the expansion - fs.Un7zip has no cancellation of its own, and a
// half-written tree is handled by the manifest ordering in Install rather than by unwinding here.
func extractArchive(ctx context.Context, archive, dst string, onProgress func(done, total int64)) error {
	if onProgress == nil {
		return sak.Un7zip(archive, dst)
	}

	total, err := uncompressedSize(archive)
	if err != nil {
		// A header that cannot be read is the extraction's problem to report, not the progress
		// bar's. Expanding without a total still moves the bar - percent falls back to complete -
		// and fs.Un7zip will fail on the same file in a moment with a better message.
		internal.Log().Debug("could not size the archive; expanding without progress",
			"archive", filepath.Base(archive), "err", err)

		return sak.Un7zip(archive, dst)
	}

	// Everything already in the destination, including the archive itself, which is only deleted
	// once the expansion has succeeded. Subtracting it leaves just what the expansion is adding.
	baseline := treeSize(dst)

	onProgress(0, total)

	done := make(chan struct{})
	defer close(done)

	go func() {
		ticker := time.NewTicker(extractSampleInterval)
		defer ticker.Stop()

		for {
			select {
			case <-done:
				return
			case <-ctx.Done():
				return
			case <-ticker.C:
				onProgress(max(treeSize(dst)-baseline, 0), total)
			}
		}
	}()

	if err = sak.Un7zip(archive, dst); err != nil {
		return err
	}

	onProgress(total, total)

	return nil
}

// uncompressedSize reads the archive's header - not its contents - to find how much it expands to.
func uncompressedSize(archive string) (int64, error) {
	reader, err := sevenzip.OpenReader(archive)
	if err != nil {
		return 0, errors.Wrapf(err, "failed to open %s", filepath.Base(archive))
	}
	defer reader.Close()

	var total int64
	for _, file := range reader.File {
		total += int64(file.UncompressedSize)
	}

	return total, nil
}

// treeSize totals the bytes under dir, ignoring anything it cannot stat. It is a progress estimate,
// so a file that disappeared between the walk and the stat is worth skipping rather than reporting.
func treeSize(dir string) int64 {
	var total int64

	filepath.WalkDir(dir, func(_ string, entry fs.DirEntry, err error) error {
		if err != nil || entry.IsDir() {
			return nil //nolint:nilerr // an unreadable entry is skipped, not fatal to a size estimate
		}

		if info, statErr := entry.Info(); statErr == nil {
			total += info.Size()
		}

		return nil
	})

	return total
}
