//! What the terminal's lines say and how they are coloured: the dependency line, the bar's label and counter, the list's
//! rows and their plain lines, the summary and the failure line. Pure text, so all of it is tested without a terminal.
//!
//! Whether to draw live lines and whether to colour is decided once, in [`Look`], the way MediaSim decides it: a
//! terminal, and `NO_COLOR` unset or empty.

use std::ffi::OsStr;
use std::fmt::Display;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use opai::{Family, InferenceProgress, Stage};
use ratatui::crossterm::style::{Color as TermColor, Stylize};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// The dependency's name and the success line.
pub const YELLOW: (u8, u8, u8) = (0xe5, 0xdb, 0x08);
/// The failure line, bold, and a `🧨` row, not bold.
pub const RED: (u8, u8, u8) = (0xf4, 0x43, 0x36);
/// A row's `n/N`, UMD's blue.
pub const BLUE: (u8, u8, u8) = (0x82, 0xaa, 0xff);
/// A row's word `image`, UMD's magenta.
pub const MAGENTA: (u8, u8, u8) = (0xc7, 0x92, 0xe9);
/// A row's path, underlined, UMD's cyan.
pub const CYAN: (u8, u8, u8) = (0x88, 0xdd, 0xff);

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
    /// Live lines are drawn and blank lines frame the view. Otherwise each image prints one plain line when it ends.
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

/// Where an image's row stands: being worked on, or how it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    /// Being loaded, enhanced or written.
    Enhancing,
    /// Its result is written.
    Enhanced,
    /// It failed.
    Failed,
    /// The run was stopped while working on it.
    Stopped,
}

/// How an image ended, with a failure's reason for its plain line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageEnd {
    /// Its result is written.
    Enhanced,
    /// It failed, for this reason. The reason need not name the image: the line already does.
    Failed(String),
    /// The run was stopped while working on it.
    Stopped,
}

impl ImageEnd {
    /// The row this end leaves.
    pub const fn state(&self) -> RowState {
        match self {
            Self::Enhanced => RowState::Enhanced,
            Self::Failed(_) => RowState::Failed,
            Self::Stopped => RowState::Stopped,
        }
    }
}

/// The list's row for the image at `index` (from one) of `total`: `🖼️ [n/N] Enhancing image <path>` or `Enhanced`,
/// with `n/N` blue, `image` magenta and the path cyan and underlined, or `🧨 [n/N] Failed image <path>` or `Stopped`,
/// red throughout. `n` is right-aligned to the width of `N`, so the words do not shift when the count gains a digit.
pub fn row(index: usize, total: usize, path: &str, state: RowState, color: bool) -> Line<'static> {
    let (emoji, word) = match state {
        RowState::Enhancing => ("🖼️", "Enhancing"),
        RowState::Enhanced => ("🖼️", "Enhanced"),
        RowState::Failed => ("🧨", "Failed"),
        RowState::Stopped => ("🧨", "Stopped"),
    };
    let fg = |(r, g, b): (u8, u8, u8)| if color { Style::new().fg(Color::Rgb(r, g, b)) } else { Style::new() };
    let red = matches!(state, RowState::Failed | RowState::Stopped);
    let [plain, count, image, path_style] = if red {
        [fg(RED); 4]
    } else {
        let underline = if color { Modifier::UNDERLINED } else { Modifier::empty() };
        [Style::new(), fg(BLUE), fg(MAGENTA), fg(CYAN).add_modifier(underline)]
    };
    let width = total.to_string().len();

    Line::from(vec![
        Span::styled(format!("{emoji} ["), plain),
        Span::styled(format!("{index:>width$}/{total}"), count),
        Span::styled(format!("] {word} "), plain),
        Span::styled("image", image),
        Span::styled(" ", plain),
        Span::styled(path.to_string(), path_style),
    ])
}

/// The line an image prints when it ends and no live view is drawn: the words of its final [`row`], then
/// `: <reason>` after a failure, coloured as the row is only when `color` is set.
pub fn plain_row(index: usize, total: usize, path: &str, end: &ImageEnd, color: bool) -> String {
    let mut line = row(index, total, path, end.state(), color);
    if let ImageEnd::Failed(reason) = end {
        let style = line.spans.first().map_or_else(Style::new, |span| span.style);
        line.spans.push(Span::styled(format!(": {reason}"), style));
    }

    line.spans.iter().map(|span| ansi(&span.content, span.style)).collect()
}

/// The summary's success line, uncoloured, or `None` when nothing was written: the file itself for one image, so a
/// numbered name stays visible, and `output` as given for more.
pub fn summary(written: &[PathBuf], output: &Path) -> Option<String> {
    match written {
        [] => None,
        [file] => Some(format!("✨ Image enhanced successfully; file saved at {}", file.display())),
        _ => Some(format!(
            "✨ {} images enhanced successfully; files saved under {}",
            written.len(),
            output.display()
        )),
    }
}

