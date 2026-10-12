//! What the terminal's lines say and how they are coloured: the header, the result lines, the dependency line, and the
//! bar's label and counter. Pure text, so all of it is tested without a terminal.
//!
//! Whether to draw live lines and whether to colour is decided once, in [`Look`], the way MediaSim decides it: a
//! terminal, and `NO_COLOR` unset or empty.

use std::ffi::OsStr;
use std::fmt::Display;
use std::io::IsTerminal;
use std::path::Path;

use opai::{Family, InferenceProgress, Stage};
use ratatui::crossterm::style::{Color as TermColor, Stylize};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// The input path in the header.
pub const GREEN: (u8, u8, u8) = (0x00, 0xc2, 0x02);
/// The dependency's name and the success line.
pub const YELLOW: (u8, u8, u8) = (0xe5, 0xdb, 0x08);
/// The failure line.
pub const RED: (u8, u8, u8) = (0xf4, 0x43, 0x36);

/// The bar's label before the run has reported anything, and for a stage this crate does not know.
const ENHANCING: &str = "Enhancing";
/// The bar's label while an operation's model is being fetched. Unlike the GUI's chip, it carries no percentage.
const DOWNLOADING: &str = "Downloading";

/// The columns the bar's label is padded to, so the bar does not move when the label changes: the widest label
/// [`label`] returns.
pub const LABEL_WIDTH: usize = "Face Recovery".len();

/// Whether a stream may be coloured: it must be a terminal, and `NO_COLOR` must be unset or empty.
pub fn use_color(is_terminal: bool, no_color: Option<&OsStr>) -> bool {
    is_terminal && no_color.is_none_or(OsStr::is_empty)
}

/// How a run draws on stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    /// Live lines are drawn and blank lines frame the bar. Otherwise only the header and the result line are printed.
    pub interactive: bool,
    /// The lines are coloured.
    pub color: bool,
}

impl Look {
    /// The look for a stderr that `is_terminal`, under a `NO_COLOR` of `no_color`.
    pub fn new(is_terminal: bool, no_color: Option<&OsStr>) -> Self {
        Self { interactive: is_terminal, color: use_color(is_terminal, no_color) }
    }

    /// The look for this process's stderr and environment.
    pub fn for_stderr() -> Self {
        Self::new(std::io::stderr().is_terminal(), std::env::var_os("NO_COLOR").as_deref())
    }
}

/// `🖼️ Applying enhancements to the image <input>`, with the path in green.
pub fn header(input: &Path, color: bool) -> String {
    format!("🖼️ Applying enhancements to the image {}", paint(input.display(), GREEN, color))
}

/// `✨ Image enhanced successfully; file saved at <output>`, all in bold yellow.
pub fn success(output: &Path, color: bool) -> String {
    paint_bold(format!("✨ Image enhanced successfully; file saved at {}", output.display()), YELLOW, color)
}

/// `🧨 <error>`, all in bold red.
pub fn failure(error: impl Display, color: bool) -> String {
    paint_bold(format!("🧨 {error}"), RED, color)
}

/// `💾 Downloading dependency <name>   <percent>%`, with the name in yellow and the percentage in bold white.
///
/// The percentage is `fraction` floored to a whole number, and `100` only once `fraction` is exactly `1.0`, which is
/// the library's last report for an install.
pub fn dependency_line(name: &str, fraction: f64, color: bool) -> Line<'static> {
    let (name_style, percent_style) = if color {
        let (r, g, b) = YELLOW;
        (Style::new().fg(Color::Rgb(r, g, b)), Style::new().fg(Color::White).add_modifier(Modifier::BOLD))
    } else {
        (Style::new(), Style::new())
    };

    Line::from(vec![
        Span::raw("💾 Downloading dependency "),
        Span::styled(name.to_string(), name_style),
        Span::raw("   "),
        Span::styled(format!("{}%", whole_percent(fraction)), percent_style),
    ])
}

/// `fraction` as a whole percentage, floored, and below `100` until `fraction` reaches `1.0`. The small nudge keeps a
/// fraction such as `0.29`, stored as `0.28999…`, from showing as `28`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..=100 first")]
fn whole_percent(fraction: f64) -> u8 {
    let fraction = fraction.clamp(0.0, 1.0);
    let percent = (fraction * 100.0 + 1e-6).floor() as u8;

    if fraction < 1.0 { percent.min(99) } else { 100 }
}

