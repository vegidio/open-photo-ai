package utils

import (
	"fmt"
	"maps"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"sync"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// EPProfile is the per-model tuning applied on top of an execution provider's defaults.
//
// It exists because the right provider settings are a property of the model, not of the machine. CoreML's
// RequireStaticInputShapes is the clearest case: it is correct for the fixed-shape tile models, and wrong for a model
// with dynamic spatial axes, where CoreML silently pushes the varying subgraphs back to the CPU partition and reports
// nothing - the run is simply slow, with no error to explain it.
//
// The reverse mistake is worth knowing too, because it does not present as a tuning problem at all. Setting
// DynamicShapes on a graph that is in fact fixed-shape turns RequireStaticInputShapes off, and CoreML will then try to
// convert nodes it would otherwise have declined - which for Osaka's VAE meant failing session creation outright with
// "axis 4 is not in valid range [-4,3]". That was read as a CoreML bug and cost the model its whole provider for
// several releases. A setting here can manufacture the failure it appears to be documenting.
//
// The zero value reproduces the behaviour that shipped before profiles existed, which is what lets every existing
// call site keep passing no profile at all.
//
// Not every field is driven by a model yet: today Osaka sets DisableMemPattern, DisableOptimizers, ExecutionMode,
// CudaPreferNHWC, TrtOptions and CoreMLComputeUnits, Athens sets CoreMLComputeUnits and ExecutionMode and - for its
// fp16 export only - CudaPreferNHWC, Santorini sets CoreMLSpecialization and ExecutionMode, Tokyo sets
// CoreMLComputeUnits and ExecutionMode, New York sets CudaPreferNHWC and ExecutionMode, Paris sets CoreMLComputeUnits
// and ExecutionMode for its fp16 export, and Kyoto, Saitama and Lyon each set CoreMLComputeUnits for their fp16 export
// alone. ExcludeEPs has no setter at all any more - Osaka was its last caller, and the TensorRT exclusion it used to
// hold is now a measured 2.5x end-to-end win instead. CudaOptions has none either: it was added alongside a sweep of
// the CUDA provider's options against Osaka, which found every one of them already at its best value in the defaults
// below - the escape hatch is there so the next graph that disagrees does not have to add a typed field for one
// setting. The rest are
// reserved for per-model TensorRT and precision tuning that is already planned - they are deliberately kept rather
// than trimmed to what has a caller today, so treat "no setter" here as "not wired up yet", not as dead code.
type EPProfile struct {
	// DynamicShapes declares that the model's input shapes vary between runs, so providers must not be configured
	// for a fixed shape.
	DynamicShapes bool

	// Fp16 lets TensorRT run in half precision. It is opt-in: a graph already exported in fp16 gains nothing, and
	// forcing it on a model that was not validated for it silently changes the output.
	Fp16 bool

	// TrtWorkspaceBytes overrides TensorRT's workspace ceiling. Zero keeps the default.
	TrtWorkspaceBytes int64

	// TrtShapes carries the trt_profile_{min,opt,max}_shapes strings for a dynamic-shape model. TensorRT needs
	// explicit optimization profiles for those, and the right ranges are model-specific.
	TrtShapes map[string]string

	// TrtOptions overlays raw TensorRT provider options onto the defaults in tensorRTOptions, for the settings that
	// have no typed field here. It is the TensorRT counterpart of Extra, and it exists for the same reason: the
	// provider has around forty options, most of which only one graph in this codebase would ever want.
	//
	// It is applied last, so it can also override a default - which is the point. Osaka uses it to drop
	// trt_builder_optimization_level from the 5 every other model gets to TensorRT's own default of 3, which on a
	// 12,940-node graph is 87 seconds of engine build for no runtime difference.
	//
	// An unrecognised key is not ignored: ONNX Runtime rejects the whole option update, the provider declines to
	// attach, and the graph quietly runs on the next provider in the chain. So a typo here costs the GPU, not a
	// setting - check the session-created log line names TensorRT after changing anything in it.
	//
	// Measuring what belongs here needs two precautions that a CUDA sweep does not, and both produce a clean table of
	// wrong numbers rather than an obvious failure. The first is the engine cache: TensorRT's cache file name carries
	// only the graph hash and the precision flags, so two configurations pointed at one cache directory hand each
	// other the first one's engine and every option reads as a no-op. Give each configuration a directory of its own.
	//
	// The second is that the builder is not deterministic. Six sessions built from an identical configuration, one
	// cache directory each, spread from -2.5% to +1.2% on tokyo's fp16 export and did not all produce the same output
	// - so a single build per configuration samples the builder rather than measuring the option, and nothing below
	// roughly +/-3% survives a second build. Tokyo's profile has the worked example, including the no-op setting that
	// measured -1.6% in one precision and +6.2% in the other.
	TrtOptions map[string]string

	// DisableMemPattern turns off ONNX Runtime's static memory planner. The planner assumes shapes repeat between
	// runs; when they vary, it over-allocates and never returns what it reserved.
	DisableMemPattern bool

	// ExcludeEPs names providers this model must not run on. It is advisory: the next provider in the platform's
	// chain is used instead, never a silent drop to the CPU.
	ExcludeEPs []types.ExecutionProvider

	// CudaPreferNHWC runs the CUDA provider's convolutions in NHWC rather than ONNX Runtime's default NCHW.
	//
	// It is per-model, and can differ per-precision within a model, because it is not an optimisation the runtime
	// can pick on its own merits. The intuition is that NHWC is the layout cuDNN's fp16 tensor-core kernels want,
	// so an fp16 graph stops paying for the transposes around every convolution while an fp32 one has no
	// tensor-core path to reach and only pays the layout conversion: athens measures -4% end to end in fp16 and
	// +8% in fp32 from this one flag, and sets it for fp16 alone.
	//
	// Do not turn that into a rule. New York wants NHWC in BOTH precisions - -35% in fp16 and -17.5% in fp32 on
	// the graph alone - because what actually decides it is which layouts cuDNN has kernels for on the shapes the
	// graph asks for, and a RetinaFace backbone at a fixed 640x640 gets a different answer from a CodeFormer at
	// 512x512. Two models here, opposite answers in fp32: measure it per graph, per precision.
	//
	// It is also not universally safe. ORT's layout transform mishandles a Conv whose weights are computed rather
	// than an initializer - a StyleGAN-style modulated convolution - and the graph then fails at Run with a channel
	// mismatch rather than at session build. That is the strongest reason this is opt-in per model: turning it on
	// globally would break models nobody re-measured.
	CudaPreferNHWC bool

	// CudaOptions overlays raw CUDA provider options onto the defaults in cudaOptions, for the settings that have no
	// typed field here. It is the CUDA counterpart of TrtOptions and exists for the same reason: the provider has
	// around twenty options, and most of them only one graph in this codebase would ever want.
	//
	// It is applied last, so it can also override a default. The same warning as TrtOptions applies and is worth
	// repeating, because on CUDA it is easier to trip over: an unrecognised key makes ONNX Runtime reject the whole
	// option update, the provider declines to attach, and the graph runs on the CPU instead - a 10x regression that
	// reports itself only as one Warn line. Check that line after changing anything here.
	CudaOptions map[string]string

	// CoreMLComputeUnits selects which of the Mac's engines CoreML may dispatch this model to. The zero value is
	// ALL, which is what every model used before profiles existed.
	CoreMLComputeUnits CoreMLComputeUnits

	// CoreMLSpecialization picks how CoreML compiles the model. The zero value is its default strategy, which is
	// what every model used before profiles existed.
	CoreMLSpecialization CoreMLSpecialization

	// GraphOptimization selects how far ONNX Runtime rewrites the graph before running it. The zero value is the
	// full pipeline, which is what every model used before profiles existed.
	GraphOptimization GraphOptimization

	// ExecutionMode selects whether ONNX Runtime may run independent branches of the graph on separate threads. The
	// zero value is parallel, which is what every model used before profiles existed.
	ExecutionMode ExecutionMode

	// DisableOptimizers names individual graph transformers to switch off while leaving the rest of the pipeline
	// on. It is the scalpel to GraphOptimization's hammer: a model that one transformer miscompiles keeps every
	// other fusion instead of giving them all up.
	DisableOptimizers []string

	// Extra carries raw session config entries, for the settings that have no typed field here.
	Extra map[string]string
}

// ResolveProfile returns the profile a variant declares for the precision being loaded, or the zero value when it
// declares none.
//
// Every model family spells "nobody has measured this graph" the same way - a nil Profile func on the variant - so
// the unwrap lives here next to EPProfile rather than being re-written in each family's variant.go.
//
// The precision is passed in because tuning does not survive the precision change: the same graph exported in fp16
// and in fp32 wants opposite answers from CudaPreferNHWC, and a profile that could not see the precision would have
// to pick the one that pessimises the other export.
func ResolveProfile(fn func(types.Precision) EPProfile, precision types.Precision) EPProfile {
	if fn == nil {
		return EPProfile{}
	}

	return fn(precision)
}

// Fp16Only applies a profile to a model's fp16 export alone, leaving fp32 on the provider defaults.
//
// It is the shape four variants share, and the fp32 half is the load-bearing one rather than a formality. Every one
// of them tunes CoreMLComputeUnits, and at fp32 that setting is either a no-op or a large regression: an MLProgram at
// fp32 cannot reach the Neural Engine at all, so CPUAndGPU there only ever restates the default, while pinning the
// Neural Engine costs kyoto and saitama 9x to 19x. Writing the guard once means a new variant cannot ship the fp16
// answer applied to both precisions.
func Fp16Only(p EPProfile) func(types.Precision) EPProfile {
	return func(precision types.Precision) EPProfile {
		if precision == types.PrecisionFp16 {
			return p
		}

		return EPProfile{}
	}
}

// CoreMLComputeUnits is the set of engines CoreML may dispatch a model to.
//
// It is a per-model property because the right answer follows the graph's op mix, not the machine. The Neural Engine
// is fp16-only and is built for dense convolution and matmul; a graph that is heavy in normalization, reshape and
// transpose spends more time crossing on and off it than it saves, and CoreML's planner does not work that out on its
// own - it takes the Neural Engine whenever ALL permits it.
type CoreMLComputeUnits int

const (
	// CoreMLComputeUnitsAll lets CoreML choose freely between the CPU, the GPU and the Neural Engine. It is the
	// right default: most of these graphs are convolutional, which is what the Neural Engine is good at.
	CoreMLComputeUnitsAll CoreMLComputeUnits = iota

	// CoreMLComputeUnitsCPUAndGPU keeps a model off the Neural Engine. It is for the graphs the Neural Engine
	// handles badly, where ALL costs real time in transitions.
	CoreMLComputeUnitsCPUAndGPU

	// CoreMLComputeUnitsCPUAndNeuralEngine keeps a model off the GPU, leaving it for other work.
	CoreMLComputeUnitsCPUAndNeuralEngine

	// CoreMLComputeUnitsCPUOnly runs the CoreML partition on the CPU. It is a diagnostic: it isolates whether a
	// wrong result came from the GPU's or the Neural Engine's reduced precision.
	CoreMLComputeUnitsCPUOnly
)

func (c CoreMLComputeUnits) value() string {
	switch c {
	case CoreMLComputeUnitsCPUAndGPU:
		return "CPUAndGPU"
	case CoreMLComputeUnitsCPUAndNeuralEngine:
		return "CPUAndNeuralEngine"
	case CoreMLComputeUnitsCPUOnly:
		return "CPUOnly"
	default:
		return "ALL"
	}
}

// CoreMLSpecialization is the strategy CoreML uses when it compiles a model for a device.
//
// It is a per-model property because it is a trade: the default strategy compiles once and produces a plan that is
// good across input sizes, while FastPrediction spends longer specialising for the shapes it was given. That is only
// worth paying for on a fixed-shape graph that stays resident, which is exactly the shape of these tile models - and
// on a graph whose partition CoreML was already handling well it buys nothing.
//
// Being fixed-shape and resident is a precondition rather than a prediction, so measure it rather than assuming.
// Lyon is both - one fused CoreML node at a fixed 1024x1024 - and FastPrediction lands within 0.3% of the default
// there in both precisions and both build orders, with bit-identical output.
type CoreMLSpecialization int

const (
	// CoreMLSpecializationDefault leaves CoreML to its own default strategy.
	CoreMLSpecializationDefault CoreMLSpecialization = iota

	// CoreMLSpecializationFastPrediction trades compile time for prediction latency. It needs macOS 15 or newer;
	// older systems ignore it rather than failing, so setting it is safe on any Mac.
	CoreMLSpecializationFastPrediction
)

func (c CoreMLSpecialization) value() string {
	switch c {
	case CoreMLSpecializationFastPrediction:
		return "FastPrediction"
	default:
		return "Default"
	}
}

// ExecutionMode is whether ONNX Runtime may run independent branches of a graph on separate threads.
//
// It is a per-model property because the win it is there for is a property of the graph's shape: the inter-op thread
// pool only pays for itself when the graph has wide, genuinely independent branches, and it costs a thread handoff at
// every node either way. These graphs are backbones - a U-Net, a StyleGAN decoder - which are close to linear, so on
// them the handoff is all there is, and it is charged per Run rather than once.
//
// The cost is not confined to the CPU provider. Under a GPU provider every node is enqueued onto one stream, so
// inter-op threading buys nothing at all there and the scheduling still has to happen: santorini measures -4% at
// fp32 and -8% at fp16 through the CUDA provider from switching this to sequential, with a bit-identical output.
//
// What it is worth scales with the node count, since the handoff is charged per node: tokyo, at 2682 nodes against
// santorini's few hundred, measures -27% at fp32 and -18% at fp16 the same way. That is the reason to try this on
// any new graph before reaching for a provider option - it is usually the larger number, and it costs nothing.
//
// It is a session setting rather than a per-provider one, so a model that sets it sets it everywhere, and the
// question of whether that is safe has been settled by measurement on every provider this codebase ships. On
// TensorRT it is a tie, because that provider fuses the graph into one node and leaves the inter-op pool nothing to
// schedule. On CoreML it splits cleanly by precision - measured out of tree on an M2 Max (macOS 26.6, ONNX Runtime
// 1.29), sequential against parallel:
//
//	              fp32        fp16
//	tokyo         +0.2%       -3.5%
//	santorini     +0.5%       -4.8%
//	newyork       +0.5%       -6.5%
//	athens        +0.3%       +0.2%
//
// Every fp16 graph but athens wins, every fp32 graph is a tie, nothing loses by more than half a percent, and the
// output is bit-identical in all of them. So there is no provider for which this needs to be made conditional, and
// the field stays a plain per-model setting.
//
// Two cautions for anyone re-measuring it on a Mac, because both can manufacture an effect the size of the one being
// looked for, and both did before the harness was corrected.
//
// The first is position bias inside a sweep. Interleaving at round granularity is not enough: if the two sessions
// always run in the same order within a round, whatever the machine does over a round is charged entirely to
// whichever runs second, which measured about 2% - and reversing the order reversed the sign, which is what gave it
// away. The harness now alternates the order every round and wants an even round count so that cancels.
//
// The second is thermal drift, which on this hardware is larger than anything being measured. The same tokyo fp16
// configuration measured 1619ms, then 4780ms on a loaded machine, then 2319ms after a three-minute pause; the
// median-to-minimum spread within a sweep is the tell that it is happening. Compare only rows from the same sweep,
// never an absolute number against one recorded earlier.
type ExecutionMode int

const (
	// ExecutionModeParallel lets ONNX Runtime run independent branches on separate threads. It is the mode every
	// model used before profiles existed, so it stays the zero value.
	ExecutionModeParallel ExecutionMode = iota

	// ExecutionModeSequential runs one node at a time on the calling thread. It is for the graphs that have no
	// branch wide enough to pay for the inter-op pool, which is most of them.
	ExecutionModeSequential
)

func (e ExecutionMode) mode() ort.ExecutionMode {
	if e == ExecutionModeSequential {
		return ort.ExecutionModeSequential
	}

	return ort.ExecutionModeParallel
}

// GraphOptimization is how far ONNX Runtime is allowed to rewrite a graph.
type GraphOptimization int

const (
	// GraphOptimizationDefault applies every optimization, which is the right choice for a graph the runtime
	// handles correctly.
	GraphOptimizationDefault GraphOptimization = iota
	GraphOptimizationExtended
	GraphOptimizationBasic

	// GraphOptimizationDisabled runs the graph as exported. It is the last resort for a model the optimizer cannot
	// process, and it costs real speed on a transformer, where the fusions are worth a lot.
	GraphOptimizationDisabled
)

func (g GraphOptimization) level() ort.GraphOptimizationLevel {
	switch g {
	case GraphOptimizationExtended:
		return ort.GraphOptimizationLevelEnableExtended
	case GraphOptimizationBasic:
		return ort.GraphOptimizationLevelEnableBasic
	case GraphOptimizationDisabled:
		return ort.GraphOptimizationLevelDisableAll
	default:
		return ort.GraphOptimizationLevelEnableAll
	}
}

// disableOptimizersKey is the ONNX Runtime session config entry naming transformers to skip. The entries are
// semicolon-separated, and an unrecognised name is ignored rather than reported - so a typo here is silent, and the
// only way to know the setting took effect is that the model loads.
const disableOptimizersKey = "optimization.disable_specified_optimizers"

func (p EPProfile) excludes(ep types.ExecutionProvider) bool {
	return slices.Contains(p.ExcludeEPs, ep)
}

// trtBuildMu serializes session builds that may attach TensorRT, so the one shared timing cache has a single writer.
// See the comment at its use in createSessionInner.
var trtBuildMu sync.Mutex

// usesTensorRT reports whether a session built for this request could attach the TensorRT provider.
//
// It re-runs resolveProviders rather than reading what createOptions worked out, which is a duplicated lookup over a
// table of at most four entries and no side effects. The alternative was to have createOptions report which providers
// it attached, which would put a return value on it that only the lock cares about - and the lock has to be taken
// before the session is built, not after the options are.
func usesTensorRT(goos string, ep types.ExecutionProvider, p EPProfile) bool {
	providers, err := resolveProviders(goos, ep, p)
	if err != nil {
		return false
	}

	return slices.Contains(providers, types.ExecutionProviderTensorRT)
}

// dropTimingCache removes the shared TensorRT timing cache after a session build failed with TensorRT in the chain.
//
// Every failure here is logged and swallowed: this runs on a path that is already returning an error, and a cache
// file that could not be removed is not worth replacing that error with.
func dropTimingCache(timingPath, goos string, ep types.ExecutionProvider, p EPProfile) {
	if !usesTensorRT(goos, ep, p) {
		return
	}

	entries, err := os.ReadDir(timingPath)
	if err != nil {
		internal.Log().Warn("failed to read the timing cache directory", "path", timingPath, "err", err)
		return
	}

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		if err = os.Remove(filepath.Join(timingPath, entry.Name())); err != nil {
			internal.Log().Warn("failed to drop the timing cache", "file", entry.Name(), "err", err)
			continue
		}

		internal.Log().Info("dropped the TensorRT timing cache after a failed session build", "file", entry.Name())
	}
}

