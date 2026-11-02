//! What the terminal's lines say and how they are coloured: the update line, the dependency line, the processor line, the list's rows
//! and their plain lines, the summary and the failure line. Pure text, so all of it is tested without a terminal.
//!
//! Whether to draw live lines and whether to colour is decided once, in [`Look`], the way MediaSim decides it: a
//! terminal, and `NO_COLOR` unset or empty.

use std::ffi::OsStr;
use std::fmt::Display;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use opai::{ExecutionProvider, Family};
use ratatui::crossterm::style::{Color as TermColor, Stylize};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::args::Quality;

/// The success line and the processor line's quality.
pub const YELLOW: (u8, u8, u8) = (0xe5, 0xdb, 0x08);
/// The failure line, bold, and a `🧨` row, not bold.
pub const RED: (u8, u8, u8) = (0xf4, 0x43, 0x36);
/// A row's percentage, UMD's blue.
pub const BLUE: (u8, u8, u8) = (0x82, 0xaa, 0xff);
/// A row's enhancement, and the dependency line's spinner, UMD's magenta.
pub const MAGENTA: (u8, u8, u8) = (0xc7, 0x92, 0xe9);
/// A row's path, underlined, UMD's cyan.
pub const CYAN: (u8, u8, u8) = (0x88, 0xdd, 0xff);
/// The dependency line's `✓` and the processor line's processor, UMD's green.
pub const GREEN: (u8, u8, u8) = (0x00, 0xc2, 0x02);

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

/// What an image being worked on is doing, as its row names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// Its run has reported nothing yet, or it has nothing to run.
    Preparing,
    /// Autopilot is analysing it, to choose its enhancements.
    Choosing,
    /// The model of an operation of this family is being fetched.
    Downloading(Family),
    /// An operation of this family runs, or is served from the run cache.
    Applying(Family),
}

/// Where an image's row stands: being worked on, or how it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    /// Being loaded, enhanced or written.
    Working(Activity),
    /// Its result is written.
    Applied,
    /// It failed.
    Failed,
    /// The run was stopped while working on it.
    Stopped,
}

/// How an image ended, with a failure's reason for its plain line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageEnd {
    /// Its result is written, with the enhancements it received in words under `--verbose`, and `None` without.
    Enhanced(Option<String>),
    /// It failed, for this reason. The reason need not name the image: the line already does.
    Failed(String),
    /// The run was stopped while working on it.
    Stopped,
}

impl ImageEnd {
    /// The row this end leaves.
    pub const fn state(&self) -> RowState {
        match self {
            Self::Enhanced(_) => RowState::Applied,
            Self::Failed(_) => RowState::Failed,
            Self::Stopped => RowState::Stopped,
        }
    }
}

/// An image's progress as its row shows it: `fraction` floored to a whole percentage, held at `99` until the image is
/// `written`, and `100` once it is, whatever `fraction` reached.
pub fn image_percent(fraction: f64, written: bool) -> u8 {
    if written { 100 } else { whole_percent(fraction).min(99) }
}

/// The columns a row's words take before its path: those of the longest, a download for the longest family name, so
/// every path starts in the same column.
const WORDS_WIDTH: usize = "Downloading ".len() + "Face Recovery".len() + " for".len();

