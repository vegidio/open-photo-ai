//! What the terminal shows while a run works: the `💾` dependency line while initializing, and one gradient bar over
//! the face detection and the whole chain while enhancing.
//!
//! ```text
//! initialize ──► [Live<Line> "💾 …"]   started on the first Downloading/Extracting report; frozen by finish_install
//! (run prints the header and a blank line)
//! detection + chain ──► [Live<AnimatedProgress>]   started by start_bar; finish_bar settles it and prints a blank line
//! ```
//!
//! Each line is a `rust_sak::tui` session, which must have stderr to itself, so `run` prints its own lines only
//! between them. In raw mode a Ctrl-C reaches the session's interrupt rather than the SIGINT handler: during the
//! dependency line it wakes [`View::install_interrupted`], and during the bar it cancels the run.
//!
//! Where stderr is not a terminal every piece here is a no-op, and no session is ever started.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use opai::{CancellationToken, InferenceProgress, OnInference, OnProgress, Phase, Progress};
use ratatui::text::Line;
use rust_sak::tui::{AnimatedProgress, Live, LiveLine, Target};
use tokio::sync::Notify;

use crate::look::{self, Look};

/// The terminal's view of a run. Cheap to clone: every clone draws the same lines.
#[derive(Clone)]
pub struct View {
    inner: Arc<Inner>,
}

struct Inner {
    look: Look,
    /// What a Ctrl-C during the bar stops.
    cancel: CancellationToken,
    /// Woken by a Ctrl-C during the dependency line.
    install_interrupt: Arc<Notify>,
    install: Mutex<Install>,
    bar: Mutex<Slot<AnimatedProgress>>,
}

/// The dependency line: the text last computed, and its session once one was started.
#[derive(Default)]
struct Install {
    /// The line the latest download or expansion report reads as, drawn or not.
    line: Option<Line<'static>>,
    slot: Slot<Line<'static>>,
}

/// One live session's state. A start is attempted at most once, so a terminal that refused one is not asked again on
/// every report.
struct Slot<L: LiveLine> {
    attempted: bool,
    live: Option<Live<L>>,
}

impl<L: LiveLine> Default for Slot<L> {
    fn default() -> Self {
        Self { attempted: false, live: None }
    }
}

impl<L: LiveLine> Slot<L> {
    /// Starts drawing `line` on stderr, unless a start was already attempted. A refusal is logged and leaves the line
    /// undrawn: losing progress is never a reason to fail a run.
    fn start(&mut self, what: &str, line: L, on_interrupt: impl Fn() + Send + 'static) {
        if self.attempted {
            return;
        }
        self.attempted = true;

        match Live::start(Target::Stderr, line, on_interrupt) {
            Ok(live) => self.live = Some(live),
            Err(error) => tracing::warn!("could not draw the {what} on the terminal: {error}"),
        }
    }

    /// Settles the session, leaving its last frame in the scrollback, and says whether one was drawn.
    fn finish(&mut self, what: &str) -> bool {
        let Some(live) = self.live.take() else {
            return false;
        };

        if let Err(error) = live.finish() {
            tracing::warn!("the {what} ended with a terminal error: {error}");
        }
        true
    }
}

impl View {
    /// A view drawing as `look` says, whose bar cancels `cancel` on a Ctrl-C.
    pub fn new(look: Look, cancel: CancellationToken) -> Self {
        Self {
            inner: Arc::new(Inner {
                look,
                cancel,
                install_interrupt: Arc::new(Notify::new()),
                install: Mutex::default(),
                bar: Mutex::default(),
            }),
        }
    }

    /// The install callback [`opai::InitOptions::on_progress`] takes.
    ///
    /// The dependency line is started by the first report of a download or an expansion, and rewritten by every one
    /// after it, the next dependency's name replacing the last. An already installed dependency, or a phase this crate
    /// does not know, draws nothing.
    pub fn on_install(&self) -> OnProgress {
        let view = self.clone();
        Arc::new(move |progress: &Progress| view.install_report(progress))
    }

    fn install_report(&self, progress: &Progress) {
        if !matches!(progress.phase, Phase::Downloading | Phase::Extracting) {
            return;
        }

        let line = look::dependency_line(progress.dependency.as_str(), progress.fraction, self.inner.look.color);
        let mut install = lock(&self.inner.install);
        install.line = Some(line.clone());

        if !self.inner.look.interactive {
            return;
        }

        match &install.slot.live {
            Some(live) => live.handle().update(|drawn| *drawn = line),
            None => {
                let interrupt = Arc::clone(&self.inner.install_interrupt);
                install.slot.start("dependency line", line, move || interrupt.notify_one());
            }
        }
    }

    /// Resolves on a Ctrl-C pressed while the dependency line is drawn.
    pub async fn install_interrupted(&self) {
        self.inner.install_interrupt.notified().await;
    }