// cachePaths are the directories the execution providers are pointed at.
//
// It is a struct rather than two string parameters because only one of the two is per-model, and a pair of bare
// strings at every call site is how they end up swapped: engine is this model's own directory, while timing is shared
// by every model in the installation. Handing TensorRT the model directory as its timing cache would silently give
// back the per-model behaviour that internal.TimingCacheDir exists to avoid, and nothing would report it - the builds
// would simply stay slow.
type cachePaths struct {
	// engine is this model's own directory, holding what a provider compiled from it: a TensorRT engine, a CoreML
	// MLProgram. See internal.EngineCacheFor.
	engine string

	// timing is the installation-wide TensorRT timing cache directory. See internal.TimingCacheDir.
	timing string
}

// providerAppender configures one execution provider onto a set of session options.
type providerAppender func(paths cachePaths, options *ort.SessionOptions, p EPProfile) error

var providerAppenders = map[types.ExecutionProvider]providerAppender{
	types.ExecutionProviderTensorRT: appendTensorRT,
	types.ExecutionProviderCUDA:     appendCuda,
	types.ExecutionProviderDirectML: appendDirectML,
	types.ExecutionProviderCoreML:   appendCoreML,
	types.ExecutionProviderOpenVINO: appendOpenVINO,
}

// autoChain is the order ExecutionProviderAuto tries on each platform, and doubles as the set of providers that
// platform supports at all: a request for one that is not listed is an error rather than a silent no-op.
//
// CPU is deliberately absent. It needs no appender, and it is what ONNX Runtime falls back to on its own once the
// providers above it decline a node.
var autoChain = map[string][]types.ExecutionProvider{
	"windows": {
		types.ExecutionProviderTensorRT,
		types.ExecutionProviderCUDA,
		types.ExecutionProviderDirectML,
		types.ExecutionProviderOpenVINO,
	},
	"linux": {
		types.ExecutionProviderTensorRT,
		types.ExecutionProviderCUDA,
		types.ExecutionProviderOpenVINO,
	},
	"darwin": {
		types.ExecutionProviderCoreML,
		types.ExecutionProviderOpenVINO,
	},
}

