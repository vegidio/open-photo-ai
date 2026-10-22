//! What the terminal shows while a run works: the `💾` dependency line while initializing, then one view for the whole
//! batch, its bar over a list of the most recent images.
//!
//! ```text
//! initialize ──► [Live<DependencyLine>]  started on the first Downloading/Extracting report; frozen by finish_install
//!   (run prints the 🧠 processor line and a blank line here, in every mode)
//! the batch  ──► [Live<BatchLine>]       started by start_batch; finish_batch freezes it and prints a blank line
//!
//!   💾 Downloading dependency ONNX Runtime... ⣾ 34% Complete    <- the spinner turns on its own; ✓ at 100%
//!
//!   🧠 Running inference using the processor Auto; quality HD
//!
//!   Enhancing [6/8]  ██████████████████████░░░░░   80.2%   ETA 1m12s   <- the batch's bar: (ended + current) / N
//!                                                           <- blank
//!   🖼️ [100%] Enhancements applied to       photos/f.jpg   <- the list: the five most recent images, paths in
//!   🖼️ [ 42%] Applying Upscale to           photos/g.jpg      one column; start_image pushes a row, the reports
//!                                                             move it, end_image settles it
//! ```
//!
//! Each image's progress is one [`ImageProgress`], kept whether a view is drawn or not, so the live row and the plain
//! line read the same percentage and words.
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
use std::time::{Duration, Instant};

use opai::{CancellationToken, Family, InferenceProgress, OnInference, OnProgress, Phase, Progress, Stage};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;
use rust_sak::tui::{AnimatedProgress, Live, LiveLine, Spinner, Target};
use tokio::sync::Notify;

use crate::look::{self, Activity, ImageEnd, Look, RowState};

/// How many images the list shows: the most recent.
const LIST_ROWS: usize = 5;

/// The most an image adds to the batch's bar before it is written, as its row holds at `99%`.
const UNWRITTEN_CAP: f64 = 0.99;

/// How much of the batch must have run before the bar estimates the time left. Below it, the rate says too little and
/// the bar reads `ETA --`.
const ETA_FROM: f64 = 0.01;

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
    /// The image being worked on, drawn or not. Wherever both are held, it is locked before `batch`.
    image: Mutex<Option<ImageProgress>>,
    batch: Mutex<Batch>,
}

/// The batch's view: how many images ended, and its session once one was started.
#[derive(Default)]
struct Batch {
    /// The images written or failed so far, the `n` of the bar's `[n/N]`.
    ended: usize,
    slot: Slot<Live<BatchLine>>,
}

/// What the batch's session draws: the batch's bar, a blank row, then the most recent images.
struct BatchLine {
    bar: AnimatedProgress,
    /// At most [`LIST_ROWS`], the most recent last.
    rows: VecDeque<ImageProgress>,
    /// The number of images in the run, which sets how many rows the list takes.
    total: usize,
    color: bool,
    /// When the batch started, which the estimate of the time left is measured from.
    started: Instant,
}

/// One image's progress, as its row reads it.
#[derive(Debug, Clone, PartialEq)]
struct ImageProgress {
    /// Its place in the run, from one. Finds its row; it is not printed.
    index: usize,
    /// Its path as it was given or found.
    path: String,
    /// How far its run has got, never decreasing.
    fraction: f64,
    state: RowState,
    /// The families whose operation the run cache served, each once. A face detection is not one of them: autopilot's
    /// analysis warms it even on a first run.
    cached: Vec<Family>,
    /// The enhancements it received, in words, once it is written under `--verbose`.
    applied: Option<String>,
}

impl ImageProgress {
    fn new(index: usize, path: &Path) -> Self {
        Self {
            index,
            path: path.display().to_string(),
            fraction: 0.0,
            state: RowState::Working(Activity::Preparing),
            cached: Vec::new(),
            applied: None,
        }
    }

    /// Moves to what `report` says the run is doing, never backwards. A report after the image ended changes nothing,
    /// and a stage this crate does not know keeps the words the row had.
    fn report(&mut self, report: &InferenceProgress) {
        if !matches!(self.state, RowState::Working(_)) {
            return;
        }

        self.fraction = self.fraction.max(report.chain_fraction);
        let family = report.subject.family();
        if report.stage == Stage::Cached && family != Family::Detection && !self.cached.contains(&family) {
            self.cached.push(family);
        }
        match report.stage {
            Stage::Installing(_) => self.state = RowState::Working(Activity::Downloading(family)),
            Stage::Running | Stage::Cached => self.state = RowState::Working(Activity::Applying(family)),
            _ => {}
        }
    }

