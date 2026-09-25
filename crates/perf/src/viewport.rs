//! The live view: the listener that feeds it, the terminal it draws on, and the one piece of terminal state it takes.
//!
//! # What this does not do
//!
//! It does not read the keyboard, it does not enter raw mode, and it does not use the alternate screen. Raw mode
//! exists to read keys, and nothing here reads one: a Ctrl-C reaches the process as a signal, where the sweep's own
//! handler cancels the token a run honours — so a sweep stops the same way whether or not anyone is watching. Under
//! raw mode that signal would have to be synthesised from a key event, and a torn-down terminal — a panic, a
//! `kill -9`, a crash inside ONNX Runtime, which is native code running the models this binary exists to stress —
//! would leave the shell without an echo. That is a real failure mode for a benchmark that runs unattended, and the
//! way not to have it is not to enter raw mode.
//!
//! The cost is that the terminal echoes `^C` into the viewport line on an interrupt. It is the last frame before the
//! summary, and the summary immediately overwrites it.
//!
//! # Why the view is one line and the finished models are not in it
//!
//! [`ratatui::Viewport::Inline`] of one row, and each finished model written into the terminal's own scrollback with
//! [`Terminal::insert_before`]. A view that kept its finished models would be capped at a screenful, would redraw
//! every finished row on every frame, and would take the whole history down with it when the process ended.

use std::io::{Stderr, stderr};
use std::sync::{Arc, Once};

use opai::OnInference;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::{cursor, execute};
use ratatui::layout::Position;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use ratatui::{Terminal, TerminalOptions};

use crate::select::Selected;
use crate::sweep::{Listener, ModelResult, Stage};
use crate::view::{self, Shared};

/// How many rows the view manages, however many models the sweep covers.
///
/// Everything above the view belongs to the terminal. One row is also why [`view::line`] never wraps.
const HEIGHT: u16 = 1;

/// The [`Listener`] half of the live view: what the sweep tells, which writes into the state and nothing else.
///
/// It draws nothing. The terminal is owned by [`Viewport`], which polls the same state on its own timer — so a
/// listener call from the sweep can never wait on a terminal write, and nothing inside a measured section does.
pub struct Watcher {
    shared: Arc<Shared>,
}

impl Watcher {
    /// A listener writing into `shared`.
    pub fn new(shared: &Arc<Shared>) -> Self {
        Self { shared: Arc::clone(shared) }
    }
}

impl Listener for Watcher {
    fn started(&mut self, index: usize, total: usize, selected: &Selected) {
        self.shared.started(index, total, selected);
    }

    fn stage(&mut self, stage: Stage) {
        self.shared.stage(stage);
    }

    fn on_progress(&self) -> Option<OnInference> {
        Some(self.shared.on_progress())
    }

    fn finished(&mut self, result: &ModelResult) {
        self.shared.finished(view::finished(result));
    }
}

/// The drawing half: a one-row inline viewport over the same state.
pub struct Viewport<B: Backend> {
    terminal: Terminal<B>,
    shared: Arc<Shared>,

    /// What the spinner is indexed by. A frame count rather than a clock, so the animation is the same speed
    /// wherever the ticker actually fires.
    tick: usize,

    /// Restores the cursor however this ends, including a panic — see [`CursorGuard`].
    _cursor: CursorGuard,
}

impl Viewport<CrosstermBackend<Stderr>> {
    /// Opens the view on stderr.
    ///
    /// # Errors
    ///
    /// Whatever the terminal refused. A view that cannot be opened is reported and the sweep falls back to the plain
    /// renderer: a benchmark that cannot draw is still a benchmark.
    pub fn open(shared: &Arc<Shared>) -> std::io::Result<Self> {
        Self::over(CrosstermBackend::new(stderr()), shared)
    }
}

impl<B: Backend> Viewport<B> {
    /// Opens a one-row inline view over `backend`.
    ///
    /// # Errors
    ///
    /// Whatever the backend refused.
    pub fn over(backend: B, shared: &Arc<Shared>) -> Result<Self, B::Error> {
        let options = TerminalOptions { viewport: ratatui::Viewport::Inline(HEIGHT) };
        let terminal = Terminal::with_options(backend, options)?;

        Ok(Self { terminal, shared: Arc::clone(shared), tick: 0, _cursor: CursorGuard::hide() })
    }