// resolveProviders returns the providers to append, in order, for a request on the given platform.
//
// An explicit request normally yields just that provider. When the profile excludes it, the rest of the platform's
// chain is used instead: a model that cannot run on TensorRT should fall to CUDA, not to the CPU, which would turn a
// provider mismatch into an hours-long run.
func resolveProviders(goos string, ep types.ExecutionProvider, p EPProfile) ([]types.ExecutionProvider, error) {
	chain, ok := autoChain[goos]
	if !ok {
		return nil, errors.Errorf("unsupported platform: %s", goos)
	}

	if ep == types.ExecutionProviderAuto {
		return filterExcluded(chain, p), nil
	}

	if !slices.Contains(chain, ep) {
		return nil, errors.Errorf("unsupported execution provider: %s", ep)
	}

	if !p.excludes(ep) {
		return []types.ExecutionProvider{ep}, nil
	}

	substitutes := filterExcluded(chain, p)
	internal.Log().Info("execution provider excluded for this model; substituting",
		"requested", ep, "substitutes", substitutes)

	return substitutes, nil
}

func filterExcluded(chain []types.ExecutionProvider, p EPProfile) []types.ExecutionProvider {
	out := make([]types.ExecutionProvider, 0, len(chain))

	for _, ep := range chain {
		if !p.excludes(ep) {
			out = append(out, ep)
		}
	}

	return out
}