    /// The percentage its row shows: held at `99` until written.
    fn percent(&self) -> u8 {
        look::image_percent(self.fraction, self.state == RowState::Applied)
    }

    /// What it adds to the batch's bar while it is worked on: the same capped value its row shows.
    fn contribution(&self) -> f64 {
        self.fraction.min(UNWRITTEN_CAP)
    }

    fn row(&self, color: bool) -> Line<'static> {
        look::with_applied(look::row(self.percent(), &self.path, self.state, color), self.applied.as_deref())
    }
}

/// What ends an image's list of enhancements when `cached` of its `ran` were served from the run cache: ` (cached)`
/// for all of them, ` (<k> of <n> cached)` for some, and nothing for none.
pub fn cache_marker(cached: usize, ran: usize) -> String {
    let cached = cached.min(ran);
    if cached == 0 {
        String::new()
    } else if cached == ran {
        " (cached)".to_owned()
    } else {
        format!(" ({cached} of {ran} cached)")
    }
}

/// Where the batch's bar stands with `ended` of `total` images ended and the current image at `current`: each ended
/// image counts whole, and the current one as far as its row has got, below `1` until it is written. So the bar never
/// reads `1` before the last image has ended, and it does not step back when one image ends and the next starts.
#[allow(clippy::cast_precision_loss, reason = "image counts are far below 2^52")]
fn batch_fraction(ended: usize, current: Option<f64>, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }

    let current = current.map_or(0.0, |fraction| fraction.clamp(0.0, UNWRITTEN_CAP));
    ((ended as f64 + current) / total as f64).min(1.0)
}

/// The time the batch has left, at the rate it has run so far: `elapsed` for `fraction` of it. `None`, read as
/// `ETA --`, until [`ETA_FROM`] of it has run, and zero once it is all done.
fn time_left(elapsed: Duration, fraction: f64) -> Option<Duration> {
    if fraction >= 1.0 {
        return Some(Duration::ZERO);
    }
    if fraction < ETA_FROM {
        return None;
    }

    Some(elapsed.mul_f64((1.0 - fraction) / fraction))
}

impl BatchLine {
    fn new(total: usize, color: bool) -> Self {
        let mut bar = AnimatedProgress::new("Enhancing");
        bar.counter = Some((0, total));
        bar.color = color;

        Self { bar, rows: VecDeque::with_capacity(LIST_ROWS), total, color, started: Instant::now() }
    }

    /// Adds `image` as the most recent, dropping the oldest once the list is full.
    fn push(&mut self, image: ImageProgress) {
        if self.rows.len() == LIST_ROWS {
            self.rows.pop_front();
        }
        self.rows.push_back(image);
    }

    /// Redraws the row of `image`, if it is still listed.
    fn set(&mut self, image: &ImageProgress) {
        if let Some(row) = self.rows.iter_mut().rev().find(|row| row.index == image.index) {
            row.clone_from(image);
        }
    }

    /// Moves the bar to `ended` images ended and the current one at `current`.
    fn show(&mut self, ended: usize, current: Option<f64>) {
        self.bar.counter = Some((ended, self.total));
        self.bar.fraction = batch_fraction(ended, current, self.total);
    }
}

impl LiveLine for BatchLine {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let [bar, _, list] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).areas(area);
        // Every frame, not every report, so the estimate keeps counting down while a long tile loop reports nothing.
        self.bar.eta = Some(time_left(self.started.elapsed(), self.bar.fraction));
        self.bar.render(bar, buf);

        // In a terminal too short for the whole list, the most recent rows that fit.
        let shown = self.rows.len().saturating_sub(usize::from(list.height));
        for (row, area) in self.rows.iter().skip(shown).zip(list.rows()) {
            row.row(self.color).render(area, buf);
        }
    }

    fn settled(&self) -> bool {
        self.bar.settled()
    }

    fn snap(&mut self) {
        self.bar.snap();
    }

    #[allow(clippy::cast_possible_truncation, reason = "at most 2 + LIST_ROWS")]
    fn height(&self) -> u16 {
        (2 + self.total.min(LIST_ROWS)) as u16
    }
}

