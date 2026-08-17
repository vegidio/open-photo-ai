package main

import (
	"fmt"
	"image"
	"os"
	"strings"
	"time"

	"charm.land/lipgloss/v2"
	"charm.land/lipgloss/v2/table"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
)

// groups is the order `list` prints, with the note that says which flag matters for each. It is a separate table from
// the catalog because it carries presentation only; a model added to the catalog shows up here automatically as long
// as its type appears below.
var groups = []struct {
	kind  types.ModelType
	label string
	note  string
}{
	{types.ModelTypeUpscale, "Upscale", "scale set with --scale"},
	{types.ModelTypeDenoise, "Denoise", "blend set with --intensity"},
	{types.ModelTypeSharpen, "Sharpen", "blend set with --intensity"},
	{types.ModelTypeLightAdjustment, "Light Adjustment", "blend set with --intensity"},
	{types.ModelTypeColorBalance, "Color Balance", "blend set with --intensity"},
	{types.ModelTypeDetection, "Detection", "returns data, not an image"},
	{types.ModelTypeFaceRecovery, "Face Recovery", "an untimed detection pass runs first"},
}

// outf and outln write the report to stdout through lipgloss.Writer rather than with fmt directly. The writer knows
// the terminal's colour profile and strips the escape codes when there isn't one, so `perftest > results.txt` gets a
// plain-text table instead of one peppered with ANSI sequences.
func outf(format string, args ...any) {
	fmt.Fprintf(lipgloss.Writer, format, args...)
}

func outln(args ...any) {
	fmt.Fprintln(lipgloss.Writer, args...)
}

// printCatalog renders the `list` output. It runs before any runtime initialization, so it is instant.
func printCatalog() {
	st := newStyles()

	for _, g := range groups {
		names := make([]string, 0, 4)
		for _, e := range catalog {
			if e.kind == g.kind {
				names = append(names, e.name)
			}
		}

		if len(names) == 0 {
			continue
		}

		outf("%s %s\n", st.title.Render(fmt.Sprintf("%-21s", g.label+" ("+string(g.kind)+")")),
			st.dim.Render(g.note))

		for _, name := range names {
			outf("  %s\n", name)
		}

		outln()
	}
}

// printHeader describes the run before it starts, so a table pasted into an issue carries the conditions that produced
// it.
func printHeader(cfg config, input *types.ImageData, faces int, selection []entry) {
	st := newStyles()
	bounds := input.Pixels.Bounds()

	outln(st.title.Render("perftest — open-photo-ai inference benchmark"))
	outln()

	line := func(label, value string) {
		outf("  %s  %s\n", st.label.Render(fmt.Sprintf("%-10s", label)), value)
	}

	megapixels := float64(bounds.Dx()*bounds.Dy()) / 1_000_000
	line("input", fmt.Sprintf("embedded sample, %dx%d (%.2f MPix, %s)",
		bounds.Dx(), bounds.Dy(), megapixels, facesNote(faces)))

	line("provider", fmt.Sprintf("%s%s", cfg.provider, st.dim.Render("   probes: "+probeSummary())))
	line("precision", string(cfg.precision))

	warmupNote := "cold start measured after the warm-up, so it excludes model downloads"
	if cfg.warmup == 0 {
		warmupNote = st.warn.Render("no warm-up: the cold start may include a model download")
	}
	line("runs", fmt.Sprintf("%d timed, %d warm-up  %s", cfg.runs, cfg.warmup, st.dim.Render("("+warmupNote+")")))

	line("params", fmt.Sprintf("scale %gx %s   intensity %g %s",
		cfg.scale, st.dim.Render("(upscale models)"),
		cfg.intensity, st.dim.Render("(denoise/sharpen/light/color models)")))

	cacheNote := "off — timings are inference, not PNG encoding"
	if cfg.cache {
		cacheNote = st.warn.Render("on — timings include the library's PNG cache write")
	}
	line("cache", cacheNote)

	line("models", fmt.Sprintf("%d selected", len(selection)))
	outln()
}

// probeSummary reports what the library can tell us about the machine. DirectML and OpenVINO have no probe, so they
// are simply absent rather than reported as unsupported. These are informational: a run is never blocked on them,
// because Auto and the CPU fallback already handle reality.
func probeSummary() string {
	yesNo := func(ok bool) string {
		if ok {
			return "yes"
		}

		return "no"
	}

	return fmt.Sprintf("CUDA %s | TensorRT %s | CoreML %s",
		yesNo(utils.IsCudaSupported()), yesNo(utils.IsTensorRtSupported()), yesNo(utils.IsCoreMLSupported()))
}