/// The bar's label for the latest `report`, or for none yet, in the GUI's English words.
///
/// A face detection reads as the face recovery it serves, as it does in the GUI.
pub fn label(report: Option<&InferenceProgress>) -> &'static str {
    let Some(report) = report else {
        return ENHANCING;
    };

    match report.stage {
        Stage::Installing(_) => DOWNLOADING,
        Stage::Running | Stage::Cached => family_label(report.subject.family()),
        _ => ENHANCING,
    }
}

/// The GUI's English chip label for `family`. Total, so a family added to the library fails to compile here until it
/// is given one.
const fn family_label(family: Family) -> &'static str {
    match family {
        Family::Detection | Family::FaceRecovery => "Face Recovery",
        Family::Denoise => "Denoise",
        Family::Colorization => "Colorize",
        Family::LightAdjustment => "Light Adjust",
        Family::ColorBalance => "Color Balance",
        Family::Sharpen => "Sharpen",
        Family::Upscale => "Upscale",
    }
}

/// The bar's `[n/N]` counter: `[0/planned]` before any report, then the reported step counted from one.
pub fn counter(report: Option<&InferenceProgress>, planned: usize) -> (usize, usize) {
    report.map_or((0, planned), |report| (report.operation_index + 1, report.operation_count))
}

fn paint(text: impl Display, (r, g, b): (u8, u8, u8), color: bool) -> String {
    if color {
        text.to_string().with(TermColor::Rgb { r, g, b }).to_string()
    } else {
        text.to_string()
    }
}

