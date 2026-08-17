package main

import (
	"context"
	"fmt"
	"math"
	"os"
	"strconv"
	"sync/atomic"
	"time"

	"charm.land/bubbles/v2/progress"
	"charm.land/bubbles/v2/spinner"
	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"github.com/vegidio/open-photo-ai/types"
)

// phase is the stage a model is currently in. It exists so the live view can say "warm-up 1/1" rather than leaving the
// user staring at a spinner during the minutes a first-time model download takes.
type phase int

const (
	phaseBuild phase = iota
	phaseWarmup
	phaseCold
	phaseRun
)

func (p phase) label() string {
	switch p {
	case phaseBuild:
		return "preparing"
	case phaseWarmup:
		return "warm-up"
	case phaseCold:
		return "cold start"
	case phaseRun:
		return "run"
	default:
		return ""
	}
}

// sweepListener is how the sweep and the models inside it report where they are: phase/runDone during one model,
// modelStart/modelDone around it.
//
// Implementations must be cheap and non-blocking: phase is called from the sweep goroutine between timed sections, but
// a live view also feeds a progress callback that fires from inside them.
type sweepListener interface {
	phase(p phase, current, total int)
	runDone(elapsed time.Duration)
	modelStart(index int, e entry)
	modelDone(index int, r result)
}

// sweep benchmarks every selected model in order. One model failing is recorded and the sweep continues; only a
// cancelled context stops it early, in which case the results gathered so far are returned.
func sweep(
	ctx context.Context,
	selection []entry,
	input *types.ImageData,
	cfg config,
	rec *fallbackRecorder,
	listener sweepListener,
) []result {
	results := make([]result, 0, len(selection))

	for i, e := range selection {
		if ctx.Err() != nil {
			break
		}

		listener.modelStart(i, e)
		res := benchmark(ctx, e, input, cfg, rec, listener)
		results = append(results, res)
		listener.modelDone(i, res)
	}

	return results
}

// region - Plain renderer

// plainListener prints one line per model to stderr as the sweep progresses. It is used when stdout isn't a terminal
// (so `perftest > results.txt` still shows progress on the console) and whenever --verbose is on, because the debug
// logger writes to stderr and would tear a live view apart.
type plainListener struct {
	total int
}

func (p plainListener) phase(phase, int, int) {}
func (p plainListener) runDone(time.Duration) {}

func (p plainListener) modelStart(index int, e entry) {
	fmt.Fprintf(os.Stderr, "[%*d/%d] %s ...\n", digits(p.total), index+1, p.total, e.name)
}

func (p plainListener) modelDone(index int, r result) {
	prefix := fmt.Sprintf("[%*d/%d] %s", digits(p.total), index+1, p.total, r.entry.name)

	if r.interrupted() {
		fmt.Fprintf(os.Stderr, "%s INTERRUPTED\n", prefix)
		return
	}

	if !r.ok() {
		fmt.Fprintf(os.Stderr, "%s FAILED: %v\n", prefix, r.err)
		return
	}

	fmt.Fprintf(os.Stderr, "%s cold %s   median %s\n", prefix, formatDuration(r.cold), formatDuration(r.stats.median))
}

var _ sweepListener = plainListener{}

// endregion

// region - Live renderer

// The live view is a Bubble Tea program that owns the terminal while the sweep runs on a background goroutine. It
// deliberately does NOT use the alternate screen: completed models should stay in the scrollback once the program
// exits.

type startMsg struct {
	index int
	entry entry
}

type phaseMsg struct {
	phase   phase
	current int
	total   int
}

// doneModelMsg only clears the active line; the finished model has already been printed into the scrollback.
type doneModelMsg struct{}

type finishedMsg struct{}

type runDoneMsg struct {
	elapsed time.Duration
}

type tickMsg time.Time

// teaListener forwards sweep events into the program. Program.Send and Program.Printf are both safe to call from any
// goroutine, and both go through the same queue, so a model's completed line can't overtake the next model's start.
type teaListener struct {
	program *tea.Program
	styles  styles
}

func (t teaListener) modelStart(index int, e entry) { t.program.Send(startMsg{index: index, entry: e}) }
func (t teaListener) runDone(d time.Duration)       { t.program.Send(runDoneMsg{elapsed: d}) }

