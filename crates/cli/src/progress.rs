//! What the terminal shows while a run works: the `💾` dependency line while initializing, then one view for the whole
//! batch, the current image's gradient bar over a list of the most recent images.
//!
//! ```text
//! initialize ──► [Live<Line> "💾 …"]   started on the first Downloading/Extracting report; frozen by finish_install
//! the batch  ──► [Live<BatchLine>]      started by start_batch; finish_batch freezes it and prints a blank line
//!
//!                                                                <- blank, only under a drawn dependency line
//!   Face Recovery   [1/2]  ████████████░░░░░░░░░░░   38.4%      <- the current image's bar, fresh for each image
//!                                                                <- blank
//!   🖼️ [6/8] Enhanced image photos/f.jpg                         <- the list: the five most recent images,
//!   🖼️ [7/8] Enhancing image photos/g.jpg                           start_image pushes a row, end_image settles it
//! ```
//!
//! Each is a `rust_sak::tui` session, which must have stderr to itself, so `run` prints its own lines only between and
//! after them. In raw mode a Ctrl-C reaches the session's interrupt rather than the SIGINT handler: during the
//! dependency line it wakes [`View::install_interrupted`], and during the batch it cancels the run, whether an image is
//! being loaded, enhanced or written.
//!
//! Where stderr is not a terminal, or the terminal refused the batch's session, no view is drawn and each image prints
//! one line when it ends, [`look::plain_row`].

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use opai::{CancellationToken, InferenceProgress, OnInference, OnProgress, Phase, Progress};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::Widget;
use rust_sak::tui::{AnimatedProgress, Live, LiveLine, Target};
use tokio::sync::Notify;

use crate::look::{self, ImageEnd, Look, RowState};

/// How many images the list shows: the most recent.
const LIST_ROWS: usize = 5;

/// The terminal's view of a run. Cheap to clone: every clone draws the same lines.
#[derive(Clone)]
pub struct View {
    inner: Arc<Inner>,
}

struct Inner {
    look: Look,
    /// What a Ctrl-C during the batch stops.
    cancel: CancellationToken,
    /// Woken by a Ctrl-C during the dependency line.
    install_interrupt: Arc<Notify>,
    install: Mutex<Install>,
    batch: Mutex<Batch>,
}

/// The batch's view: how many images it has, and its session once one was started.
#[derive(Default)]
struct Batch {
    total: usize,
    slot: Slot<Live<BatchLine>>,
}

/// What the batch's session draws: a blank row under the dependency line when there is one, the current image's bar, a
/// blank row, then the most recent images.
struct BatchLine {
    /// A blank row above the bar, separating it from a drawn dependency line. Part of the session, so a terminal that
    /// refuses it is left no stray blank line.
    gap: bool,
    /// `None` leaves the bar's row blank: no image started yet, or one with nothing left to run.
    bar: Option<AnimatedProgress>,
    /// At most [`LIST_ROWS`], the most recent last.
    rows: VecDeque<Row>,
    /// The `N` in `[n/N]`.
    total: usize,
    color: bool,
}

/// One image in the list.
struct Row {
    /// Its place in the run, from one.
    index: usize,
    /// Its path as it was given or found.
    path: String,
    state: RowState,
}

impl BatchLine {
    fn new(total: usize, gap: bool, color: bool) -> Self {
        Self { gap, bar: None, rows: VecDeque::with_capacity(LIST_ROWS), total, color }
    }

    /// Adds `row` as the most recent, dropping the oldest once the list is full.
    fn push(&mut self, row: Row) {
        if self.rows.len() == LIST_ROWS {
            self.rows.pop_front();
        }
        self.rows.push_back(row);
    }

    /// Sets the state of the image at `index`, if it is still listed.
    fn set(&mut self, index: usize, state: RowState) {
        if let Some(row) = self.rows.iter_mut().rev().find(|row| row.index == index) {
            row.state = state;
        }
    }
}

impl LiveLine for BatchLine {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let [_, area] = Layout::vertical([Constraint::Length(u16::from(self.gap)), Constraint::Fill(1)]).areas(area);
        let [bar, _, list] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).areas(area);
        if let Some(progress) = &mut self.bar {
            progress.render(bar, buf);
        }

        // In a terminal too short for the whole list, the most recent rows that fit.
        let shown = self.rows.len().saturating_sub(usize::from(list.height));
        for (row, area) in self.rows.iter().skip(shown).zip(list.rows()) {
            look::row(row.index, self.total, &row.path, row.state, self.color).render(area, buf);
        }
    }

    fn settled(&self) -> bool {
        self.bar.as_ref().is_none_or(AnimatedProgress::settled)
    }

    fn snap(&mut self) {
        if let Some(bar) = &mut self.bar {
            bar.snap();
        }
    }

    #[allow(clippy::cast_possible_truncation, reason = "at most 3 + LIST_ROWS")]
    fn height(&self) -> u16 {
        (usize::from(self.gap) + 2 + self.total.min(LIST_ROWS)) as u16
    }
}