// createOptions builds the session options for one model on one execution provider.
func createOptions(goos string, paths cachePaths, ep types.ExecutionProvider, p EPProfile) (*ort.SessionOptions, error) {
	if _, ok := autoChain[goos]; !ok {
		return nil, errors.Errorf("unsupported platform: %s", goos)
	}

	options, err := ort.NewSessionOptions()
	if err != nil {
		return nil, errors.Wrapf(err, "failed to create %s session options", goos)
	}

	// The CPU provider is always present and takes no configuration, so there is nothing to append for it.
	if ep == types.ExecutionProviderCPU {
		return options, nil
	}

	providers, err := resolveProviders(goos, ep, p)
	if err != nil {
		options.Destroy()
		return nil, err
	}

	// A provider that declines to attach is not fatal: the ones after it, and ultimately the CPU, still run the
	// graph. That is the long-standing behaviour and the reason these errors are only logged.
	//
	// Warn rather than Debug, though. This is the line that separates "the GPU is doing the work" from "it quietly
	// fell back to CPU kernels", which is the single most common thing a user reports as "the app got slow" - and at
	// Debug it was invisible in every log anyone would actually send us. It fires at most once per provider per
	// session build, so it costs a handful of lines even on a machine where every provider declines.
	for _, provider := range providers {
		// Looked up rather than called straight out of the map: providers comes from autoChain, which is a separate
		// table. The two agree today, but adding a provider to the chain and forgetting its appender would panic on a
		// nil func here - at session build, far from the edit - instead of declining like any other unusable provider.
		appender, ok := providerAppenders[provider]
		if !ok {
			internal.Log().Warn("no appender is registered for the execution provider; skipping it", "ep", provider)
			continue
		}

		if err = appender(paths, options, p); err != nil {
			internal.Log().Warn("execution provider declined to attach; the graph will run on the next provider "+
				"in the chain", "ep", provider, "requested_ep", ep, "err", err)
		}
	}

	return options, nil
}