/// `line`, all in bold yellow: the summary's success line.
pub fn success(line: impl Display, color: bool) -> String {
    paint_bold(line, YELLOW, color)
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

/// `text` with the parts of `style` a row uses, its foreground colour and underline, as escape sequences.
fn ansi(text: &str, style: Style) -> String {
    let mut styled = text.stylize();
    if let Some(Color::Rgb(r, g, b)) = style.fg {
        styled = styled.with(TermColor::Rgb { r, g, b });
    }
    if style.add_modifier.contains(Modifier::UNDERLINED) {
        styled = styled.underlined();
    }

    styled.to_string()
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
        assert_eq!(success("✨ done", false), "✨ done");
        assert_eq!(failure("stopped: nothing more was written", false), "🧨 stopped: nothing more was written");
    }

    #[test]
    fn the_result_lines_are_bold_and_coloured_throughout() {
        let (r, g, b) = YELLOW;
        let text = "✨ Image enhanced successfully; file saved at out.avif";
        assert_eq!(success(text, true), text.with(TermColor::Rgb { r, g, b }).bold().to_string());

        let (r, g, b) = RED;
        assert_eq!(failure("boom", true), "🧨 boom".with(TermColor::Rgb { r, g, b }).bold().to_string());
    }

    const STATES: [RowState; 4] = [RowState::Enhancing, RowState::Enhanced, RowState::Failed, RowState::Stopped];

    #[test]
    fn each_row_state_is_its_exact_text() {
        let texts = STATES.map(|state| plain(&row(2, 5, "photos/b.jpg", state, false)));

        assert_eq!(
            texts,
            [
                "🖼️ [2/5] Enhancing image photos/b.jpg",
                "🖼️ [2/5] Enhanced image photos/b.jpg",
                "🧨 [2/5] Failed image photos/b.jpg",
                "🧨 [2/5] Stopped image photos/b.jpg",
            ]
        );
    }

    #[test]
    fn the_position_is_padded_to_the_width_of_the_total() {
        assert_eq!(plain(&row(2, 15, "a.jpg", RowState::Enhanced, false)), "🖼️ [ 2/15] Enhanced image a.jpg");
        assert_eq!(plain(&row(12, 15, "a.jpg", RowState::Enhanced, false)), "🖼️ [12/15] Enhanced image a.jpg");
        assert_eq!(plain(&row(7, 100, "a.jpg", RowState::Failed, false)), "🧨 [  7/100] Failed image a.jpg");
    }

    #[test]
    fn a_working_row_colours_the_count_the_word_image_and_the_path() {
        let rgb = |(r, g, b): (u8, u8, u8)| Style::new().fg(Color::Rgb(r, g, b));

        for state in [RowState::Enhancing, RowState::Enhanced] {
            let line = row(2, 5, "b.jpg", state, true);
            let styles: Vec<_> = line.spans.iter().map(|span| span.style).collect();

            assert_eq!(
                styles,
                [
                    Style::new(),
                    rgb(BLUE),
                    Style::new(),
                    rgb(MAGENTA),
                    Style::new(),
                    rgb(CYAN).add_modifier(Modifier::UNDERLINED)
                ]
            );
            assert_eq!(line.spans[1].content, "2/5");
            assert_eq!(line.spans[3].content, "image");
            assert_eq!(line.spans[5].content, "b.jpg");
        }
    }

    #[test]
    fn a_failed_or_stopped_row_is_red_throughout_and_not_bold() {
        let (r, g, b) = RED;

        for state in [RowState::Failed, RowState::Stopped] {
            let line = row(2, 5, "b.jpg", state, true);
            assert!(line.spans.iter().all(|span| span.style == Style::new().fg(Color::Rgb(r, g, b))), "{line:?}");
        }
    }

    #[test]
    fn a_row_without_colour_has_no_style() {
        for state in STATES {
            let line = row(2, 5, "b.jpg", state, false);
            assert!(line.spans.iter().all(|span| span.style == Style::new()), "{line:?}");
        }
    }

    #[test]
    fn a_plain_row_is_the_final_rows_text_with_a_failures_reason() {
        for end in [ImageEnd::Enhanced, ImageEnd::Failed("corrupt".into()), ImageEnd::Stopped] {
            let line = plain(&row(3, 5, "c.jpg", end.state(), true));
            let reason = match &end {
                ImageEnd::Failed(reason) => format!(": {reason}"),
                _ => String::new(),
            };

            assert_eq!(plain_row(3, 5, "c.jpg", &end, false), format!("{line}{reason}"));
        }

        assert_eq!(
            plain_row(3, 5, "c.jpg", &ImageEnd::Failed("corrupt".into()), false),
            "🧨 [3/5] Failed image c.jpg: corrupt"
        );
    }

    #[test]
    fn a_plain_row_is_coloured_only_when_asked() {
        let (r, g, b) = RED;
        let failed = plain_row(3, 5, "c.jpg", &ImageEnd::Failed("corrupt".into()), true);
        assert!(failed.ends_with(&": corrupt".with(TermColor::Rgb { r, g, b }).to_string()), "{failed:?}");

        let (r, g, b) = CYAN;
        let enhanced = plain_row(3, 5, "c.jpg", &ImageEnd::Enhanced, true);
        assert!(
            enhanced.ends_with(&"c.jpg".with(TermColor::Rgb { r, g, b }).underlined().to_string()),
            "{enhanced:?}"
        );

        assert!(!plain_row(3, 5, "c.jpg", &ImageEnd::Enhanced, false).contains('\x1b'));
    }

    #[test]
    fn the_summary_names_the_file_for_one_image_and_the_output_for_more() {
        let output = Path::new("out");

        assert_eq!(summary(&[], output), None);
        assert_eq!(
            summary(&["out_1.png".into()], Path::new("out.png")).as_deref(),
            Some("✨ Image enhanced successfully; file saved at out_1.png")
        );
        assert_eq!(
            summary(&["out/a.png".into(), "out/b.png".into(), "out/c.png".into()], output).as_deref(),
            Some("✨ 3 images enhanced successfully; files saved under out")
        );
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
