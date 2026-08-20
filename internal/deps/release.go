package deps

import (
	"fmt"
	"runtime"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
)

// releaseUrl is where the project publishes its dependency archives. It is spelled out once here;
// the generator that pins their hashes reads the same releases through the GitHub API.
const releaseUrl = "https://github.com/vegidio/open-photo-ai/releases/download/%s/%s"

// ReleaseDependency describes an archive published as a release asset, for the platform this binary
// was built for. It is the second of the package's two constructors - ModelDependency covers the
// files fetched per model, this one covers the trees fetched per release.
//
// Everything that varies between releases comes from the generated artifact table: the tag the URL
// is built from, the hash the download is checked against, and the size the progress report is
// spread over. A version bump is therefore a regeneration of that table and nothing else - neither
// this function nor its callers name a release.
//
// prefix names the asset family ("onnx", "cuda"); name is what the dependency is called in logs and
// in its manifest. A release archive always owns its destination, because extraction produces a
// file list nobody declared and only an exclusive directory can be read back to find out what it
// was - see Dependency.Exclusive.
func ReleaseDependency(name, prefix, destination string) (Dependency, error) {
	pinned, found := internal.PinnedArchive(prefix)
	if !found {
		return Dependency{}, errors.Newf("no %s archive is pinned for %s/%s",
			prefix, runtime.GOOS, runtime.GOARCH)
	}

	return Dependency{
		Name:        name,
		Version:     pinned.Tag,
		Destination: destination,
		Exclusive:   true,
		Sources: []Source{{
			URL:    fmt.Sprintf(releaseUrl, pinned.Tag, pinned.Name),
			Sha256: pinned.Hash,
			Size:   pinned.Size,
		}},
	}, nil
}