// applyProfile applies the session-level settings a profile carries, as opposed to the per-provider ones.
func applyProfile(options *ort.SessionOptions, p EPProfile) error {
	if err := options.SetGraphOptimizationLevel(p.GraphOptimization.level()); err != nil {
		return errors.Wrap(err, "failed to set the graph optimization level")
	}

	if err := options.SetExecutionMode(p.ExecutionMode.mode()); err != nil {
		return errors.Wrap(err, "failed to set the execution mode")
	}

	if len(p.DisableOptimizers) > 0 {
		if err := options.AddSessionConfigEntry(disableOptimizersKey, strings.Join(p.DisableOptimizers, ";")); err != nil {
			return errors.Wrap(err, "failed to disable the specified optimizers")
		}
	}

	if p.DisableMemPattern {
		if err := options.SetMemPattern(false); err != nil {
			return errors.Wrap(err, "failed to disable the memory pattern planner")
		}
	}

	for key, value := range p.Extra {
		if err := options.AddSessionConfigEntry(key, value); err != nil {
			return errors.Wrapf(err, "failed to set session config entry %q", key)
		}
	}

	return nil
}

// region - Provider option maps
//
// Each provider's settings are built by a pure function so the mapping from profile to options can be tested without
// an ONNX Runtime present, which is the only part of this file a unit test can reach.