/// The list's row for an image at `percent`: `🖼️ [ 42%] Applying Upscale to` and the other words `state` reads as,
/// padded to [`WORDS_WIDTH`], then the path. The percentage and its `%` are blue, the enhancement magenta and the path cyan and underlined, or a `🧨` row is red throughout. The percentage is right-aligned to three
/// columns and the words padded, so every path starts in the same column.
pub fn row(percent: u8, path: &str, state: RowState, color: bool) -> Line<'static> {
    let fg = |rgb| fg(rgb, color);
    let red = matches!(state, RowState::Failed | RowState::Stopped);
    let [plain, percent_style, word_style, path_style] = if red {
        [fg(RED); 4]
    } else {
        let underline = if color { Modifier::UNDERLINED } else { Modifier::empty() };
        [Style::new(), fg(BLUE), fg(MAGENTA), fg(CYAN).add_modifier(underline)]
    };

    // The words before the magenta one, the magenta one, and the words after it.
    let (emoji, before, word, after) = match state {
        RowState::Working(Activity::Preparing) => ("🖼️", "Preparing to enhance", None, ""),
        RowState::Working(Activity::Choosing) => ("🖼️", "Choosing enhancements for", None, ""),
        RowState::Working(Activity::Downloading(family)) => ("🖼️", "Downloading ", Some(family_label(family)), " for"),
        RowState::Working(Activity::Applying(family)) => ("🖼️", "Applying ", Some(family_label(family)), " to"),
        RowState::Applied => ("🖼️", "Enhancements applied to", None, ""),
        RowState::Failed => ("🧨", "Failed to enhance", None, ""),
        RowState::Stopped => ("🧨", "Stopped enhancing", None, ""),
    };
    let words = before.len() + word.map_or(0, str::len) + after.len();
    let padding = " ".repeat(WORDS_WIDTH.saturating_sub(words) + 1);

    let mut spans = vec![
        Span::styled(format!("{emoji} ["), plain),
        Span::styled(format!("{percent:>3}%"), percent_style),
        Span::styled(format!("] {before}"), plain),
    ];
    if let Some(word) = word {
        spans.push(Span::styled(word, word_style));
    }
    spans.push(Span::styled(format!("{after}{padding}"), plain));
    spans.push(Span::styled(path.to_string(), path_style));

    Line::from(spans)
}

