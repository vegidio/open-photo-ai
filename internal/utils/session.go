package utils

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/deps"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

// Session is an ONNX session together with the on-disk size of the files backing it, which is the proxy the model
// registry budgets against. Real device memory is larger - arenas, cuDNN workspaces and the CoreML MLProgram all sit
// on top of the weights - and none of it is queryable through these bindings, so the budget that consumes this number
// is deliberately conservative.
//
// The embedded session is promoted, so a Session is a drop-in for the *ort.DynamicAdvancedSession it wraps: Run and
// Destroy work unchanged.
//
// Models embed a *Session rather than holding it in a named field, which promotes ResidentBytes and Destroy and so
// satisfies types.Measurable and types.Destroyable without any per-model code. That is what stops a new model from
// silently being charged the default size because someone forgot to write the method.
type Session struct {
	*ort.DynamicAdvancedSession
	bytes int64
}

// ResidentBytes implements types.Measurable, reporting the on-disk size of the files behind this session.
func (s *Session) ResidentBytes() int64 {
	if s == nil {
		return 0
	}

	return s.bytes
}

// Destroy releases the native resources behind the session. It implements types.Destroyable.
//
// It exists to drop the error that the ONNX binding's own Destroy returns, which types.Destroyable does not have: a
// model that fails to free is unreachable either way, and there is nothing a caller could do about it. Without this
// method the promoted one would be the wrong shape and no model would satisfy the interface.
func (s *Session) Destroy() {
	if s == nil || s.DynamicAdvancedSession == nil {
		return
	}

	if err := s.DynamicAdvancedSession.Destroy(); err != nil {
		internal.Log().Warn("failed to destroy ONNX session", "err", err)
	}
}

// Sessions is the set of sessions behind one model, for the upscalers that hold one per scale factor. It reports and
// releases them as a unit, so those models embed it exactly the way the single-session models embed a *Session.
type Sessions []*Session

// ResidentBytes sums the on-disk size of every session in the set. It implements types.Measurable.
func (s Sessions) ResidentBytes() int64 {
	var total int64
	for _, session := range s {
		total += session.ResidentBytes()
	}

	return total
}

// Destroy releases every session in the set. It implements types.Destroyable.
func (s Sessions) Destroy() {
	for _, session := range s {
		session.Destroy()
	}
}

// CreateSession builds an ONNX session for the given model file and execution provider.
//
// An optional EPProfile carries the per-model provider tuning; passing none applies the provider defaults, which is
// what every fixed-shape model wants. Only the first profile is used - the parameter is variadic so that adding
// per-model tuning did not have to touch the call sites that need none.
//
// Every failure it returns - the provider options, the session options and the model load alike - is marked with
// internal.ErrCreateSession, so callers can tell "this session couldn't be built" apart from the failures around it,
// such as a model that couldn't be downloaded. That's what lets AcquireModel decide a retry on the CPU is worth
// attempting.
func CreateSession(
	modelFile string,
	inputs, outputs []string,
	ep types.ExecutionProvider,
	profile ...EPProfile,
) (*Session, error) {
	var p EPProfile
	if len(profile) > 0 {
		p = profile[0]
	}

	session, err := createSession(modelFile, inputs, outputs, ep, p)
	if err != nil {
		return nil, errors.Mark(err, internal.ErrCreateSession)
	}

	return session, nil
}

// SessionSpec describes one graph to load: the model ID it is downloaded and opened under, and the tensor names it
// takes and returns.
type SessionSpec struct {
	ModelId string
	Inputs  []string
	Outputs []string
}

// ModelSpec is the spec for a conventionally-exported graph - one tensor in named "input", one out named "output" -
// which is every fixed-shape model this codebase ships.
func ModelSpec(modelId string) SessionSpec {
	return SessionSpec{ModelId: modelId, Inputs: []string{"input"}, Outputs: []string{"output"}}
}