// CUDA graph capture - trt_cuda_graph_enable on TensorRT, enable_cuda_graph on the CUDA provider - is off in both
// option maps below, and this is why. It is not a "revisit when there is time" setting: it is measured, and both
// providers fail it, in opposite ways.
//
// On TensorRT the capture run is correct and every run after it silently returns an all-zero output. Re-measured
// against athens on 2026-09-02 (RTX 5090, driver 610.88, ONNX Runtime 1.26, both precisions): the first Run matches
// the graph-off result exactly, then 20 of 20 subsequent Runs on that session return zeros, with no error and with
// plausible timings - the replay writes nothing, which is also where the "9% faster" that makes this tempting comes
// from. In the app that surfaces as no faces found from the second detection on, or blank recovered faces, so a
// benchmark that only checks its first run will report a win and ship a broken model.
//
// On the CUDA provider it fails loudly instead: the capture run dies in cudaStreamEndCapture because cuBLAS
// initialises lazily inside the captured stream ("CUBLAS failure 1: the library was not initialized"), so the very
// first inference errors out. ORT also requires every input and output to be bound to device memory for capture,
// which this codebase's host tensors are not.
//
// Anyone revisiting this needs one thing the earlier attempt lacked: verify the SECOND Run on the SAME session
// against a CPU or graph-off result. A single-run comparison cannot see either failure.