    /// One frame: everything that finished since the last one, then the model in flight.
    ///
    /// The state is copied out before anything is drawn, so no lock is held across terminal I/O.
    ///
    /// # Errors
    ///
    /// Whatever the backend refused.
    pub fn draw(&mut self) -> Result<(), B::Error> {
        self.scrollback()?;

        let snapshot = self.shared.snapshot();
        let tick = self.tick;
        self.tick = self.tick.wrapping_add(1);

        self.terminal.draw(|frame| {
            let area = frame.area();
            let composed = view::line(&snapshot, tick, area.width as usize);

            frame.render_widget(Line::from(composed), area);
        })?;

        Ok(())
    }

    /// Closes the view, leaving the scrollback it wrote and nothing of itself.
    ///
    /// The cursor is put back at the start of the row the view occupied before it is cleared, so the summary printed
    /// next begins where the view was rather than beside it.
    ///
    /// # Errors
    ///
    /// Whatever the backend refused.
    pub fn close(mut self) -> Result<(), B::Error> {
        self.scrollback()?;

        let row = self.terminal.get_frame().area().y;
        self.terminal.set_cursor_position(Position::new(0, row))?;
        self.terminal.clear()?;
        self.terminal.flush()?;

        Ok(())
    }

    /// Writes every model that has finished into the terminal's own scrollback.
    ///
    /// Above the view rather than into it, and by the terminal's scrolling region rather than by clearing and
    /// redrawing — which is what keeps a sweep of twenty models from flickering once per model.
    fn scrollback(&mut self) -> Result<(), B::Error> {
        for line in self.shared.scrollback() {
            self.terminal.insert_before(HEIGHT, |buffer| Line::from(line).render(buffer.area, buffer))?;
        }

        Ok(())
    }
}

/// The one piece of terminal state the view takes, and the two ways it is given back.
///
/// Hidden on construction, shown again by [`Drop`] — which covers the ordinary end, a Ctrl-C that unwinds, and an
/// early return — and by a panic hook for the case `Drop` does not run because the process is aborting.
struct CursorGuard;

impl CursorGuard {
    fn hide() -> Self {
        install_panic_hook();
        let _ = execute!(stderr(), cursor::Hide);

        Self
    }
}

impl Drop for CursorGuard {
    fn drop(&mut self) {
        let _ = execute!(stderr(), cursor::Show);
    }
}