/// The dependency line: the text last computed, and its session once one was started.
#[derive(Default)]
struct Install {
    /// The line the latest download or expansion report reads as, drawn or not.
    line: Option<Line<'static>>,
    slot: Slot<Live<Line<'static>>>,
    /// Whether the line was drawn, once [`View::finish_install`] froze it.
    drawn: bool,
}

/// One live session's state. A start is attempted whenever none is live, but never again once the terminal refused
/// one, so a terminal that cannot draw is not asked on every report, nor once the slot was [closed](Slot::close).
struct Slot<S> {
    closed: bool,
    live: Option<S>,
    /// How many starts were attempted, for the tests to count.
    #[cfg(test)]
    attempts: usize,
}

impl<S> Default for Slot<S> {
    fn default() -> Self {
        Self {
            closed: false,
            live: None,
            #[cfg(test)]
            attempts: 0,
        }
    }
}

/// What a [`Slot`] holds while drawing: a [`Live`] session, or a test's stand-in for one.
trait Session {
    /// Settles the session, leaving its last frame in the scrollback.
    fn finish(self) -> std::io::Result<()>;
}

impl<L: LiveLine> Session for Live<L> {
    fn finish(self) -> std::io::Result<()> {
        Live::finish(self)
    }
}

impl<S: Session> Slot<S> {
    /// Starts drawing with `open`, unless a session is already live or the slot is closed. A refusal is logged, closes
    /// the slot and leaves the line undrawn: losing progress is never a reason to fail a run.
    fn start(&mut self, what: &str, open: impl FnOnce() -> std::io::Result<S>) {
        if self.closed || self.live.is_some() {
            return;
        }
        #[cfg(test)]
        {
            self.attempts += 1;
        }

        match open() {
            Ok(live) => self.live = Some(live),
            Err(error) => {
                self.closed = true;
                tracing::warn!("could not draw the {what} on the terminal: {error}");
            }
        }
    }

    /// Settles the session, and says whether one was drawn.
    fn finish(&mut self, what: &str) -> bool {
        let Some(live) = self.live.take() else {
            return false;
        };

        if let Err(error) = live.finish() {
            tracing::warn!("the {what} ended with a terminal error: {error}");
        }
        true
    }

    /// Settles the session, as [`finish`](Self::finish) does, says whether one was drawn, and never starts another.
    fn close(&mut self, what: &str) -> bool {
        self.closed = true;
        self.finish(what)
    }
}