func tensorRTOptions(paths cachePaths, p EPProfile) map[string]string {
	workspace := int64(4) << 30
	if p.TrtWorkspaceBytes > 0 {
		workspace = p.TrtWorkspaceBytes
	}

	fp16 := "0"
	if p.Fp16 {
		fp16 = "1"
	}

	// trt_engine_hw_compatible is off, and it was the single largest TensorRT setting in this file while it was on.
	// It builds an engine that runs on any Ampere-or-newer card, which means TensorRT may only pick kernels that
	// exist on all of them - so the newer the card, the more it gives up. Measured on an RTX 5090 (sm_120, driver
	// 610.88, ONNX Runtime 1.26), median of 5, hardware-compatible against architecture-specific:
	//
	//	athens      -23.6%      tokyo       -12.0%      saitama     -9.9%
	//	kyoto        -7.2%      santorini    -5.9%      stockholm   -4.7%
	//	osaka       -48.0%      newyork      too fast to resolve at 4ms
	//
	// Cold start improves with it, too - the compatible engine is the slower one to build as well as to run.
	//
	// Nothing is lost by this, because the portability it buys has no consumer here: the engine cache is built on the
	// user's own machine on first use, never shipped, and the cache file names carry the architecture they were built
	// for (`..._sm80+.engine` against `..._sm120.engine`), so a machine that changes GPU asks for a name that is not
	// there and rebuilds rather than loading something wrong. A model that measures a loss can set it back to "1"
	// through EPProfile.TrtOptions.
	options := map[string]string{
		"device_id":                      "0",
		"trt_max_workspace_size":         fmt.Sprintf("%d", workspace),
		"trt_fp16_enable":                fp16,
		"trt_int8_enable":                "0",
		"trt_engine_hw_compatible":       "0",
		"trt_cuda_graph_enable":          "0",
		"trt_builder_optimization_level": "5",
		"trt_engine_cache_enable":        "1",
		"trt_engine_cache_path":          paths.engine,
		"trt_timing_cache_enable":        "1",
		"trt_timing_cache_path":          paths.timing,
	}

	maps.Copy(options, p.TrtShapes)
	maps.Copy(options, p.TrtOptions)

	return options
}