// LoadSessions downloads and opens every graph in specs, in the order given, and returns them as one set.
//
// It is the single place that knows how to turn model IDs into open sessions, so the families that load one session,
// one per scale factor, or three stages of a pipeline all share the same behaviour - in particular the guarantee
// below, which is the one worth centralising: a partially-loaded set is destroyed rather than leaked. The diffusion
// upscaler's first graph alone is nearly 7 GB, so returning an error while still holding it would strand memory
// nothing can reach.
//
// The returned sessions are in the same order as specs, which is what lets a caller needing named roles bind them.
//
// The specs are installed one at a time. Within a single model the files already download concurrently - a graph and
// its external-data blob are one dependency with two sources, which is where the several gigabytes actually are - but
// two deps.Install calls sharing one onProgress would report from two goroutines into a callback written for one, and
// each keeps its own 0-100% aggregate, so their percentages would interleave rather than combine. Overlapping them
// needs a progress aggregator spanning dependencies; until there is one, this stays serial.
func LoadSessions(
	ctx context.Context,
	specs []SessionSpec,
	ep types.ExecutionProvider,
	profile EPProfile,
	onProgress types.DownloadProgress,
) (_ Sessions, retErr error) {
	sessions := make(Sessions, 0, len(specs))

	defer func() {
		if retErr != nil {
			sessions.Destroy()
		}
	}()

	for _, spec := range specs {
		if err := deps.Install(ctx, deps.ModelDependency(spec.ModelId), onProgress); err != nil {
			return nil, errors.Wrapf(err, "failed to prepare the %s model", spec.ModelId)
		}

		internal.Log().Debug("loading model session", "model_id", spec.ModelId)

		session, err := CreateSession(spec.ModelId+".onnx", spec.Inputs, spec.Outputs, ep, profile)
		if err != nil {
			return nil, errors.Wrapf(err, "failed to create the %s session", spec.ModelId)
		}

		internal.Log().Debug("model session ready", "model_id", spec.ModelId)

		sessions = append(sessions, session)
	}

	return sessions, nil
}

// LoadSingleSession downloads and opens the one session behind a model whose ID is `<prefix>_<variant>_<precision>`,
// e.g. `dn_stockholm_fp16`. It covers the fixed-shape families that have no scale matrix - denoise and sharpen.
func LoadSingleSession(
	ctx context.Context,
	prefix, variant string,
	precision types.Precision,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*Session, error) {
	modelId := fmt.Sprintf("%s_%s_%s", prefix, variant, precision)

	sessions, err := LoadSessions(ctx, []SessionSpec{ModelSpec(modelId)}, ep, EPProfile{}, onProgress)
	if err != nil {
		return nil, err
	}

	return sessions[0], nil
}

// FormatModelName builds the display name a model family shows in the UI, e.g. "Denoise (FP16)".
func FormatModelName(label string, precision types.Precision) string {
	return fmt.Sprintf("%s (%s)", label, cases.Upper(language.English).String(string(precision)))
}

// createSession reports how one session build ended and delegates the work to createSessionInner.
//
// The split exists so the six failure returns below need no logging of their own: each already wraps its cause, and
// one line here carries the two attributes none of them have - which model and which provider. Before this, a session
// that failed to build produced no line at any level, so "it just doesn't work on my machine" had nothing behind it.
//
// The success line matters as much as the failure one: it is what explains a first run that took four minutes while
// TensorRT compiled an engine, versus a later run that hit the cache.
func createSession(
	modelFile string,
	inputs, outputs []string,
	ep types.ExecutionProvider,
	p EPProfile,
) (*Session, error) {
	start := time.Now()

	session, err := createSessionInner(modelFile, inputs, outputs, ep, p)
	if err != nil {
		internal.Log().Warn("failed to create the session", "model_file", modelFile, "ep", ep, "err", err)
		return nil, err
	}

	internal.Log().Info("session created",
		"model_file", modelFile, "ep", ep, "bytes", session.bytes, "duration", time.Since(start))

	return session, nil
}