impl View {
    /// A view drawing as `look` says, whose batch cancels `cancel` on a Ctrl-C.
    pub fn new(look: Look, cancel: CancellationToken) -> Self {
        Self {
            inner: Arc::new(Inner {
                look,
                cancel,
                install_interrupt: Arc::new(Notify::new()),
                install: Mutex::default(),
                batch: Mutex::default(),
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
                install
                    .slot
                    .start("dependency line", || Live::start(Target::Stderr, line, move || interrupt.notify_one()));
            }
        }
    }

    /// Resolves on a Ctrl-C pressed while the dependency line is drawn.
    pub async fn install_interrupted(&self) {
        self.inner.install_interrupt.notified().await;
    }

    /// Freezes the dependency line where it was last drawn, with the cursor below it. A report arriving after this
    /// draws nothing, so the line appears at most once and never under the batch's view.
    pub fn finish_install(&self) {
        let mut install = lock(&self.inner.install);
        install.drawn = install.slot.close("dependency line");
    }

    /// Starts the batch's view for `total` images on a terminal, a blank row under the dependency line when one was
    /// drawn. A Ctrl-C while it is drawn cancels the run. Off a terminal, or where the terminal refuses the session,
    /// prints nothing.
    pub fn start_batch(&self, total: usize) {
        let gap = lock(&self.inner.install).drawn;
        let mut batch = lock(&self.inner.batch);
        batch.total = total;

        if !self.inner.look.interactive {
            return;
        }

        let line = BatchLine::new(total, gap, self.inner.look.color);
        let view = self.clone();
        batch.slot.start("batch view", || Live::start(Target::Stderr, line, move || view.interrupt()));
    }

    /// Lists the image at `index` (from one) as `Enhancing`, and starts a fresh bar at `Enhancing [0/planned]`, or
    /// leaves the bar's row blank where `planned` is `0`. A fresh bar starts empty rather than moving back down.
    pub fn start_image(&self, index: usize, path: &Path, planned: usize) {
        let batch = lock(&self.inner.batch);
        let Some(live) = &batch.slot.live else {
            return;
        };

        let bar = (planned > 0).then(|| {
            let mut bar = AnimatedProgress::new(look::label(None));
            bar.label_width = look::LABEL_WIDTH;
            bar.counter = Some(look::counter(None, planned));
            bar.color = self.inner.look.color;
            bar
        });
        let row = Row { index, path: path.display().to_string(), state: RowState::Enhancing };

        live.handle().update(|line| {
            line.bar = bar;
            line.push(row);
        });
    }

    /// The inference callback the current image's bar is moved by, meant for [`opai::Handover::new`].
    pub fn on_bar(&self) -> OnInference {
        let view = self.clone();
        Arc::new(move |report: &InferenceProgress| view.bar_report(report))
    }

    fn bar_report(&self, report: &InferenceProgress) {
        let batch = lock(&self.inner.batch);
        let Some(live) = &batch.slot.live else {
            return;
        };

        live.handle().update(|line| {
            if let Some(bar) = &mut line.bar {
                bar.label = look::label(Some(report)).to_string();
                bar.counter = Some(look::counter(Some(report), report.operation_count));
                bar.fraction = report.chain_fraction;
            }
        });
    }

    /// Settles the row of the image at `index` as `end` says, or, where no view is drawn, prints its plain line.
    pub fn end_image(&self, index: usize, path: &Path, end: &ImageEnd) {
        if let Some(line) = self.end_line(index, path, end) {
            eprintln!("{line}");
        }
    }

    /// What [`end_image`](Self::end_image) prints: nothing while the view is drawn, and the image's plain line
    /// otherwise, coloured as the look says.
    pub(crate) fn end_line(&self, index: usize, path: &Path, end: &ImageEnd) -> Option<String> {
        let batch = lock(&self.inner.batch);
        if let Some(live) = &batch.slot.live {
            live.handle().update(|line| line.set(index, end.state()));
            return None;
        }

        Some(look::plain_row(index, batch.total, &path.display().to_string(), end, self.inner.look.color))
    }

    /// A Ctrl-C while the batch is drawn: the run stops at its next check, and the view keeps drawing until it does.
    fn interrupt(&self) {
        tracing::info!("stopping…");
        self.inner.cancel.cancel();
    }

    /// Freezes the batch's view, leaving its last frame in the scrollback, and prints the blank line after it. Says
    /// whether a view was drawn; where none was, prints nothing.
    pub fn finish_batch(&self) -> bool {
        let drawn = lock(&self.inner.batch).slot.close("batch view");
        if drawn {
            eprintln!();
        }
        drawn
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
        assert_eq!(install.slot.attempts, 0, "no session is attempted off a terminal");
        assert!(install.slot.live.is_none());
    }

    #[test]
    fn an_already_installed_dependency_draws_nothing() {
        let view = View::new(plain(), CancellationToken::new());

        view.on_install()(&report(Dependency::Runtime, Phase::AlreadyInstalled, 1.0));

        assert_eq!(line_text(&view), None);
    }

    #[test]
    fn off_a_terminal_no_session_is_attempted_and_each_image_prints_its_plain_line() {
        let view = View::new(plain(), CancellationToken::new());

        view.start_batch(3);
        view.start_image(2, Path::new("photos/b.jpg"), 2);
        let line = view.end_line(2, Path::new("photos/b.jpg"), &ImageEnd::Failed("corrupt".into()));
        assert!(!view.finish_batch());

        assert_eq!(line.as_deref(), Some("🧨 [2/3] Failed image photos/b.jpg: corrupt"));
        assert_eq!(lock(&view.inner.batch).slot.attempts, 0);
    }

    #[test]
    fn a_refused_session_falls_back_to_coloured_plain_lines() {
        let look = Look { interactive: true, color: true };
        let view = View::new(look, CancellationToken::new());
        {
            let mut batch = lock(&view.inner.batch);
            batch.total = 5;
            batch.slot.start("batch view", || Err(std::io::Error::from(std::io::ErrorKind::Unsupported)));
        }

        view.start_image(1, Path::new("a.jpg"), 1);
        let line = view.end_line(1, Path::new("a.jpg"), &ImageEnd::Enhanced);

        assert_eq!(line, Some(look::plain_row(1, 5, "a.jpg", &ImageEnd::Enhanced, true)));
        assert!(!view.finish_batch());
    }

    fn row(index: usize) -> Row {
        Row { index, path: format!("{index}.jpg"), state: RowState::Enhanced }
    }

    /// Each row of `buf`, its cells joined and its trailing blanks dropped.
    fn rows(buf: &Buffer) -> Vec<String> {
        let area = buf.area;
        (area.top()..area.bottom())
            .map(|y| {
                let text: String = (area.left()..area.right()).map(|x| buf[(x, y)].symbol()).collect();
                text.trim_end().to_string()
            })
            .collect()
    }

    fn draw(line: &mut BatchLine, height: u16) -> Vec<String> {
        let area = Rect::new(0, 0, 60, height);
        let mut buf = Buffer::empty(area);
        line.render(area, &mut buf);
        rows(&buf)
    }

    #[test]
    fn the_view_is_two_rows_taller_than_its_list() {
        let heights = [1, 3, 5, 8].map(|total| BatchLine::new(total, false, false).height());

        assert_eq!(heights, [3, 5, 7, 7]);
    }

    #[test]
    fn under_a_dependency_line_the_view_starts_with_a_blank_row() {
        let mut line = BatchLine::new(1, true, false);
        let mut bar = AnimatedProgress::new("Enhancing");
        bar.counter = Some((0, 1));
        line.bar = Some(bar);
        line.push(Row { index: 1, path: "a.jpg".into(), state: RowState::Enhancing });

        assert_eq!(line.height(), 4);
        let drawn = draw(&mut line, 4);
        assert_eq!(drawn[0], "");
        assert!(drawn[1].starts_with("Enhancing"), "{drawn:?}");
        assert_eq!(drawn[2], "");
        assert!(drawn[3].contains("[1/1] Enhancing image a.jpg"), "{drawn:?}");
    }

    #[test]
    fn the_gap_follows_whether_the_dependency_line_was_drawn() {
        let view = View::new(plain(), CancellationToken::new());
        view.on_install()(&report(Dependency::Runtime, Phase::Downloading, 0.5));
        view.finish_install();

        assert!(!lock(&view.inner.install).drawn, "off a terminal, nothing was drawn");
    }

    #[test]
    fn a_sixth_row_scrolls_the_oldest_away() {
        let mut line = BatchLine::new(8, false, false);
        for index in 1..=6 {
            line.push(row(index));
        }

        let drawn = draw(&mut line, 7);

        assert_eq!(drawn[0], "", "no bar");
        assert_eq!(drawn[1], "");
        assert!(drawn[2].contains("[2/8] Enhanced image 2.jpg"), "{drawn:?}");
        assert!(drawn[6].contains("[6/8] Enhanced image 6.jpg"), "{drawn:?}");
        assert_eq!(line.rows.len(), LIST_ROWS);
    }

    #[test]
    fn a_short_terminal_shows_the_newest_rows() {
        let mut line = BatchLine::new(8, false, false);
        for index in 1..=4 {
            line.push(row(index));
        }
        line.set(4, RowState::Failed);

        let drawn = draw(&mut line, 4);

        assert_eq!(drawn.len(), 4);
        assert!(drawn[2].contains("[3/8] Enhanced image 3.jpg"), "{drawn:?}");
        assert!(drawn[3].contains("[4/8] Failed image 4.jpg"), "{drawn:?}");
    }

    #[test]
    fn the_bar_is_drawn_over_the_list_and_settles_the_line() {
        let mut line = BatchLine::new(1, false, false);
        assert!(line.settled(), "no bar is settled");

        let mut bar = AnimatedProgress::new("Enhancing");
        bar.counter = Some((0, 2));
        line.bar = Some(bar);
        line.push(Row { index: 1, path: "a.jpg".into(), state: RowState::Enhancing });
        let drawn = draw(&mut line, 3);

        assert!(drawn[0].starts_with("Enhancing"), "{drawn:?}");
        assert!(drawn[0].contains("[0/2]"), "{drawn:?}");
        assert_eq!(drawn[1], "");
        assert!(drawn[2].contains("[1/1] Enhancing image a.jpg"), "{drawn:?}");
    }

    /// A stand-in for a drawn session.
    struct Drawn;

    impl Session for Drawn {
        fn finish(self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_terminal_that_refused_a_session_is_not_asked_again() {
        let mut slot = Slot::<Drawn>::default();

        for _ in 0..2 {
            slot.start("progress bar", || Err(std::io::Error::from(std::io::ErrorKind::Unsupported)));
            assert!(!slot.finish("progress bar"));
        }

        assert_eq!(slot.attempts, 1);
    }

    #[test]
    fn a_closed_slot_starts_nothing_more() {
        let mut slot = Slot::<Drawn>::default();
        slot.start("dependency line", || Ok(Drawn));

        slot.close("dependency line");
        slot.start("dependency line", || Ok(Drawn));

        assert_eq!(slot.attempts, 1);
        assert!(slot.live.is_none());
    }

    #[test]
    fn a_ctrl_c_on_the_batch_cancels_the_run() {
        let cancel = CancellationToken::new();
        let view = View::new(plain(), cancel.clone());

        view.interrupt();

        assert!(cancel.is_cancelled());
    }

    #[test]
    fn finishing_without_a_session_prints_nothing_and_says_so() {
        let view = View::new(plain(), CancellationToken::new());

        assert!(!lock(&view.inner.batch).slot.finish("batch view"));
        assert!(!lock(&view.inner.install).slot.finish("dependency line"));
    }
}