// modelDone prints the finished model above the live view rather than adding it to the view's own content. Printf
// output is unmanaged by the program, so it survives every later render and is still on screen once the program has
// exited — which is the whole point of not using the alternate screen.
func (t teaListener) modelDone(_ int, r result) {
	t.program.Printf("%s", completedLine(r, t.styles))
	t.program.Send(doneModelMsg{})
}

func (t teaListener) phase(p phase, current, total int) {
	t.program.Send(phaseMsg{phase: p, current: current, total: total})
}

var _ sweepListener = teaListener{}

// tileProgress carries the fraction reported by the library's per-tile progress callback from the inference goroutine
// to the renderer.
//
// It is an atomic float, not a channel or a mutex, because the callback that writes it runs INSIDE the timed section:
// a single atomic store per tile is invisible in the measurement, whereas a channel send could block on the renderer
// and a terminal write certainly would. The renderer polls it on a timer instead of being pushed to.
type tileProgress struct {
	bits atomic.Uint64
}

func (t *tileProgress) set(v float64) { t.bits.Store(math.Float64bits(v)) }
func (t *tileProgress) get() float64  { return math.Float64frombits(t.bits.Load()) }

// callback returns the types.InferenceProgress to hand to opai.Process. The operation name is ignored: the view
// already knows which model is running.
func (t *tileProgress) callback() types.InferenceProgress {
	return func(_ string, fraction float64) { t.set(fraction) }
}

type liveModel struct {
	total   int
	spinner spinner.Model
	bar     progress.Model
	tiles   *tileProgress
	styles  styles

	index   int
	current entry
	started bool
	phase   phase
	step    int
	steps   int
	last    time.Duration
}

// frameRate is how often the view re-reads the tile progress. 20 fps is smooth to the eye and still far below the rate
// at which tiles complete, so the bar moves continuously rather than in visible jumps.
const frameRate = time.Second / 20

func newLiveModel(total int, tiles *tileProgress, st styles) liveModel {
	return liveModel{
		total:   total,
		spinner: spinner.New(spinner.WithSpinner(spinner.Dot), spinner.WithStyle(st.spinner)),
		bar:     progress.New(progress.WithDefaultBlend(), progress.WithWidth(24), progress.WithoutPercentage()),
		tiles:   tiles,
		styles:  st,
	}
}

func (m liveModel) Init() tea.Cmd {
	return tea.Batch(m.spinner.Tick, tick())
}

func tick() tea.Cmd {
	return tea.Tick(frameRate, func(t time.Time) tea.Msg { return tickMsg(t) })
}

func (m liveModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case startMsg:
		m.index = msg.index
		m.current = msg.entry
		m.started = true
		m.phase = phaseBuild
		m.step, m.steps = 0, 0
		m.last = 0
		m.tiles.set(0)
		return m, nil

	case runDoneMsg:
		m.last = msg.elapsed
		return m, nil

	case phaseMsg:
		m.phase = msg.phase
		m.step, m.steps = msg.current, msg.total
		m.tiles.set(0)
		return m, nil

	case doneModelMsg:
		m.started = false
		return m, nil

	case finishedMsg:
		return m, tea.Quit

	case tickMsg:
		return m, tick()

	case tea.KeyPressMsg:
		// Ctrl-C reaches the sweep through the cancelled context; quitting here as well would tear the terminal down
		// while a native inference is still running.
		return m, nil

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return m, cmd
	}

	return m, nil
}

// View renders only the model currently being benchmarked. Finished models are not part of the view: they were
// printed into the scrollback by teaListener.modelDone, so the managed area stays one line tall no matter how many
// models the sweep covers.
func (m liveModel) View() tea.View {
	if !m.started {
		return tea.NewView("")
	}

	return tea.NewView(m.activeLine())
}