func createSessionInner(
	modelFile string,
	inputs, outputs []string,
	ep types.ExecutionProvider,
	p EPProfile,
) (*Session, error) {
	modelsPath, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		return nil, errors.Wrap(err, "failed to resolve the models directory")
	}

	// The execution providers compile what they cache - a TensorRT engine, a CoreML MLProgram - from this model's
	// weights, so the cache gets a directory of its own per model. Sharing one directory with the models, as this used
	// to, made the cache impossible to invalidate: TensorRT drops its .engine and .profile files loose beside the
	// models and names them after the graph inside the file rather than the file itself, so nothing said whose cache
	// was whose. With a directory per model, replacing a model is enough to drop exactly its cache.
	//
	// The directory comes from EngineCacheFor, which is also what deps.Install clears when it replaces the weights.
	stem := strings.TrimSuffix(modelFile, filepath.Ext(modelFile))
	cachePath, err := fs.MkUserConfigDir(internal.AppName, strings.Split(internal.EngineCacheFor(stem), "/")...)
	if err != nil {
		return nil, errors.Wrap(err, "failed to resolve the engine cache directory")
	}

	options, err := createOptions(currentPlatform, cachePath, ep, p)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create session options")
	}
	defer options.Destroy()

	// The session-level settings - the graph optimization level, the execution mode, the memory planner - are all
	// applied by applyProfile, since each of them is a per-model property.
	if err = applyProfile(options, p); err != nil {
		return nil, err
	}

	modelPath := filepath.Join(modelsPath, modelFile)
	session, err := ort.NewDynamicAdvancedSession(modelPath, inputs, outputs, options)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create session")
	}

	return &Session{DynamicAdvancedSession: session, bytes: modelFileBytes(modelPath)}, nil
}

// modelFileBytes reports the total size of the files backing the model at modelPath: the graph itself plus any
// external-data blob stored beside it.
//
// ONNX keeps the weights of a model larger than the 2 GB protobuf limit in a separate file, named by a `location`
// field inside the graph. Reading that field would mean parsing the protobuf, so this probes the three naming
// conventions in use instead (`<model>.onnx.data`, `<model>.onnx_data`, `<model>.data`). That matters by three orders
// of magnitude for the diffusion upscaler: `up_osaka_fp16.onnx` is 7.7 MB and `up_osaka_fp16.onnx.data` is 6.8 GB, so
// charging the budget for the graph alone would make a 7 GB model look free.
//
// Probing exact names rather than globbing the directory matters for two reasons. The models directory doubles as the
// TensorRT engine cache and the CoreML model cache, so a glob's cost grows with every compiled kernel - and this runs
// on every session build, once per scale for the upscalers and twice on the CPU-fallback path. It also keeps a model
// that merely shares a stem out of the total by construction: `up_osaka_vae_decoder_fp16.onnx` is its own model, not
// part of `up_osaka_fp16`.
//
// A file that can't be stat-ed contributes 0: an under-count degrades the budget, whereas failing here would fail the
// session build.
func modelFileBytes(modelPath string) int64 {
	candidates := []string{modelPath, modelPath + ".data", modelPath + "_data"}

	// `<model>.data` drops the `.onnx` extension rather than appending to it, so it is derived from the stem - and is
	// only a distinct candidate when there is an extension to drop. Without one the stem is the whole path, making it
	// a duplicate of `<model>.data` above, which would charge that blob twice.
	if ext := filepath.Ext(modelPath); ext != "" {
		candidates = append(candidates, strings.TrimSuffix(modelPath, ext)+".data")
	}

	var total int64

	for _, candidate := range candidates {
		info, err := os.Stat(candidate)
		if err != nil || info.IsDir() {
			continue
		}

		total += info.Size()
	}

	return total
}