/// Shows the cursor before whatever was going to be printed about the panic.
///
/// Once per process, chained onto the hook already installed rather than replacing it: the panic message is what
/// says why the benchmark stopped, and losing it to restore a cursor would be the wrong trade.
fn install_panic_hook() {
    static INSTALLED: Once = Once::new();

    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();

        std::panic::set_hook(Box::new(move |info| {
            let _ = execute!(stderr(), cursor::Show);
            previous(info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats;
    use crate::sweep::{Measured, Outcome, Verdict};
    use opai::{ExecutionProvider, Family, FloatPrecision, Scale, Upscale};
    use ratatui::backend::TestBackend;
    use std::time::Duration;

    /// A terminal wide enough for the whole line and tall enough for a scrollback to land in.
    fn backend() -> TestBackend {
        TestBackend::new(100, 8)
    }

    fn selected(codename: &'static str) -> Selected {
        Selected {
            codename,
            family: Family::Upscale,
            operation: Upscale::kyoto(FloatPrecision::Fp32, Scale::new(4.0).expect("4x is in range")),
            blocked: None,
        }
    }

    fn completed(codename: &'static str) -> ModelResult {
        let runs = vec![Duration::from_millis(412)];

        ModelResult {
            codename,
            family: Family::Upscale,
            display_name: "Kyoto 4x (FP32)".to_string(),
            outcome: Outcome::Completed(Box::new(Measured {
                cold: Duration::from_millis(2_145),
                stats: stats::compute(&runs),
                runs,
                output: (2560, 2560),
                providers: Verdict::AsRequested(ExecutionProvider::Cpu),
            })),
        }
    }

    /// Every row of the terminal, trailing blanks trimmed.
    fn rows(view: &Viewport<TestBackend>) -> Vec<String> {
        let buffer = view.terminal.backend().buffer();

        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn the_view_is_one_row_however_many_models_have_finished() {
        let shared = Shared::new();
        let mut watcher = Watcher::new(&shared);
        let mut view = Viewport::over(backend(), &shared).expect("the view opens");

        for index in 0..4 {
            watcher.started(index, 4, &selected("kyoto"));
            watcher.stage(Stage::Timed { run: 1, of: 1 });
            view.draw().expect("a frame");
            watcher.finished(&completed("kyoto"));
            view.draw().expect("a frame");
        }

        watcher.started(4, 4, &selected("osaka"));
        watcher.stage(Stage::Timed { run: 2, of: 5 });
        view.draw().expect("a frame");

        // One row carries the model in flight, and four models finishing did not make the view four rows taller.
        let written = rows(&view);
        let active = written.iter().filter(|row| row.contains("osaka")).count();

        assert_eq!(active, 1, "{written:#?}");
    }

    #[test]
    fn a_finished_model_goes_into_the_scrollback_with_its_cold_start_and_its_median() {
        let shared = Shared::new();
        let mut watcher = Watcher::new(&shared);
        let mut view = Viewport::over(backend(), &shared).expect("the view opens");

        watcher.started(0, 2, &selected("kyoto"));
        watcher.finished(&completed("kyoto"));
        view.draw().expect("a frame");

        let written = rows(&view).join("\n");

        assert!(written.contains("kyoto"), "{written}");
        assert!(written.contains(&crate::report::duration(Duration::from_millis(2_145)).trim().to_string()));
        assert!(written.contains(&crate::report::duration(Duration::from_millis(412)).trim().to_string()));

        // And it is written once: a second frame does not re-insert it, which is what would flicker.
        assert!(shared.scrollback().is_empty());
    }

    #[test]
    fn a_model_that_was_declined_failed_or_interrupted_says_which_rather_than_a_timing() {
        let reasons = [
            (
                Outcome::Declined {
                    reason: "no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph"
                        .to_string(),
                },
                "declined",
            ),
            (Outcome::Failed { reason: "cold-start run: out of memory".to_string() }, "failed"),
            (Outcome::Interrupted, "interrupted"),
        ];

        for (outcome, expected) in reasons {
            let result = ModelResult { outcome, ..completed("osaka") };
            let line = view::finished(&result);

            assert!(line.contains("osaka"), "{line}");
            assert!(line.contains(expected), "{line}");
        }
    }

    #[test]
    fn what_is_left_in_the_scrollback_when_a_sweep_ends_is_written_before_the_view_is_closed() {
        let shared = Shared::new();
        let mut watcher = Watcher::new(&shared);
        let view = Viewport::over(backend(), &shared).expect("the view opens");

        // The last model of a sweep finishes and nothing ticks after it — which is the ordinary ending, and an
        // interrupted sweep besides.
        watcher.started(0, 1, &selected("tokyo"));
        watcher.finished(&completed("tokyo"));

        view.close().expect("the view closes");

        // Nothing of it is still queued, so nothing of it was lost.
        assert!(shared.scrollback().is_empty());
    }

    #[test]
    fn the_watcher_registers_a_callback_and_the_two_renderers_beside_it_do_not() {
        let shared = Shared::new();

        assert!(Watcher::new(&shared).on_progress().is_some());
        assert!(crate::sweep::Silent.on_progress().is_none());
        assert!(crate::sweep::Lines.on_progress().is_none());
    }

    #[test]
    fn a_panic_gives_the_cursor_back_without_taking_the_panic_message_with_it() {
        let _guard = CursorGuard::hide();

        // Panicked on **another** thread, so the guard on this one is still alive and has not run: what puts the
        // cursor back here is the hook, which is the half that covers a panic `Drop` never sees.
        let panicked = std::thread::spawn(|| panic!("deliberate, to prove the hook is installed")).join();

        let payload = panicked.expect_err("the thread panicked");
        let message = payload.downcast_ref::<&str>().copied().unwrap_or_default();

        // Chained rather than replacing, so the panic message survives the hook that restores the cursor.
        assert!(message.contains("deliberate"), "{message}");
    }

    #[test]
    fn neither_raw_mode_nor_the_alternate_screen_is_ever_entered() {
        // The property the module note rests on, and one that cannot be asserted from a value: what makes a panic or
        // a `kill -9` survivable here is that there is no terminal state to restore but the cursor. Read over the
        // source, because what is being asserted is the absence of a call.
        let source = include_str!("viewport.rs");
        let module = source.split("#[cfg(test)]").next().expect("the file has a first part");

        for forbidden in ["enable_raw_mode", "EnterAlternateScreen", "raw_mode", "AlternateScreen", "event::read"] {
            let calls: Vec<&str> = module
                .lines()
                .filter(|line| line.contains(forbidden) && !line.trim_start().starts_with("//"))
                .collect();

            assert!(calls.is_empty(), "the view takes terminal state it cannot give back: {calls:?}");
        }

        // And the one piece it does take is given back by a `Drop` rather than by a path that an early return, a
        // panic or a Ctrl-C could skip.
        assert!(module.contains("impl Drop for CursorGuard"));
        assert!(module.contains("cursor::Show"));
    }
}