// completedLine is the permanent one-line summary printed above the live view when a model finishes.
func completedLine(r result, st styles) string {
	name := st.name.Render(pad(r.entry.name, nameWidth))

	if r.interrupted() {
		return fmt.Sprintf("  %s %s %s", st.warn.Render("—"), name, st.warn.Render("INTERRUPTED"))
	}

	if !r.ok() {
		return fmt.Sprintf("  %s %s %s", st.fail.Render("✗"), name, st.fail.Render("FAILED"))
	}

	return fmt.Sprintf("  %s %s cold %s   median %s",
		st.ok.Render("✓"), name,
		st.value.Render(formatDuration(r.cold)),
		st.value.Render(formatDuration(r.stats.median)))
}

func (m liveModel) activeLine() string {
	counter := fmt.Sprintf("(%*d/%d)", digits(m.total), m.index+1, m.total)

	stage := m.phase.label()
	if m.steps > 0 {
		stage = fmt.Sprintf("%s %d/%d", stage, m.step, m.steps)
	}

	last := ""
	if m.last > 0 {
		last = m.styles.dim.Render("  last " + formatDuration(m.last))
	}

	return fmt.Sprintf("  %s %s %s  %s  %s%s",
		m.spinner.View(),
		m.styles.name.Render(pad(m.current.name, nameWidth)),
		m.styles.dim.Render(counter),
		m.styles.dim.Render(pad(stage, 14)),
		m.bar.ViewAs(m.tiles.get()),
		last)
}

// runLive drives the sweep inside a Bubble Tea program and returns its results once the program has exited and the
// terminal is ours again, so the summary can be printed normally.
//
// The results travel on a channel from the sweep goroutine, not on the program's final model. The program can end
// without the sweep having reported in - a cancelled context stops it early - and results that depend on the UI
// surviving would be silently lost exactly when there is something worth showing.
func runLive(ctx context.Context, run func(listener sweepListener) []result, total int, tiles *tileProgress) []result {
	st := newStyles()

	// WithContext ties the program's lifetime to the same cancellation the sweep observes, so Ctrl-C can't leave the
	// program running against a sweep that has already stopped.
	program := tea.NewProgram(newLiveModel(total, tiles, st), tea.WithContext(ctx))
	done := make(chan []result, 1)

	go func() {
		results := run(teaListener{program: program, styles: st})
		program.Send(finishedMsg{})
		done <- results
	}()

	// The error is deliberately ignored: a cancelled context surfaces here, and the sweep observes the same
	// cancellation, so the results waiting on the channel are what the user should see either way.
	_, _ = program.Run()

	// Blocks until the sweep goroutine is finished, which also guarantees no inference is still running when the
	// caller's deferred opai.Destroy tears the native runtime down.
	return <-done
}

// endregion

const nameWidth = 11

func pad(s string, width int) string {
	return fmt.Sprintf("%-*s", width, s)
}

// digits is how wide a counter has to be to hold n, so "[ 9/14]" lines up with "[10/14]".
func digits(n int) int {
	return len(strconv.Itoa(n))
}

// region - Styles

type styles struct {
	title   lipgloss.Style
	label   lipgloss.Style
	dim     lipgloss.Style
	name    lipgloss.Style
	value   lipgloss.Style
	ok      lipgloss.Style
	fail    lipgloss.Style
	warn    lipgloss.Style
	spinner lipgloss.Style
	header  lipgloss.Style
	cell    lipgloss.Style
	border  lipgloss.Style
}

func newStyles() styles {
	return styles{
		title:   lipgloss.NewStyle().Bold(true),
		label:   lipgloss.NewStyle().Foreground(lipgloss.Color("245")),
		dim:     lipgloss.NewStyle().Foreground(lipgloss.Color("241")),
		name:    lipgloss.NewStyle().Bold(true),
		value:   lipgloss.NewStyle().Foreground(lipgloss.Color("39")),
		ok:      lipgloss.NewStyle().Foreground(lipgloss.Color("42")),
		fail:    lipgloss.NewStyle().Foreground(lipgloss.Color("203")),
		warn:    lipgloss.NewStyle().Foreground(lipgloss.Color("214")),
		spinner: lipgloss.NewStyle().Foreground(lipgloss.Color("39")),
		header:  lipgloss.NewStyle().Bold(true).Padding(0, 1),
		cell:    lipgloss.NewStyle().Padding(0, 1),
		border:  lipgloss.NewStyle().Foreground(lipgloss.Color("238")),
	}
}

// endregion
