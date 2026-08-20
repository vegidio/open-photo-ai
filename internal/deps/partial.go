package deps

import (
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/vegidio/open-photo-ai/internal"
)

// partStateSchema versions the sidecar. A record written by a different schema is treated as
// absent, which costs a restart from zero rather than a resume onto bytes whose meaning has moved.
const partStateSchema = 1

// stalePartAge is how long an abandoned part file is kept before it is swept.
//
// It is generous on purpose. models/ is shared, so another dependency's install may legitimately
// have a transfer in flight while this one sweeps, and the package already accepts (see the note on
// locks in install.go) that two copies of the app racing costs a duplicated download rather than a
// corrupted one. Reclaiming disk a fortnight late is the cheaper mistake.
const stalePartAge = 14 * 24 * time.Hour

// partState describes what a `.part` file holds, so a later run can tell whether appending to it
// would produce the artifact it is now being asked for.
//
// The bytes alone cannot answer that. Release asset names repeat across tags - `cuda_linux_amd64.7z`
// is the name under every `cuda/*` release - so a part file left by an interrupted install of one
// version is indistinguishable on disk from a part file of the next. What separates them is the URL
// and the pinned hash, and neither is recoverable from the file.
type partState struct {
	Schema int    `json:"schema"`
	URL    string `json:"url"`
	Sha256 string `json:"sha256"`
	Size   int64  `json:"size"`

	// ETag and Modified are the server's own account of what it served. They catch the one case the
	// URL and the pinned hash cannot: an artifact republished at the same address, for a dependency
	// installed without a pinned hash at all.
	ETag     string `json:"etag,omitempty"`
	Modified string `json:"modified,omitempty"`
}

// partPaths names the two files a transfer in progress owns: the bytes, and the record of what they
// are. Both are derived here so the four places that create, read, rename and delete them cannot
// disagree.
func partPaths(dir, name string) (part, state string) {
	part = filepath.Join(dir, "."+name+partSuffix)

	return part, part + stateSuffix
}

// readPartState returns the record beside a part file, reporting false when there is none to read
// or it was written by a schema this build does not know.
func readPartState(path string) (partState, bool) {
	data, err := os.ReadFile(path)
	if err != nil {
		return partState{}, false
	}

	var state partState
	if err = json.Unmarshal(data, &state); err != nil || state.Schema != partStateSchema {
		return partState{}, false
	}

	return state, true
}

// writePartState records what the part file is being filled with. It goes through the same atomic
// write as a manifest: a torn record would describe a resume that isn't valid, which is worse than
// no record at all.
func writePartState(path string, state partState) error {
	state.Schema = partStateSchema

	return internal.WriteJSONAtomic(filepath.Dir(path), filepath.Base(path), state)
}

// removePart drops a transfer's bytes and its record together. They are only meaningful as a pair -
// bytes with no record cannot be resumed, and a record with no bytes describes nothing.
func removePart(part, state string) {
	os.Remove(part)
	os.Remove(state)
}

// resumable reports whether have bytes already on disk can be continued towards src.
//
// Every field has to agree. The URL is what distinguishes two releases that share an asset name;
// the pinned hash is what distinguishes two artifacts that shared a URL; and the recorded size is
// what catches a part file longer than the artifact it claims to be, which cannot be a prefix of
// anything.
func (s partState) resumable(src Source, have int64) bool {
	switch {
	case have <= 0:
		return false
	case s.URL != src.URL:
		return false
	case s.Sha256 != src.Sha256:
		return false
	case src.Size > 0 && s.Size != src.Size:
		return false
	case s.Size > 0 && have >= s.Size:
		// A complete part file was never renamed, so the run that wrote it did not get as far as
		// verifying it. Starting over is the honest reading of that.
		return false
	}

	return true
}

// unchanged reports whether the server is still serving what the part file was filled from. A
// mismatch means the artifact moved under a stable URL, and the bytes on disk are a prefix of
// something that no longer exists.
func (s partState) unchanged(resp *http.Response) bool {
	if etag := resp.Header.Get("ETag"); etag != "" && s.ETag != "" {
		return etag == s.ETag
	}

	if modified := resp.Header.Get("Last-Modified"); modified != "" && s.Modified != "" {
		return modified == s.Modified
	}

	// Neither side offered a validator. The URL and pinned hash already matched, which for a pinned
	// release archive is the whole of the identity; the final hash check is what backs this up.
	return true
}