/// `line`, a written image's row, followed by `: <applied>` in the terminal's default colour where there is a text,
/// so the path stays the row's most visible part. A row too wide for the terminal is cut at its edge when drawn.
pub fn with_applied(mut line: Line<'static>, applied: Option<&str>) -> Line<'static> {
    if let Some(applied) = applied {
        line.spans.push(Span::raw(format!(": {applied}")));
    }
    line
}

/// The line an image prints when it ends and no live view is drawn: the words of its final [`row`] at `percent`, then
/// `: <reason>` after a failure or `: <applied>` after a written image under `--verbose`, coloured as the row is only
/// when `color` is set.
pub fn plain_row(percent: u8, path: &str, end: &ImageEnd, color: bool) -> String {
    let mut line = row(percent, path, end.state(), color);
    match end {
        ImageEnd::Failed(reason) => {
            let style = line.spans.first().map_or_else(Style::new, |span| span.style);
            line.spans.push(Span::styled(format!(": {reason}"), style));
        }
        ImageEnd::Enhanced(applied) => line = with_applied(line, applied.as_deref()),
        ImageEnd::Stopped => {}
    }

    line.spans.iter().map(|span| ansi(&span.content, span.style)).collect()
}

/// `🧠 Running inference using the processor <provider>; quality <HD|SD>`, with the processor green and the quality
/// yellow. The processor is spelled as `opai` spells it, `Auto` included.
pub fn processor_line(provider: ExecutionProvider, quality: Quality, color: bool) -> String {
    let fg = |rgb| fg(rgb, color);

    [
        ("🧠 Running inference using the processor ", Style::new()),
        (provider.as_str(), fg(GREEN)),
        ("; quality ", Style::new()),
        (quality.label(), fg(YELLOW)),
    ]
    .iter()
    .map(|(text, style)| ansi(text, *style))
    .collect()
}

/// Where a newer release is downloaded from, as the update line names it.
pub const RELEASES_URL: &str = "https://github.com/vegidio/open-photo-ai";

/// `💡 New update available: download at <RELEASES_URL>`, all yellow, with the URL underlined. Static, not pulsing as
/// the GUI's button does: anything printed below it leaves it out of reach of a redraw.
pub fn update_line(color: bool) -> String {
    let underline = if color { Modifier::UNDERLINED } else { Modifier::empty() };

    [
        ("💡 New update available: download at ", fg(YELLOW, color)),
        (RELEASES_URL, fg(YELLOW, color).add_modifier(underline)),
    ]
    .iter()
    .map(|(text, style)| ansi(text, *style))
    .collect()
}

/// The summary's success line, uncoloured, or `None` when nothing was written: the file itself for one image, so a
/// numbered name stays visible, and for more `output` as given, or, without one, that each was saved beside its source.
pub fn summary(written: &[PathBuf], output: Option<&Path>) -> Option<String> {
    match (written, output) {
        ([], _) => None,
        ([file], _) => Some(format!("✨ Image enhanced successfully; file saved at {}", file.display())),
        (_, Some(output)) => Some(format!(
            "✨ {} images enhanced successfully; files saved under {}",
            written.len(),
            output.display()
        )),
        (_, None) => {
            Some(format!("✨ {} images enhanced successfully; files saved beside their sources", written.len()))
        }
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

/// The dependency line's mark once its install reads `100`: `✓`, green.
pub fn done_mark(color: bool) -> Span<'static> {
    Span::styled("✓", fg(GREEN, color))
}

/// Text in `(r, g, b)`, or unstyled without `color`.
fn fg((r, g, b): (u8, u8, u8), color: bool) -> Style {
    if color { Style::new().fg(Color::Rgb(r, g, b)) } else { Style::new() }
}

/// `💾 Downloading dependency <name>... <mark> <percent>% Complete`, with the name and the percentage in bold white
/// and `mark`, the spinner or [`done_mark`], drawn as given.
///
/// The percentage is `fraction` floored to a whole number, and `100` only once `fraction` is exactly `1.0`, which is
/// the library's last report for an install.
pub fn dependency_line(name: &str, fraction: f64, mark: Span<'static>, color: bool) -> Line<'static> {
    let bold = if color { Style::new().fg(Color::White).add_modifier(Modifier::BOLD) } else { Style::new() };

    Line::from(vec![
        Span::raw("💾 Downloading dependency "),
        Span::styled(name.to_string(), bold),
        Span::raw("... "),
        mark,
        Span::raw(" "),
        Span::styled(format!("{}%", whole_percent(fraction)), bold),
        Span::raw(" Complete"),
    ])
}

/// `fraction` as a whole percentage, floored, and below `100` until `fraction` reaches `1.0`. The small nudge keeps a
/// fraction such as `0.29`, stored as `0.28999…`, from showing as `28`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..=100 first")]
pub fn whole_percent(fraction: f64) -> u8 {
    let fraction = fraction.clamp(0.0, 1.0);
    let percent = (fraction * 100.0 + 1e-6).floor() as u8;

    if fraction < 1.0 { percent.min(99) } else { 100 }
}

/// The GUI's English chip label for `family`. Total, so a family added to the library fails to compile here until it
/// is given one. A face detection reads as the face recovery it serves, as it does in the GUI.
pub(crate) const fn family_label(family: Family) -> &'static str {
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

/// `text` with the parts of `style` a line uses, its foreground colour and underline, as escape sequences.
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
    use super::*;

    fn plain(line: &Line<'_>) -> String {
        line.spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn rgb((r, g, b): (u8, u8, u8)) -> Style {
        Style::new().fg(Color::Rgb(r, g, b))
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

    const APPLYING: RowState = RowState::Working(Activity::Applying(Family::Upscale));

    #[test]
    fn each_row_state_is_its_exact_text() {
        let path = "photos/b.jpg";
        let texts = [
            row(0, path, RowState::Working(Activity::Preparing), false),
            row(12, path, RowState::Working(Activity::Downloading(Family::Upscale)), false),
            row(42, path, RowState::Working(Activity::Applying(Family::FaceRecovery)), false),
            row(42, path, APPLYING, false),
            row(100, path, RowState::Applied, false),
            row(42, path, RowState::Failed, false),
            row(7, path, RowState::Stopped, false),
        ]
        .map(|line| plain(&line));

        assert_eq!(
            texts,
            [
                "🖼️ [  0%] Preparing to enhance          photos/b.jpg",
                "🖼️ [ 12%] Downloading Upscale for       photos/b.jpg",
                "🖼️ [ 42%] Applying Face Recovery to     photos/b.jpg",
                "🖼️ [ 42%] Applying Upscale to           photos/b.jpg",
                "🖼️ [100%] Enhancements applied to       photos/b.jpg",
                "🧨 [ 42%] Failed to enhance             photos/b.jpg",
                "🧨 [  7%] Stopped enhancing             photos/b.jpg",
            ]
        );
    }

    #[test]
    fn every_path_starts_in_the_same_column_and_the_longest_words_keep_a_space() {
        let mut states = vec![
            RowState::Working(Activity::Preparing),
            RowState::Working(Activity::Choosing),
            RowState::Applied,
            RowState::Failed,
            RowState::Stopped,
        ];
        for family in Family::ALL {
            states.push(RowState::Working(Activity::Downloading(family)));
            states.push(RowState::Working(Activity::Applying(family)));
        }

        for state in states {
            let text = plain(&row(42, "x.jpg", state, false));
            let words = text.split("] ").nth(1).and_then(|rest| rest.strip_suffix("x.jpg")).expect("words");

            assert_eq!(words.len(), WORDS_WIDTH + 1, "{text:?}");
            assert!(words.ends_with(' ') && !words.trim_end().is_empty(), "{text:?}");
        }
    }

    #[test]
    fn a_choosing_row_is_its_exact_text_and_colours_its_percentage_and_path() {
        let choosing = RowState::Working(Activity::Choosing);

        assert_eq!(
            plain(&row(0, "photo.jpg", choosing, false)),
            "🖼️ [  0%] Choosing enhancements for     photo.jpg"
        );

        let spans: Vec<_> = row(0, "photo.jpg", choosing, true)
            .spans
            .iter()
            .map(|span| (span.content.to_string(), span.style))
            .collect();
        assert_eq!(
            spans,
            [
                ("🖼️ [".to_owned(), Style::new()),
                ("  0%".to_owned(), rgb(BLUE)),
                ("] Choosing enhancements for".to_owned(), Style::new()),
                ("     ".to_owned(), Style::new()),
                ("photo.jpg".to_owned(), rgb(CYAN).add_modifier(Modifier::UNDERLINED)),
            ]
        );
    }

    #[test]
    fn a_detection_reads_as_face_recovery() {
        let detecting = RowState::Working(Activity::Applying(Family::Detection));

        assert!(plain(&row(3, "a.jpg", detecting, false)).starts_with("🖼️ [  3%] Applying Face Recovery to "));
    }

    #[test]
    fn an_applying_row_colours_the_percentage_the_family_and_the_path() {
        let line = row(42, "b.jpg", APPLYING, true);
        let spans: Vec<_> = line.spans.iter().map(|span| (span.content.as_ref(), span.style)).collect();

        assert_eq!(
            spans,
            [
                ("🖼️ [", Style::new()),
                (" 42%", rgb(BLUE)),
                ("] Applying ", Style::new()),
                ("Upscale", rgb(MAGENTA)),
                (" to           ", Style::new()),
                ("b.jpg", rgb(CYAN).add_modifier(Modifier::UNDERLINED)),
            ]
        );
    }

    #[test]
    fn an_applied_row_colours_its_percentage_but_not_its_words() {
        let line = row(100, "b.jpg", RowState::Applied, true);

        assert_eq!(line.spans[1].content, "100%");
        assert_eq!(line.spans[1].style, rgb(BLUE));
        assert_eq!(line.spans[2].content, "] Enhancements applied to");
        assert_eq!(line.spans[2].style, Style::new());
        assert_eq!(line.spans[4].style, rgb(CYAN).add_modifier(Modifier::UNDERLINED));
    }

    #[test]
    fn a_failed_or_stopped_row_is_red_throughout_and_not_bold() {
        for state in [RowState::Failed, RowState::Stopped] {
            let line = row(42, "b.jpg", state, true);
            assert!(line.spans.iter().all(|span| span.style == rgb(RED)), "{line:?}");
        }
    }

    #[test]
    fn a_row_without_colour_has_no_style() {
        let states = [
            RowState::Working(Activity::Preparing),
            RowState::Working(Activity::Choosing),
            RowState::Working(Activity::Downloading(Family::Upscale)),
            APPLYING,
            RowState::Applied,
            RowState::Failed,
            RowState::Stopped,
        ];

        for state in states {
            let line = row(42, "b.jpg", state, false);
            assert!(line.spans.iter().all(|span| span.style == Style::new()), "{line:?}");
        }
    }

    #[test]
    fn the_image_percentage_is_held_at_99_until_written() {
        assert_eq!(image_percent(0.0, false), 0);
        assert_eq!(image_percent(0.429, false), 42);
        assert_eq!(image_percent(0.999, false), 99);
        assert_eq!(image_percent(1.0, false), 99);
        assert_eq!(image_percent(1.0, true), 100);
        assert_eq!(image_percent(0.0, true), 100, "an image with nothing to run is still all applied");
    }

    #[test]
    fn a_plain_row_is_the_final_rows_text_with_a_failures_reason() {
        for end in [ImageEnd::Enhanced(None), ImageEnd::Failed("corrupt".into()), ImageEnd::Stopped] {
            let line = plain(&row(42, "c.jpg", end.state(), true));
            let reason = match &end {
                ImageEnd::Failed(reason) => format!(": {reason}"),
                _ => String::new(),
            };

            assert_eq!(plain_row(42, "c.jpg", &end, false), format!("{line}{reason}"));
        }

        assert_eq!(
            plain_row(42, "c.jpg", &ImageEnd::Failed("corrupt".into()), false),
            "🧨 [ 42%] Failed to enhance             c.jpg: corrupt"
        );
        assert_eq!(
            plain_row(100, "c.jpg", &ImageEnd::Enhanced(None), false),
            "🖼️ [100%] Enhancements applied to       c.jpg"
        );
    }

    fn written(applied: &str) -> ImageEnd {
        ImageEnd::Enhanced(Some(applied.to_owned()))
    }

    #[test]
    fn a_written_images_plain_row_under_verbose_ends_with_what_it_received() {
        assert_eq!(
            plain_row(100, "photo.jpg", &written("Face Recovery Athens, Upscale Tokyo 2x"), false),
            "🖼️ [100%] Enhancements applied to       photo.jpg: Face Recovery Athens, Upscale Tokyo 2x"
        );
        assert_eq!(
            plain_row(100, "large.jpg", &written("no enhancement needed"), false),
            "🖼️ [100%] Enhancements applied to       large.jpg: no enhancement needed"
        );
        assert_eq!(
            plain_row(100, "a.jpg", &written("Denoise Stockholm strength 100 (cached)"), false),
            "🖼️ [100%] Enhancements applied to       a.jpg: Denoise Stockholm strength 100 (cached)"
        );
    }

    #[test]
    fn the_applied_text_is_drawn_in_the_default_colour_after_the_coloured_path() {
        let (r, g, b) = CYAN;
        let line = plain_row(100, "c.jpg", &written("Sharpen Moscow strength 80"), true);

        let path = "c.jpg".with(TermColor::Rgb { r, g, b }).underlined().to_string();
        assert!(line.ends_with(&format!("{path}: Sharpen Moscow strength 80")), "{line:?}");

        let row = with_applied(row(100, "c.jpg", RowState::Applied, true), Some("Sharpen Moscow strength 80"));
        let last = row.spans.last().expect("a span");
        assert_eq!((last.content.as_ref(), last.style), (": Sharpen Moscow strength 80", Style::new()));
    }

    #[test]
    fn the_applied_text_leaves_every_path_in_its_column() {
        let applied = plain(&with_applied(row(100, "x.jpg", RowState::Applied, false), Some("Upscale Tokyo 2x")));
        let words = applied.split("] ").nth(1).and_then(|rest| rest.split_once("x.jpg")).expect("words").0;

        // The same columns every other row's words take, as the alignment test above pins them.
        assert_eq!(words.len(), WORDS_WIDTH + 1, "{applied:?}");
        assert!(applied.ends_with("x.jpg: Upscale Tokyo 2x"), "{applied:?}");
        assert_eq!(
            with_applied(row(100, "x.jpg", RowState::Applied, false), None),
            row(100, "x.jpg", RowState::Applied, false)
        );
    }

    #[test]
    fn a_plain_row_is_coloured_only_when_asked() {
        let (r, g, b) = RED;
        let failed = plain_row(42, "c.jpg", &ImageEnd::Failed("corrupt".into()), true);
        assert!(failed.ends_with(&": corrupt".with(TermColor::Rgb { r, g, b }).to_string()), "{failed:?}");

        let (r, g, b) = CYAN;
        let enhanced = plain_row(100, "c.jpg", &ImageEnd::Enhanced(None), true);
        assert!(
            enhanced.ends_with(&"c.jpg".with(TermColor::Rgb { r, g, b }).underlined().to_string()),
            "{enhanced:?}"
        );

        assert!(!plain_row(100, "c.jpg", &ImageEnd::Enhanced(None), false).contains('\x1b'));
        assert!(!plain_row(100, "c.jpg", &written("Colorize Delhi"), false).contains('\x1b'));
    }

    #[test]
    fn the_summary_names_the_file_for_one_image_and_the_output_for_more() {
        let output = Some(Path::new("out"));

        assert_eq!(summary(&[], output), None);
        assert_eq!(
            summary(&["out_1.png".into()], Some(Path::new("out.png"))).as_deref(),
            Some("✨ Image enhanced successfully; file saved at out_1.png")
        );
        assert_eq!(
            summary(&["out/a.png".into(), "out/b.png".into(), "out/c.png".into()], output).as_deref(),
            Some("✨ 3 images enhanced successfully; files saved under out")
        );
    }

    #[test]
    fn without_an_output_the_summary_names_the_file_for_one_image_and_says_beside_for_more() {
        assert_eq!(summary(&[], None), None);
        assert_eq!(
            summary(&["photo_opai.jpg".into()], None).as_deref(),
            Some("✨ Image enhanced successfully; file saved at photo_opai.jpg")
        );
        assert_eq!(
            summary(&["photos/a_opai.jpg".into(), "photos/2024/b_opai.jpg".into()], None).as_deref(),
            Some("✨ 2 images enhanced successfully; files saved beside their sources")
        );
    }

    #[test]
    fn the_processor_line_names_what_was_asked_for() {
        assert_eq!(
            processor_line(ExecutionProvider::Auto, Quality::Hd, false),
            "🧠 Running inference using the processor Auto; quality HD"
        );
        assert_eq!(
            processor_line(ExecutionProvider::Cpu, Quality::Sd, false),
            "🧠 Running inference using the processor CPU; quality SD"
        );
    }

    #[test]
    fn the_processor_line_colours_the_processor_and_the_quality() {
        let ((gr, gg, gb), (yr, yg, yb)) = (GREEN, YELLOW);
        let expected = format!(
            "🧠 Running inference using the processor {}; quality {}",
            "Auto".with(TermColor::Rgb { r: gr, g: gg, b: gb }),
            "HD".with(TermColor::Rgb { r: yr, g: yg, b: yb }),
        );

        assert_eq!(processor_line(ExecutionProvider::Auto, Quality::Hd, true), expected);
    }

    #[test]
    fn the_update_line_without_colour_is_its_exact_text() {
        let line = update_line(false);

        assert_eq!(line, "💡 New update available: download at https://github.com/vegidio/open-photo-ai");
        assert!(!line.contains('\x1b'), "{line:?}");
    }

    #[test]
    fn the_update_line_is_yellow_with_the_url_underlined() {
        let (r, g, b) = YELLOW;
        let line = update_line(true);

        assert!(
            line.starts_with(&"💡 New update available: download at ".with(TermColor::Rgb { r, g, b }).to_string())
        );
        assert!(
            line.ends_with(&RELEASES_URL.with(TermColor::Rgb { r, g, b }).underlined().to_string()),
            "{line:?}"
        );
    }

    #[test]
    fn the_dependency_line_is_its_exact_text() {
        let line = dependency_line("ONNX Runtime", 0.34, Span::raw("⣾"), false);

        assert_eq!(plain(&line), "💾 Downloading dependency ONNX Runtime... ⣾ 34% Complete");
        assert!(line.spans.iter().all(|span| span.style == Style::new()), "{line:?}");
    }

    #[test]
    fn the_dependency_line_makes_the_name_and_the_percentage_bold_white_and_keeps_the_marks_style() {
        let mark = Span::styled("⣾", rgb(MAGENTA));
        let line = dependency_line("ONNX Runtime", 0.34, mark.clone(), true);
        let bold = Style::new().fg(Color::White).add_modifier(Modifier::BOLD);
        let styles: Vec<_> = line.spans.iter().map(|span| span.style).collect();

        assert_eq!(styles, [Style::new(), bold, Style::new(), mark.style, Style::new(), bold, Style::new()]);
        assert_eq!(line.spans[1].content, "ONNX Runtime");
        assert_eq!(line.spans[5].content, "34%");
    }

    #[test]
    fn the_done_mark_is_a_green_check() {
        assert_eq!(done_mark(true), Span::styled("✓", rgb(GREEN)));
        assert_eq!(done_mark(false), Span::raw("✓"));
    }

    #[test]
    fn the_dependency_percentage_reaches_100_only_at_the_end() {
        let text = |fraction| plain(&dependency_line("x", fraction, Span::raw("*"), false));

        assert_eq!(text(0.999), "💾 Downloading dependency x... * 99% Complete");
        assert_eq!(text(0.999_999_999_9), "💾 Downloading dependency x... * 99% Complete");
        assert_eq!(text(1.0), "💾 Downloading dependency x... * 100% Complete");
        assert_eq!(text(0.29), "💾 Downloading dependency x... * 29% Complete");
        assert_eq!(text(0.0), "💾 Downloading dependency x... * 0% Complete");
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
}