fn paint_bold(text: impl Display, (r, g, b): (u8, u8, u8), color: bool) -> String {
    if color {
        text.to_string().with(TermColor::Rgb { r, g, b }).bold().to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use opai::{
        Dependency, Detection, FaceRecovery, Faces, FloatPrecision, Operation, Phase, Progress, Scale, Subject,
        Upscale, UpscaleVariant,
    };

    fn plain(line: &Line<'_>) -> String {
        line.spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn report(subject: Subject, stage: Stage, (index, count): (usize, usize)) -> InferenceProgress {
        InferenceProgress {
            subject: Arc::new(subject),
            stage,
            operation_fraction: 0.5,
            operation_index: index,
            operation_count: count,
            chain_fraction: 0.5,
        }
    }

    fn face_recovery() -> Subject {
        Subject::Enhancement(FaceRecovery::santorini(FloatPrecision::Fp32, Faces::empty()))
    }

    fn upscale() -> Subject {
        let scale = Scale::new(2.0).expect("a valid scale");
        Subject::Enhancement(Operation::Upscale(Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp32), scale)))
    }

    fn installing() -> Stage {
        Stage::Installing(Progress {
            dependency: Dependency::Runtime,
            phase: Phase::Downloading,
            bytes: 10,
            total: Some(100),
            fraction: 0.1,
        })
    }

    #[test]
    fn color_needs_a_terminal_and_no_non_empty_no_color() {
        assert!(!use_color(false, None));
        assert!(use_color(true, None));
        assert!(use_color(true, Some(OsStr::new(""))));
        assert!(!use_color(true, Some(OsStr::new("1"))));
    }

    #[test]
    fn the_look_draws_live_lines_on_a_terminal_whatever_no_color_says() {
        assert_eq!(Look::new(true, None), Look { interactive: true, color: true });
        assert_eq!(Look::new(true, Some(OsStr::new(""))), Look { interactive: true, color: true });
        assert_eq!(Look::new(true, Some(OsStr::new("1"))), Look { interactive: true, color: false });
        assert_eq!(Look::new(false, None), Look { interactive: false, color: false });
    }

    #[test]
    fn the_lines_without_colour_are_their_exact_text() {
        assert_eq!(header(Path::new("photo.jpg"), false), "🖼️ Applying enhancements to the image photo.jpg");
        assert_eq!(success(Path::new("out.avif"), false), "✨ Image enhanced successfully; file saved at out.avif");
        assert_eq!(failure("stopped: nothing was written", false), "🧨 stopped: nothing was written");
    }

    #[test]
    fn the_header_paints_only_the_path_green() {
        let line = header(Path::new("photo.jpg"), true);

        assert_eq!(line, format!("🖼️ Applying enhancements to the image {}", paint("photo.jpg", GREEN, true)));
        assert!(line.starts_with("🖼️ Applying enhancements to the image \x1b"), "{line:?}");
    }

    #[test]
    fn the_result_lines_are_bold_and_coloured_throughout() {
        let (r, g, b) = YELLOW;
        let text = "✨ Image enhanced successfully; file saved at out.avif";
        assert_eq!(success(Path::new("out.avif"), true), text.with(TermColor::Rgb { r, g, b }).bold().to_string());

        let (r, g, b) = RED;
        assert_eq!(failure("boom", true), "🧨 boom".with(TermColor::Rgb { r, g, b }).bold().to_string());
    }

    #[test]
    fn the_dependency_line_is_its_exact_text() {
        let line = dependency_line("ONNX Runtime", 0.34, false);

        assert_eq!(plain(&line), "💾 Downloading dependency ONNX Runtime   34%");
        assert!(line.spans.iter().all(|span| span.style == Style::new()), "{line:?}");
    }

    #[test]
    fn the_dependency_line_colours_only_the_name_and_the_percentage() {
        let line = dependency_line("ONNX Runtime", 0.34, true);
        let (r, g, b) = YELLOW;

        assert_eq!(plain(&line), "💾 Downloading dependency ONNX Runtime   34%");
        assert_eq!(line.spans[0].style, Style::new());
        assert_eq!(line.spans[1].style, Style::new().fg(Color::Rgb(r, g, b)));
        assert_eq!(line.spans[2].style, Style::new());
        assert_eq!(line.spans[3].style, Style::new().fg(Color::White).add_modifier(Modifier::BOLD));
    }

    #[test]
    fn the_dependency_percentage_reaches_100_only_at_the_end() {
        assert_eq!(plain(&dependency_line("x", 0.999, false)), "💾 Downloading dependency x   99%");
        assert_eq!(plain(&dependency_line("x", 0.999_999_999_9, false)), "💾 Downloading dependency x   99%");
        assert_eq!(plain(&dependency_line("x", 1.0, false)), "💾 Downloading dependency x   100%");
        assert_eq!(plain(&dependency_line("x", 0.29, false)), "💾 Downloading dependency x   29%");
        assert_eq!(plain(&dependency_line("x", 0.0, false)), "💾 Downloading dependency x   0%");
    }

    #[test]
    fn before_any_report_the_bar_is_enhancing_at_step_zero() {
        assert_eq!(label(None), "Enhancing");
        assert_eq!(counter(None, 2), (0, 2));
    }

    #[test]
    fn an_install_reads_downloading_and_keeps_its_step() {
        let installing = report(upscale(), installing(), (1, 2));

        assert_eq!(label(Some(&installing)), "Downloading");
        assert_eq!(counter(Some(&installing), 2), (2, 2));
    }

    #[test]
    fn a_running_or_cached_operation_reads_as_its_family() {
        for stage in [Stage::Running, Stage::Cached] {
            let recovering = report(face_recovery(), stage.clone(), (0, 2));
            assert_eq!(label(Some(&recovering)), "Face Recovery");
            assert_eq!(counter(Some(&recovering), 2), (1, 2));

            let upscaling = report(upscale(), stage, (1, 2));
            assert_eq!(label(Some(&upscaling)), "Upscale");
            assert_eq!(counter(Some(&upscaling), 2), (2, 2));
        }
    }

    #[test]
    fn a_detection_reads_as_the_face_recovery_at_its_position() {
        // As `Handover::detection` rewrites it: attributed to the face recovery, operation 0 of 2.
        let detecting = report(Subject::Analysis(Detection::for_face_recovery()), Stage::Running, (0, 2));

        assert_eq!(label(Some(&detecting)), "Face Recovery");
        assert_eq!(counter(Some(&detecting), 2), (1, 2));
    }

    #[test]
    fn every_family_has_the_guis_english_label() {
        let labels = Family::ALL.map(family_label);

        assert_eq!(
            labels,
            [
                "Face Recovery",
                "Denoise",
                "Face Recovery",
                "Colorize",
                "Light Adjust",
                "Color Balance",
                "Sharpen",
                "Upscale"
            ]
        );
    }

    #[test]
    fn the_label_width_fits_the_widest_label() {
        assert_eq!(LABEL_WIDTH, "Face Recovery".len());
        let widest = Family::ALL.map(family_label).into_iter().chain([ENHANCING, DOWNLOADING]).map(str::len).max();
        assert_eq!(widest, Some(LABEL_WIDTH));
    }
}