/// What the dependency line's session draws: the dependency being installed, how far it has got, and a spinner that
/// turns on its own clock, started once with the line so it does not jump back when the next dependency starts.
struct DependencyLine {
    name: String,
    fraction: f64,
    spinner: Spinner,
    color: bool,
}

impl DependencyLine {
    fn new(name: String, fraction: f64, color: bool) -> Self {
        let (r, g, b) = look::MAGENTA;
        let mut spinner = Spinner::new();
        spinner.style = Style::new().fg(Color::Rgb(r, g, b));
        spinner.color = color;

        Self { name, fraction, spinner, color }
    }

    /// The line as it reads now: the spinner's current frame, or `✓` while the install reads `100`.
    fn line(&self) -> Line<'static> {
        let mark = if look::whole_percent(self.fraction) == 100 {
            look::done_mark(self.color)
        } else {
            self.spinner.span()
        };

        look::dependency_line(&self.name, self.fraction, mark, self.color)
    }
}

impl LiveLine for DependencyLine {
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.line().render(area, buf);
    }
}

/// The dependency line's session, once one was started.
#[derive(Default)]
struct Install {
    slot: Slot<Live<DependencyLine>>,
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

    /// Settles the session, leaving its last frame in the scrollback, says whether one was drawn, and never starts
    /// another.
    fn close(&mut self, what: &str) -> bool {
        self.closed = true;
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
    /// A view drawing as `look` says, whose batch cancels `cancel` on a Ctrl-C.
    pub fn new(look: Look, cancel: CancellationToken) -> Self {
        Self {
            inner: Arc::new(Inner {
                look,
                cancel,
                install_interrupt: Arc::new(Notify::new()),
                install: Mutex::default(),
                image: Mutex::default(),
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

        if !self.inner.look.interactive {
            return;
        }

        let name = progress.dependency.as_str();
        let fraction = progress.fraction;
        let mut install = lock(&self.inner.install);

        match &install.slot.live {
            Some(live) => live.handle().update(|line| {
                // Copied only when the next dependency replaces the last, not on every report of the same one.
                if line.name != name {
                    name.clone_into(&mut line.name);
                }
                line.fraction = fraction;
            }),
            None => {
                let line = DependencyLine::new(name.to_owned(), fraction, self.inner.look.color);
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
        lock(&self.inner.install).slot.close("dependency line");
    }

    /// Starts the batch's view for `total` images on a terminal, its bar at `Enhancing [0/total]`. A Ctrl-C while it is
    /// drawn cancels the run. Off a terminal, or where the terminal refuses the session, prints nothing.
    pub fn start_batch(&self, total: usize) {
        let mut batch = lock(&self.inner.batch);
        batch.ended = 0;

        if !self.inner.look.interactive {
            return;
        }

        let line = BatchLine::new(total, self.inner.look.color);
        let view = self.clone();
        batch.slot.start("batch view", || Live::start(Target::Stderr, line, move || view.interrupt()));
    }

    /// Makes the image at `index` (from one) the current one, at `[  0%] Preparing`, and lists it. The bar is left
    /// where it is: the image adds nothing to it yet.
    pub fn start_image(&self, index: usize, path: &Path) {
        let image = ImageProgress::new(index, path);
        let mut current = lock(&self.inner.image);
        *current = Some(image.clone());

        let batch = lock(&self.inner.batch);
        if let Some(live) = &batch.slot.live {
            live.handle().update(|line| line.push(image));
        }
    }

    /// Marks the image at `index` as being analysed by autopilot: its row reads `Choosing enhancements for`, at the
    /// percentage it already had. The bar is left where it is.
    pub fn choosing(&self, index: usize) {
        self.set_activity(index, Activity::Choosing);
    }

    /// Ends the analysis of the image at `index`: its row reads `Preparing to enhance` again until its chain first
    /// reports, which for an empty chain is never.
    pub fn chosen(&self, index: usize) {
        self.set_activity(index, Activity::Preparing);
    }

    /// Sets the words of the current image's row, if it is the image at `index` and still being worked on.
    fn set_activity(&self, index: usize, activity: Activity) {
        let mut current = lock(&self.inner.image);
        let Some(image) = current.as_mut().filter(|image| image.index == index) else {
            return;
        };
        if !matches!(image.state, RowState::Working(_)) {
            return;
        }
        image.state = RowState::Working(activity);

        let batch = lock(&self.inner.batch);
        if let Some(live) = &batch.slot.live {
            live.handle().update(|line| line.set(image));
        }
    }

    /// The inference callback the current image's row and the batch's bar are moved by, meant for
    /// [`opai::ProcessOptions::on_progress`].
    pub fn on_inference(&self) -> OnInference {
        let view = self.clone();
        Arc::new(move |report: &InferenceProgress| view.inference_report(report))
    }

    fn inference_report(&self, report: &InferenceProgress) {
        let mut current = lock(&self.inner.image);
        let Some(image) = current.as_mut() else {
            return;
        };
        image.report(report);

        let batch = lock(&self.inner.batch);
        if let Some(live) = &batch.slot.live {
            let ended = batch.ended;
            live.handle().update(|line| {
                line.set(image);
                line.show(ended, Some(image.contribution()));
            });
        }
    }

    /// How many of the current image's enhancements the run cache served so far, if it is the image at `index`.
    pub fn cached(&self, index: usize) -> usize {
        lock(&self.inner.image)
            .as_ref()
            .filter(|image| image.index == index)
            .map_or(0, |image| image.cached.len())
    }

    /// Settles the row of the image at `index` as `end` says, or, where no view is drawn, prints its plain line.
    pub fn end_image(&self, index: usize, path: &Path, end: &ImageEnd) {
        if let Some(line) = self.end_line(index, path, end) {
            eprintln!("{line}");
        }
    }

    /// What [`end_image`](Self::end_image) prints: nothing while the view is drawn, and the image's plain line, at the
    /// percentage it reached, otherwise, coloured as the look says.
    ///
    /// A written or failed image counts as ended on the bar. A stopped one ends the run, so the bar is left as it was.
    pub(crate) fn end_line(&self, index: usize, path: &Path, end: &ImageEnd) -> Option<String> {
        let mut current = lock(&self.inner.image);
        let mut image = current
            .take()
            .filter(|image| image.index == index)
            .unwrap_or_else(|| ImageProgress::new(index, path));
        image.state = end.state();
        if let ImageEnd::Enhanced(applied) = end {
            image.applied.clone_from(applied);
        }

        let stopped = matches!(end, ImageEnd::Stopped);
        let mut batch = lock(&self.inner.batch);
        if !stopped {
            batch.ended += 1;
        }

        if let Some(live) = &batch.slot.live {
            let ended = batch.ended;
            live.handle().update(|line| {
                line.set(&image);
                if !stopped {
                    line.show(ended, None);
                }
            });
            return None;
        }

        Some(look::plain_row(image.percent(), &image.path, end, self.inner.look.color))
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
pub(crate) mod tests {
    use super::*;
    use opai::{
        Dependency, Detection, FaceRecovery, Faces, Family, FloatPrecision, Operation, Scale, Subject, Upscale,
        UpscaleVariant,
    };

    fn plain() -> Look {
        Look { interactive: false, color: false }
    }

    fn install(dependency: Dependency, phase: Phase, fraction: f64) -> Progress {
        Progress { dependency, phase, bytes: 0, total: None, fraction }
    }

    /// A report on `subject` at `stage`, the chain at `chain_fraction`.
    pub(crate) fn inference(subject: Subject, stage: Stage, chain_fraction: f64) -> InferenceProgress {
        InferenceProgress { subject: Arc::new(subject), stage, operation_fraction: 0.5, chain_fraction }
    }

    pub(crate) fn upscale() -> Subject {
        let scale = Scale::new(2.0).expect("a valid scale");
        Subject::Enhancement(Operation::Upscale(Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp32), scale)))
    }

    fn face_recovery() -> Subject {
        Subject::Enhancement(FaceRecovery::santorini(FloatPrecision::Fp32, Faces::empty()))
    }

    fn installing() -> Stage {
        Stage::Installing(install(Dependency::Runtime, Phase::Downloading, 0.1))
    }

    #[test]
    fn the_dependency_line_draws_nothing_off_a_terminal() {
        let view = View::new(plain(), CancellationToken::new());
        let on_install = view.on_install();

        for progress in [
            install(Dependency::Runtime, Phase::Downloading, 0.34),
            install(Dependency::Runtime, Phase::Extracting, 1.0),
            install(Dependency::Cuda, Phase::Downloading, 0.05),
            install(Dependency::Runtime, Phase::AlreadyInstalled, 1.0),
        ] {
            on_install(&progress);
        }

        let install = lock(&view.inner.install);
        assert_eq!(install.slot.attempts, 0, "no session is attempted off a terminal");
        assert!(install.slot.live.is_none());
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

    fn draw(line: &mut impl LiveLine, height: u16) -> Vec<String> {
        let area = Rect::new(0, 0, 80, height);
        let mut buf = Buffer::empty(area);
        line.render(area, &mut buf);
        rows(&buf)
    }

    #[test]
    fn the_dependency_line_shows_a_check_at_100_and_a_spinner_below_it() {
        let mut done = DependencyLine::new("ONNX Runtime".into(), 1.0, true);
        let drawn = draw(&mut done, 1);
        assert!(drawn[0].ends_with(" Downloading dependency ONNX Runtime... ✓ 100% Complete"), "{drawn:?}");

        let mut working = DependencyLine::new("ONNX Runtime".into(), 0.999, true);
        let text = working.line().spans.iter().map(|span| span.content.to_string()).collect::<String>();
        let mark = text
            .strip_prefix("💾 Downloading dependency ONNX Runtime... ")
            .and_then(|rest| rest.strip_suffix(" 99% Complete"))
            .expect("the line around the mark");
        assert!(mark.chars().all(|glyph| ('\u{2800}'..='\u{28ff}').contains(&glyph)), "{mark:?}");
        assert_eq!(mark.chars().count(), 1);
        let (r, g, b) = look::MAGENTA;
        assert_eq!(working.line().spans[3].style, Style::new().fg(Color::Rgb(r, g, b)));
        assert!(draw(&mut working, 1)[0].ends_with("99% Complete"));
    }

    #[test]
    fn an_image_follows_its_reports_and_never_moves_backwards() {
        let mut image = ImageProgress::new(1, Path::new("a.jpg"));
        assert_eq!(image.state, RowState::Working(Activity::Preparing));

        image.report(&inference(upscale(), installing(), 0.12));
        assert_eq!(image.state, RowState::Working(Activity::Downloading(Family::Upscale)));
        assert_eq!(image.percent(), 12);

        image.report(&inference(face_recovery(), Stage::Running, 0.3));
        assert_eq!(image.state, RowState::Working(Activity::Applying(Family::FaceRecovery)));

        image.report(&inference(upscale(), Stage::Cached, 0.2));
        assert_eq!(image.state, RowState::Working(Activity::Applying(Family::Upscale)));
        assert_eq!(image.percent(), 30, "a late report does not move it back");

        image.report(&inference(upscale(), Stage::Running, 1.0));
        assert_eq!(image.percent(), 99, "held until written");
        assert_eq!(image.contribution(), UNWRITTEN_CAP);

        image.state = RowState::Applied;
        assert_eq!(image.percent(), 100);
        image.report(&inference(upscale(), installing(), 1.0));
        assert_eq!(image.state, RowState::Applied, "an ended image ignores reports");
    }

    #[test]
    fn a_detection_reads_as_face_recovery() {
        let mut image = ImageProgress::new(1, Path::new("a.jpg"));
        image.report(&inference(Subject::Analysis(Detection::for_face_recovery()), Stage::Running, 0.03));

        assert_eq!(image.row(false).to_string(), "🖼️ [  3%] Applying Face Recovery to     a.jpg");
    }

    #[test]
    fn off_a_terminal_each_image_prints_its_plain_line_at_the_percentage_it_reached() {
        let view = View::new(plain(), CancellationToken::new());
        let on_inference = view.on_inference();
        let [a, b, c] = ["photos/a.jpg", "photos/b.jpg", "photos/c.jpg"].map(Path::new);

        view.start_batch(3);

        view.start_image(1, a);
        on_inference(&inference(upscale(), Stage::Running, 1.0));
        let first = view.end_line(1, a, &ImageEnd::Enhanced(None));

        view.start_image(2, b);
        on_inference(&inference(upscale(), Stage::Running, 0.42));
        let second = view.end_line(2, b, &ImageEnd::Failed("corrupt".into()));

        // An empty chain reports nothing and goes straight to written.
        view.start_image(3, c);
        let third = view.end_line(3, c, &ImageEnd::Enhanced(None));

        assert!(!view.finish_batch());
        assert_eq!(first.as_deref(), Some("🖼️ [100%] Enhancements applied to       photos/a.jpg"));
        assert_eq!(second.as_deref(), Some("🧨 [ 42%] Failed to enhance             photos/b.jpg: corrupt"));
        assert_eq!(third.as_deref(), Some("🖼️ [100%] Enhancements applied to       photos/c.jpg"));

        let batch = lock(&view.inner.batch);
        assert_eq!(batch.slot.attempts, 0);
        assert_eq!(batch.ended, 3, "written and failed images both count as ended");
    }

    fn current(view: &View) -> Option<ImageProgress> {
        lock(&view.inner.image).clone()
    }

    #[test]
    fn a_report_after_choosing_moves_the_row_on() {
        let view = View::new(plain(), CancellationToken::new());
        view.start_batch(1);
        view.start_image(1, Path::new("a.jpg"));

        view.choosing(1);
        let choosing = current(&view).expect("an image is current");
        assert_eq!(choosing.state, RowState::Working(Activity::Choosing));
        assert_eq!(choosing.percent(), 0);

        view.on_inference()(&inference(upscale(), Stage::Running, 0.3));
        let applying = current(&view).expect("an image is current");
        assert_eq!(applying.state, RowState::Working(Activity::Applying(Family::Upscale)));
        assert_eq!(applying.percent(), 30);
    }

    #[test]
    fn choosing_another_image_changes_nothing() {
        let view = View::new(plain(), CancellationToken::new());
        view.start_batch(2);
        view.start_image(2, Path::new("b.jpg"));

        view.choosing(1);

        assert_eq!(current(&view).map(|image| image.state), Some(RowState::Working(Activity::Preparing)));
    }

    #[test]
    fn an_image_ended_straight_from_choosing_reads_applied() {
        let view = View::new(plain(), CancellationToken::new());
        view.start_batch(1);
        view.start_image(1, Path::new("a.jpg"));
        view.choosing(1);

        let line = view.end_line(1, Path::new("a.jpg"), &ImageEnd::Enhanced(None));

        assert_eq!(line.as_deref(), Some("🖼️ [100%] Enhancements applied to       a.jpg"));
    }

    #[test]
    fn an_image_given_nothing_to_run_returns_to_preparing_and_ends_applied() {
        let view = View::new(plain(), CancellationToken::new());
        view.start_batch(1);
        view.start_image(1, Path::new("a.jpg"));
        view.choosing(1);
        view.chosen(1);
        assert_eq!(current(&view).map(|image| image.state), Some(RowState::Working(Activity::Preparing)));

        // Autopilot suggested nothing: an empty chain reports nothing and goes straight to written.
        let line = view.end_line(1, Path::new("a.jpg"), &ImageEnd::Enhanced(None));

        assert_eq!(line.as_deref(), Some("🖼️ [100%] Enhancements applied to       a.jpg"));
    }

    #[test]
    fn a_stopped_image_keeps_its_percentage_and_does_not_count_as_ended() {
        let view = View::new(plain(), CancellationToken::new());
        view.start_batch(2);
        view.start_image(1, Path::new("a.jpg"));
        view.on_inference()(&inference(upscale(), Stage::Running, 0.07));

        let line = view.end_line(1, Path::new("a.jpg"), &ImageEnd::Stopped);

        assert_eq!(line.as_deref(), Some("🧨 [  7%] Stopped enhancing             a.jpg"));
        assert_eq!(lock(&view.inner.batch).ended, 0);
    }

    #[test]
    fn a_refused_session_falls_back_to_coloured_plain_lines() {
        let look = Look { interactive: true, color: true };
        let view = View::new(look, CancellationToken::new());
        {
            let mut batch = lock(&view.inner.batch);
            batch.slot.start("batch view", || Err(std::io::Error::from(std::io::ErrorKind::Unsupported)));
        }

        view.start_image(1, Path::new("a.jpg"));
        let line = view.end_line(1, Path::new("a.jpg"), &ImageEnd::Enhanced(None));

        assert_eq!(line, Some(look::plain_row(100, "a.jpg", &ImageEnd::Enhanced(None), true)));
        assert!(!view.finish_batch());
    }

    #[test]
    fn the_batch_fraction_counts_ended_images_and_the_current_ones_progress() {
        assert!((batch_fraction(2, Some(0.5), 4) - 0.625).abs() < 1e-12);
        assert!((batch_fraction(1, None, 2) - 0.5).abs() < 1e-12);
        assert!((batch_fraction(0, None, 0)).abs() < 1e-12);
    }

    #[test]
    fn the_batch_fraction_reaches_1_only_once_the_last_image_ended() {
        assert!(batch_fraction(3, Some(1.0), 4) < 1.0);
        assert!(batch_fraction(0, Some(1.0), 1) < 1.0);
        assert!((batch_fraction(4, None, 4) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_batch_fraction_does_not_step_back_between_images() {
        let before = batch_fraction(1, Some(UNWRITTEN_CAP), 3);
        let ended = batch_fraction(2, None, 3);
        let next = batch_fraction(2, Some(0.0), 3);

        assert!(before <= ended && ended <= next, "{before} {ended} {next}");
    }

    fn ended(index: usize) -> ImageProgress {
        ImageProgress {
            state: RowState::Applied,
            fraction: 1.0,
            ..ImageProgress::new(index, Path::new(&format!("{index}.jpg")))
        }
    }

    #[test]
    fn the_time_left_follows_the_rate_so_far() {
        let elapsed = Duration::from_secs(30);

        assert_eq!(time_left(elapsed, 0.0), None, "nothing has run");
        assert_eq!(time_left(elapsed, ETA_FROM / 2.0), None, "too little has run");
        assert_eq!(time_left(elapsed, 0.25), Some(Duration::from_secs(90)));
        assert_eq!(time_left(elapsed, 0.5), Some(elapsed));
        assert_eq!(time_left(elapsed, 1.0), Some(Duration::ZERO));
    }

    #[test]
    fn the_bar_reads_eta_unknown_before_anything_has_run() {
        let mut line = BatchLine::new(2, false);

        let drawn = draw(&mut line, 4);

        assert!(drawn[0].ends_with("ETA --"), "{drawn:?}");
    }

    #[test]
    fn the_view_is_two_rows_taller_than_its_list() {
        let heights = [1, 3, 5, 8].map(|total| BatchLine::new(total, false).height());

        assert_eq!(heights, [3, 5, 7, 7]);
    }

    #[test]
    fn the_bar_reads_enhancing_with_the_ended_count_over_the_list() {
        let mut line = BatchLine::new(1, false);
        line.push(ImageProgress::new(1, Path::new("a.jpg")));

        let drawn = draw(&mut line, 3);

        assert!(drawn[0].starts_with("Enhancing"), "{drawn:?}");
        assert!(drawn[0].contains("[0/1]"), "{drawn:?}");
        assert_eq!(drawn[1], "");
        assert!(drawn[2].ends_with(" [  0%] Preparing to enhance          a.jpg"), "{drawn:?}");
    }

    #[test]
    fn the_bar_moves_with_the_ended_count_and_the_current_image() {
        let mut line = BatchLine::new(4, false);

        line.show(2, Some(0.5));

        assert_eq!(line.bar.counter, Some((2, 4)));
        assert!((line.bar.fraction - 0.625).abs() < 1e-12);
    }

    #[test]
    fn a_row_is_redrawn_from_its_image() {
        let mut line = BatchLine::new(2, false);
        let mut image = ImageProgress::new(2, Path::new("b.jpg"));
        line.push(ended(1));
        line.push(image.clone());

        image.fraction = 0.42;
        image.state = RowState::Failed;
        line.set(&image);

        let drawn = draw(&mut line, 4);
        assert!(drawn[2].ends_with(" [100%] Enhancements applied to       1.jpg"), "{drawn:?}");
        assert!(drawn[3].ends_with(" [ 42%] Failed to enhance             b.jpg"), "{drawn:?}");
    }

    #[test]
    fn a_cached_enhancement_is_counted_once_and_a_cached_detection_not_at_all() {
        let denoise =
            Subject::Enhancement(opai::Denoise::stockholm(FloatPrecision::Fp32, opai::Strength::clamped(1.0)));
        let detection = Subject::Analysis(Detection::for_face_recovery());
        let mut image = ImageProgress::new(1, Path::new("a.jpg"));

        image.report(&inference(detection, Stage::Cached, 0.1));
        image.report(&inference(denoise, Stage::Running, 0.4));
        image.report(&inference(upscale(), Stage::Cached, 0.9));
        image.report(&inference(upscale(), Stage::Cached, 1.0));

        assert_eq!(image.cached, [Family::Upscale]);
        assert_eq!(image.row(false).to_string(), "🖼️ [ 99%] Applying Upscale to           a.jpg", "words unchanged");
    }

    #[test]
    fn the_view_answers_the_current_images_cached_count_only() {
        let view = View::new(plain(), CancellationToken::new());
        let on_inference = view.on_inference();
        view.start_image(1, Path::new("a.jpg"));
        on_inference(&inference(upscale(), Stage::Cached, 1.0));

        assert_eq!(view.cached(1), 1);
        assert_eq!(view.cached(2), 0, "another image");
    }

    #[test]
    fn the_cache_marker_reads_all_some_or_nothing() {
        assert_eq!(cache_marker(0, 3), "");
        assert_eq!(cache_marker(2, 3), " (2 of 3 cached)");
        assert_eq!(cache_marker(3, 3), " (cached)");
        assert_eq!(cache_marker(0, 0), "");
        assert_eq!(cache_marker(4, 3), " (cached)", "never more than ran");
    }

    #[test]
    fn a_written_rows_applied_text_is_drawn_after_its_path_and_cut_at_the_edge() {
        let mut line = BatchLine::new(2, false);
        let mut short = ImageProgress::new(1, Path::new("a.jpg"));
        let mut long = ImageProgress::new(2, Path::new("b.jpg"));
        line.push(short.clone());
        line.push(long.clone());

        short.state = RowState::Applied;
        short.applied = Some("Upscale Tokyo 2x (cached)".to_owned());
        line.set(&short);
        long.state = RowState::Applied;
        long.applied = Some("Denoise Stockholm strength 100, Light Adjust Paris bias 50, Upscale Tokyo 2x".to_owned());
        line.set(&long);

        // The test terminal is 80 columns wide: the long row is cut there, and the view keeps its height.
        let drawn = draw(&mut line, 4);
        assert_eq!(drawn.len(), 4, "{drawn:?}");
        assert!(
            drawn[2].ends_with(" [100%] Enhancements applied to       a.jpg: Upscale Tokyo 2x (cached)"),
            "{drawn:?}"
        );
        assert!(drawn[3].contains("b.jpg: Denoise Stockholm strength 100, "), "{drawn:?}");
        assert!(!drawn[3].contains("Upscale Tokyo"), "{drawn:?}");
    }

    #[test]
    fn end_line_keeps_the_text_for_the_live_row_and_none_leaves_todays_line() {
        let view = View::new(plain(), CancellationToken::new());
        view.start_image(1, Path::new("a.jpg"));

        let line = view.end_line(1, Path::new("a.jpg"), &ImageEnd::Enhanced(None)).expect("a plain line");

        assert_eq!(line, "🖼️ [100%] Enhancements applied to       a.jpg");
    }

    #[test]
    fn a_sixth_row_scrolls_the_oldest_away() {
        let mut line = BatchLine::new(8, false);
        for index in 1..=6 {
            line.push(ended(index));
        }

        let drawn = draw(&mut line, 7);

        assert!(drawn[0].starts_with("Enhancing"), "{drawn:?}");
        assert_eq!(drawn[1], "");
        assert!(drawn[2].ends_with("2.jpg"), "{drawn:?}");
        assert!(drawn[6].ends_with("6.jpg"), "{drawn:?}");
        assert_eq!(line.rows.len(), LIST_ROWS);
    }

    #[test]
    fn a_short_terminal_shows_the_newest_rows() {
        let mut line = BatchLine::new(8, false);
        for index in 1..=4 {
            line.push(ended(index));
        }

        let drawn = draw(&mut line, 4);

        assert_eq!(drawn.len(), 4);
        assert!(drawn[2].ends_with("3.jpg"), "{drawn:?}");
        assert!(drawn[3].ends_with("4.jpg"), "{drawn:?}");
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
            assert!(slot.live.is_none());
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

        assert!(!lock(&view.inner.batch).slot.close("batch view"));
        assert!(!lock(&view.inner.install).slot.close("dependency line"));
    }
}