func cudaOptions(p EPProfile) map[string]string {
	// NCHW is ORT's default and stays the default here, because the models that want NHWC do not agree on when:
	// athens is a win in fp16 and a loss in fp32, newyork is a win in both - see EPProfile.CudaPreferNHWC.
	preferNHWC := "0"
	if p.CudaPreferNHWC {
		preferNHWC = "1"
	}

	options := map[string]string{
		"cudnn_conv_algo_search":       "EXHAUSTIVE",
		"cudnn_conv_use_max_workspace": "1",
		"device_id":                    "0",
		"do_copy_in_default_stream":    "1",
		"enable_cuda_graph":            "0",
		"gpu_mem_limit":                "0",
		"prefer_nhwc":                  preferNHWC,
	}

	maps.Copy(options, p.CudaOptions)

	return options
}

func coreMLOptions(paths cachePaths, p EPProfile) map[string]string {
	// CoreML compiles a fixed-shape MLProgram when it may assume static inputs. For a model whose spatial axes vary
	// per run that assumption does not hold, and leaving it on makes CoreML decline the varying subgraphs silently.
	staticShapes := "1"
	if p.DynamicShapes {
		staticShapes = "0"
	}

	// ModelFormat is pinned rather than exposed on EPProfile, and that is a correctness decision rather than an
	// oversight. NeuralNetwork is the older format and has no typed execution, so CoreML may put an fp32 graph on
	// the Neural Engine and run it in half precision - and it does: Saitama's fp32 graph measures -38.9% under it,
	// landing to four significant figures on the time AND the accuracy of that model's fp16 export. A model author
	// sweeping provider options would read that as the largest win available and ship a precision downgrade the
	// user never asked for. The honest way to take it is to select the fp16 model.
	return map[string]string{
		"EnableOnSubgraphs":        "0",
		"MLComputeUnits":           p.CoreMLComputeUnits.value(),
		"ModelCacheDirectory":      paths.engine,
		"ModelFormat":              "MLProgram",
		"RequireStaticInputShapes": staticShapes,
		"SpecializationStrategy":   p.CoreMLSpecialization.value(),
	}
}

// endregion

// region - Provider appenders

func appendTensorRT(paths cachePaths, options *ort.SessionOptions, p EPProfile) error {
	trtOptions, err := ort.NewTensorRTProviderOptions()
	if err != nil {
		return errors.Wrap(err, "failed to create TensorRT EP options")
	}
	defer trtOptions.Destroy()

	if err = trtOptions.Update(tensorRTOptions(paths, p)); err != nil {
		return errors.Wrap(err, "failed to apply the TensorRT EP options")
	}

	return options.AppendExecutionProviderTensorRT(trtOptions)
}

func appendCuda(_ cachePaths, options *ort.SessionOptions, p EPProfile) error {
	cudaOpts, err := ort.NewCUDAProviderOptions()
	if err != nil {
		return errors.Wrap(err, "failed to create CUDA EP options")
	}
	defer cudaOpts.Destroy()

	if err = cudaOpts.Update(cudaOptions(p)); err != nil {
		return errors.Wrap(err, "failed to apply the CUDA EP options")
	}

	return options.AppendExecutionProviderCUDA(cudaOpts)
}

func appendDirectML(_ cachePaths, options *ort.SessionOptions, _ EPProfile) error {
	return options.AppendExecutionProviderDirectML(0)
}

func appendCoreML(paths cachePaths, options *ort.SessionOptions, p EPProfile) error {
	return options.AppendExecutionProviderCoreMLV2(coreMLOptions(paths, p))
}

func appendOpenVINO(_ cachePaths, _ *ort.SessionOptions, _ EPProfile) error {
	// Reporting the no-op rather than returning nil. While this is stubbed out, returning nil told the caller the
	// provider had attached, so a machine resolving to OpenVINO ran entirely on CPU kernels with nothing anywhere
	// saying why. The error is not fatal - the caller logs it and moves down the chain, which is the correct
	// behaviour - it just makes the downgrade visible.
	return errors.New("the OpenVINO provider is disabled in this build")

	// TODO: Temporarily disable OpenVINO EP
	//return options.AppendExecutionProviderOpenVINO(map[string]string{
	//	"device_type":    "AUTO",
	//	"precision":      "FP32",
	//	"num_of_threads": fmt.Sprintf("%d", runtime.NumCPU()),
	//	"num_streams":    "2",
	//	"cache_dir":      cachePath,
	//})
}

// endregion

// currentPlatform is a variable, so tests can exercise the per-platform chains without cross-compiling.
var currentPlatform = runtime.GOOS