    /// Freezes the dependency line where it was last drawn, with the cursor below it.
    pub fn finish_install(&self) {
        lock(&self.inner.install).slot.finish("dependency line");
    }

    /// Starts the bar at `Enhancing [0/planned]`, or draws nothing where `planned` is `0`.
    pub fn start_bar(&self, planned: usize) {
        if !self.inner.look.interactive || planned == 0 {
            return;
        }

        let mut line = AnimatedProgress::new(look::label(None));
        line.label_width = look::LABEL_WIDTH;
        line.counter = Some(look::counter(None, planned));
        line.color = self.inner.look.color;

        let view = self.clone();
        lock(&self.inner.bar).start("progress bar", line, move || view.interrupt_bar());
    }

    /// The inference callback the bar is moved by, meant for [`opai::Handover::new`].
    pub fn on_bar(&self) -> OnInference {
        let view = self.clone();
        Arc::new(move |report: &InferenceProgress| view.bar_report(report))
    }

    fn bar_report(&self, report: &InferenceProgress) {
        let bar = lock(&self.inner.bar);
        let Some(live) = &bar.live else {
            return;
        };

        live.handle().update(|line| {
            line.label = look::label(Some(report)).to_string();
            line.counter = Some(look::counter(Some(report), report.operation_count));
            line.fraction = report.chain_fraction;
        });
    }

    /// A Ctrl-C while the bar is drawn: the run stops at the library's next check, and the bar keeps drawing until it
    /// does.
    fn interrupt_bar(&self) {
        tracing::info!("stopping…");
        self.inner.cancel.cancel();
    }

    /// Settles the bar, leaving its last frame in the scrollback, and prints the blank line after it. Where no bar was
    /// drawn, prints nothing.
    pub fn finish_bar(&self) {
        if lock(&self.inner.bar).finish("progress bar") {
            eprintln!();
        }
    }
}

/// A callback that panicked while holding a lock leaves a line that is still worth drawing.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opai::Dependency;

    fn plain() -> Look {
        Look { interactive: false, color: false }
    }

    fn report(dependency: Dependency, phase: Phase, fraction: f64) -> Progress {
        Progress { dependency, phase, bytes: 0, total: None, fraction }
    }

    fn line_text(view: &View) -> Option<String> {
        lock(&view.inner.install)
            .line
            .as_ref()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
    }

    #[test]
    fn the_dependency_line_follows_the_reports_without_drawing_off_a_terminal() {
        let view = View::new(plain(), CancellationToken::new());
        let on_install = view.on_install();
        let mut seen = Vec::new();

        for progress in [
            report(Dependency::Runtime, Phase::Downloading, 0.34),
            report(Dependency::Runtime, Phase::Extracting, 0.9),
            report(Dependency::Runtime, Phase::Extracting, 1.0),
            report(Dependency::Cuda, Phase::Downloading, 0.05),
            report(Dependency::Cuda, Phase::Downloading, 1.0),
        ] {
            on_install(&progress);
            seen.push(line_text(&view).expect("a line for every transfer"));
        }

        assert_eq!(
            seen,
            [
                "💾 Downloading dependency ONNX Runtime   34%",
                "💾 Downloading dependency ONNX Runtime   90%",
                "💾 Downloading dependency ONNX Runtime   100%",
                format!("💾 Downloading dependency {}   5%", Dependency::Cuda.as_str()).as_str(),
                format!("💾 Downloading dependency {}   100%", Dependency::Cuda.as_str()).as_str(),
            ]
        );

        let install = lock(&view.inner.install);
        assert!(!install.slot.attempted, "no session is attempted off a terminal");
        assert!(install.slot.live.is_none());
    }

    #[test]
    fn an_already_installed_dependency_draws_nothing() {
        let view = View::new(plain(), CancellationToken::new());

        view.on_install()(&report(Dependency::Runtime, Phase::AlreadyInstalled, 1.0));

        assert_eq!(line_text(&view), None);
    }

    #[test]
    fn the_bar_starts_nothing_off_a_terminal_or_for_an_empty_chain() {
        let view = View::new(plain(), CancellationToken::new());
        view.start_bar(2);
        assert!(!lock(&view.inner.bar).attempted);

        let interactive = View::new(Look { interactive: true, color: false }, CancellationToken::new());
        interactive.start_bar(0);
        assert!(!lock(&interactive.inner.bar).attempted);
    }

    #[test]
    fn a_ctrl_c_on_the_bar_cancels_the_run() {
        let cancel = CancellationToken::new();
        let view = View::new(plain(), cancel.clone());

        view.interrupt_bar();

        assert!(cancel.is_cancelled());
    }

    #[test]
    fn finishing_without_a_bar_prints_nothing_and_says_so() {
        let view = View::new(plain(), CancellationToken::new());

        assert!(!lock(&view.inner.bar).finish("progress bar"));
        assert!(!lock(&view.inner.install).slot.finish("dependency line"));
    }
}
