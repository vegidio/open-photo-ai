//! What the terminal shows while a run works: one bar per dependency while initializing, one bar per library call
//! while enhancing, and every message printed above them without tearing them.
//!
//! The bars are drawn on stderr, and only when stderr is a terminal: indicatif hides them by itself otherwise, so a
//! log of a piped run holds the messages and no redraw sequences.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use opai::{InferenceProgress, OnInference, OnPlan, OnProgress, Phase, PlannedDependency, Progress, Stage};

/// The resolution of an inference bar: per-mille, so a long run still moves.
const STEPS: u64 = 1000;

/// The terminal's view of a run. Cheap to clone: every clone draws into the same set of bars.
#[derive(Clone)]
pub struct View {
    multi: MultiProgress,
}

impl View {
    /// A view drawing on stderr, or drawing nothing where stderr is not a terminal.
    pub fn new() -> Self {
        Self { multi: MultiProgress::with_draw_target(ProgressDrawTarget::stderr()) }
    }

    /// Prints `line` to stderr, above any bar being drawn.
    ///
    /// `suspend` rather than `MultiProgress::println`, because the latter prints nothing at all when the bars are
    /// hidden, and a piped run still owes the user its messages.
    pub fn say(&self, line: impl Display) {
        self.multi.suspend(|| eprintln!("{line}"));
    }

    /// The two install callbacks [`opai::InitOptions`] takes: the plan lays out one bar per dependency, and each
    /// progress report moves its dependency's bar.
    pub fn install(&self) -> (OnPlan, OnProgress) {
        let bars = Arc::new(Install { multi: self.multi.clone(), bars: Mutex::new(HashMap::new()) });

        let on_plan: OnPlan = {
            let bars = Arc::clone(&bars);
            Arc::new(move |plan: &[PlannedDependency]| {
                for planned in plan {
                    bars.bar(planned.dependency.as_str(), planned.size);
                }
            })
        };

        let on_progress: OnProgress = Arc::new(move |progress: &Progress| bars.report(progress));

        (on_plan, on_progress)
    }

    /// A bar for one library call — a face detection, or the whole chain — and the callback that moves it.
    ///
    /// The bar is drawn on the first report rather than up front, so a call that reports nothing — a chain with no
    /// face recovery asks for no detection — leaves nothing on the screen.
    pub fn inference(&self) -> Run {
        Run { multi: self.multi.clone(), bar: Arc::new(OnceLock::new()) }
    }
}

/// The install bars, one per dependency, keyed by the name the library reports it under.
struct Install {
    multi: MultiProgress,
    bars: Mutex<HashMap<String, ProgressBar>>,
}

impl Install {
    /// The bar for `name`, laid out with `size` bytes to go where it is new.
    fn bar(&self, name: &str, size: u64) -> ProgressBar {
        let mut bars = self.bars.lock().unwrap_or_else(PoisonError::into_inner);

        bars.entry(name.to_string())
            .or_insert_with(|| {
                let bar = self.multi.add(ProgressBar::new(size));
                bar.set_style(style("{prefix:<16} {msg:<10} [{wide_bar}] {bytes:>10} / {total_bytes:<10}"));
                bar.set_prefix(name.to_string());
                bar.set_message("waiting");
                bar
            })
            .clone()
    }

    fn report(&self, progress: &Progress) {
        let bar = self.bar(progress.dependency.as_str(), progress.total.unwrap_or(0));

        if progress.phase == Phase::AlreadyInstalled {
            bar.set_style(style("{prefix:<16} {msg}"));
            bar.finish_with_message("already installed");
            return;
        }

        if progress.fraction >= 1.0 {
            bar.set_style(style("{prefix:<16} {msg}"));
            bar.finish_with_message("installed");
            return;
        }

        bar.set_message(match progress.phase {
            Phase::Downloading => "downloading",
            Phase::Extracting => "expanding",
            _ => "installing",
        });
        if let Some(total) = progress.total {
            bar.set_length(total);
        }
        bar.set_position(progress.bytes);
    }
}

/// One library call's bar, drawn once the call first reports.
#[derive(Clone)]
pub struct Run {
    multi: MultiProgress,
    bar: Arc<OnceLock<ProgressBar>>,
}

impl Run {
    /// The callback a run's options take.
    pub fn on_progress(&self) -> OnInference {
        let run = self.clone();
        Arc::new(move |progress: &InferenceProgress| run.report(progress))
    }

    /// Takes the bar off the screen, where one was drawn.
    pub fn clear(&self) {
        if let Some(bar) = self.bar.get() {
            bar.finish_and_clear();
        }
    }

    fn report(&self, progress: &InferenceProgress) {
        let bar = self.bar.get_or_init(|| {
            let bar = self.multi.add(ProgressBar::new(STEPS));
            bar.set_style(style("{msg:<44} [{wide_bar}] {percent:>3}%"));
            bar
        });

        bar.set_message(format!("{} — {}", progress.subject.display_name(), stage(&progress.stage)));
        bar.set_position((progress.chain_fraction.clamp(0.0, 1.0) * STEPS as f64).round() as u64);
    }
}

/// What an operation is doing, as its bar says it.
fn stage(stage: &Stage) -> String {
    match stage {
        Stage::Installing(install) => match (install.phase, install.total) {
            (Phase::Downloading, Some(total)) => {
                format!("downloading the model {} / {}", HumanBytes(install.bytes), HumanBytes(total))
            }
            (Phase::Downloading, None) => format!("downloading the model {}", HumanBytes(install.bytes)),
            (Phase::Extracting, _) => "expanding the model".to_string(),
            _ => "preparing the model".to_string(),
        },
        Stage::Running => "running".to_string(),
        Stage::Cached => "from cache".to_string(),
        _ => "working".to_string(),
    }
}

/// A bar style from `template`, which is fixed in this file and so always parses.
fn style(template: &str) -> ProgressStyle {
    ProgressStyle::with_template(template)
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=> ")
}