// printSummary renders the results table plus the failure and warning lists.
func printSummary(results []result, cfg config, input *types.ImageData, verbose bool) {
	if len(results) == 0 {
		return
	}

	st := newStyles()
	bounds := input.Pixels.Bounds()

	rows := make([][]string, 0, len(results))
	for _, r := range results {
		rows = append(rows, summaryRow(r, bounds))
	}

	t := table.New().
		Border(lipgloss.NormalBorder()).
		BorderStyle(st.border).
		Headers("MODEL", "TYPE", "COLD", "MIN", "MEDIAN", "MEAN", "MAX", "STDDEV", "MPIX/S", "OUTPUT", "NOTE").
		Rows(rows...).
		StyleFunc(func(row, _ int) lipgloss.Style {
			if row == table.HeaderRow {
				return st.header
			}

			return st.cell
		})

	outln(t)

	if verbose {
		printRawTimings(results, st)
	}

	printFailures(results, st)
	printWarnings(results, cfg, st)
	printFooter(cfg, st)
}

func summaryRow(r result, bounds image.Rectangle) []string {
	if !r.ok() {
		note := "FAILED"
		if r.interrupted() {
			note = "INTERRUPTED"
		}

		return []string{r.entry.name, string(r.entry.kind), "-", "-", "-", "-", "-", "-", "-", "-", note}
	}

	output := "-"
	if r.outcome.width > 0 && r.outcome.height > 0 {
		output = fmt.Sprintf("%dx%d", r.outcome.width, r.outcome.height)
	}

	return []string{
		r.entry.name,
		string(r.entry.kind),
		formatDuration(r.cold),
		formatDuration(r.stats.min),
		formatDuration(r.stats.median),
		formatDuration(r.stats.mean),
		formatDuration(r.stats.max),
		formatDuration(r.stats.stdDev),
		fmt.Sprintf("%.2f", megapixelsPerSecond(bounds, r.stats.median)),
		output,
		r.outcome.note,
	}
}

// printRawTimings prints every timed run in run order, which is how you spot a thermal ramp that the median hides.
func printRawTimings(results []result, st styles) {
	outln()
	outln(st.title.Render("Timed runs (in order):"))

	for _, r := range results {
		if !r.ok() {
			continue
		}

		parts := make([]string, 0, len(r.runs))
		for _, d := range r.runs {
			parts = append(parts, formatDuration(d))
		}

		outf("  %-12s %s\n", r.entry.name, strings.Join(parts, "  "))
	}
}

// printFailures lists the genuine failures. Models the user interrupted are left out: the table already marks them
// INTERRUPTED, and repeating "context canceled" once per model buries the real errors.
func printFailures(results []result, st styles) {
	failures := make([]result, 0)
	for _, r := range results {
		if !r.ok() && !r.interrupted() {
			failures = append(failures, r)
		}
	}

	if len(failures) == 0 {
		return
	}

	outln()
	outln(st.fail.Render("Failures:"))

	for i, r := range failures {
		outf("  %d. %s: %v\n", i+1, r.entry.name, r.err)
	}
}

// printWarnings surfaces the silent GPU->CPU downgrades. Without this a CUDA run that actually executed on the CPU
// would be reported as a CUDA measurement.
func printWarnings(results []result, cfg config, st styles) {
	warnings := make([]string, 0)

	for _, r := range results {
		if r.fallback == nil {
			continue
		}

		warnings = append(warnings, fmt.Sprintf("%s: %s was requested but the session could not be built; "+
			"the model ran on the CPU (%v)", r.entry.name, r.fallback.provider, r.fallback.err))
	}

	if len(warnings) == 0 {
		return
	}

	outln()
	outln(st.warn.Render("Warnings:"))

	for i, w := range warnings {
		outf("  %d. %s\n", i+1, w)
	}

	if cfg.provider != types.ExecutionProviderCPU {
		outln(st.dim.Render("  Those rows are CPU timings, not " + string(cfg.provider) + " timings."))
	}
}

func printFooter(cfg config, st styles) {
	outln()
	outln(st.dim.Render("  MPix/s is input megapixels over the median run."))
	outln(st.dim.Render("  No GC control or CPU pinning: numbers are comparable within one invocation, " +
		"not across machines or processes."))

	if cfg.cache {
		outln(st.dim.Render("  --cache is on, so the timings include the library's PNG encode and disk write."))
	}
}

// formatDuration keeps the columns aligned. time.Duration.String() yields values like "842.318417ms", which vary in
// width and make the table unreadable.
func formatDuration(d time.Duration) string {
	if d <= 0 {
		return "-"
	}

	if d < time.Second {
		return fmt.Sprintf("%.1fms", float64(d)/float64(time.Millisecond))
	}

	return fmt.Sprintf("%.3fs", d.Seconds())
}

// printDownloadProgress reports the ONNX runtime and model downloads during Initialize. Without it a first run looks
// hung for several minutes.
func printDownloadProgress(_, _ int64, percent float64) {
	fmt.Fprintf(os.Stderr, "\rDownloading runtime... %3.0f%%", percent*100)

	if percent >= 1 {
		fmt.Fprintln(os.Stderr)
	}
}
