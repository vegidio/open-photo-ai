//! The command's usage events, under the names and parameter names the GUI's window sends them with.
//!
//! The window sends its events through Faro (`frontend/lib/events.ts`). This command has no window, so each one here is
//! an `info` record under [`TARGET`], the event's name as its message and each parameter a field of the same name, and
//! it reaches the collector as every other record does. Only the events a terminal run has an answer for are here; the
//! window's own (a crop, the comparison view, a setting, the TensorRT question, the update notice) have no counterpart.
//!
//! **A closed vocabulary**, as the window's: every value is a count, a duration, a yes-or-no, a spelling the GUI already
//! sends, or [`format_list`]'s output. Never a path, a name or an error's text.

use std::path::Path;

use opai::Family;
use opai::export::{ExportFormat, FormatChoice};

/// The target every event is recorded under.
pub const TARGET: &str = "cli::event";

/// `files_added`'s `source` for a run's inputs, beside the window's `browse`, `empty` and `drop`.
const CLI_SOURCE: &str = "cli";

/// Where an enhancement on an image came from: its flags, or `--autopilot`. The window's `manual` and `autopilot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Manual,
    Autopilot,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Autopilot => "autopilot",
        }
    }
}

/// Once per run, once initialization has finished and the processor was accepted: what people run with.
pub fn app_ready(processor: &str, autopilot: bool) {
    tracing::info!(target: TARGET, processor, autopilot, "app_ready");
}

/// The photographs a run admitted: how many, and their types.
pub fn files_added(paths: &[&Path]) {
    let count = paths.len();
    let formats = format_list(paths);

    tracing::info!(target: TARGET, count, source = CLI_SOURCE, formats = %formats, "files_added");
}

/// An enhancement planned for an image, from its flags or by autopilot.
pub fn enhancement_added(family: Family, source: Source) {
    tracing::info!(target: TARGET, family = family_name(family), source = source.as_str(), "enhancement_added");
}

/// An image's analysis, completed: how many enhancements it suggested, zero included.
pub fn autopilot_run(count: usize) {
    tracing::info!(target: TARGET, count, "autopilot_run");
}

/// The batch starting.
pub fn export_started(file_count: usize, format: &str, processor: &str) {
    tracing::info!(target: TARGET, file_count, format, processor, "export_started");
}

/// The batch ending, run to the end or stopped. Milliseconds, as the window sends them.
pub fn export_finished(file_count: usize, exported: usize, completed: bool, duration_ms: u128) {
    // Saturated rather than truncated: no batch runs for 584 million years, and a `u64` is what a field records as an
    // integer.
    let duration_ms = u64::try_from(duration_ms).unwrap_or(u64::MAX);

    tracing::info!(target: TARGET, file_count, exported, completed, duration_ms, "export_finished");
}

/// The format a run writes, as the window's `export_started` names it: `preserve`, or the format's name.
pub fn format_name(choice: FormatChoice) -> String {
    choice.to_string()
}

/// The format a single output file is written in, under the same name an export of it would be: the one export
/// format that writes it, or `None` where none does.
pub fn export_format_of(format: opai::ImageFormat) -> Option<ExportFormat> {
    ExportFormat::ALL.into_iter().find(|candidate| opai::ImageFormat::from(*candidate) == format)
}

/// The file types among `paths`: sorted, distinct, lowercase extensions joined by commas, as the window's `formatList`
/// summarises one import. Extensions only, never a file's name.
pub fn format_list(paths: &[&Path]) -> String {
    let mut extensions: Vec<String> = paths
        .iter()
        .filter_map(|path| path.extension())
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .filter(|extension| !extension.is_empty())
        .collect();
    extensions.sort();
    extensions.dedup();

    extensions.join(",")
}

/// `family` as the window's `Family` type and the library's serde spell it.
fn family_name(family: Family) -> &'static str {
    match family {
        Family::Detection => "detection",
        Family::Denoise => "denoise",
        Family::FaceRecovery => "face_recovery",
        Family::Colorization => "colorization",
        Family::LightAdjustment => "light_adjustment",
        Family::ColorBalance => "color_balance",
        Family::Sharpen => "sharpen",
        Family::Upscale => "upscale",
    }
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::*;
    use crate::test_support::{Recorded, SeenEvent, recorded};

    /// The one event `emit` recorded, checked to be an `info` record under [`TARGET`] named `name`.
    fn only(recorded: &Recorded, name: &str) -> SeenEvent {
        let [event] = recorded.events.as_slice() else { panic!("{:#?}", recorded.events) };
        assert_eq!(event.level, Level::INFO);
        assert_eq!(event.target, TARGET);
        assert_eq!(event.field("message"), Some(name));

        event.clone()
    }

    #[test]
    fn each_event_carries_the_windows_name_and_parameter_names() {
        let (seen, ()) = recorded(|| app_ready("coreml", true));
        let event = only(&seen, "app_ready");
        assert_eq!((event.field("processor"), event.field("autopilot")), (Some("coreml"), Some("true")));

        let (seen, ()) = recorded(|| files_added(&[Path::new("a/B.JPG"), Path::new("c.png"), Path::new("d.jpg")]));
        let event = only(&seen, "files_added");
        assert_eq!(event.field("count"), Some("3"));
        assert_eq!(event.field("source"), Some("cli"));
        assert_eq!(event.field("formats"), Some("jpg,png"));

        let (seen, ()) = recorded(|| enhancement_added(Family::FaceRecovery, Source::Autopilot));
        let event = only(&seen, "enhancement_added");
        assert_eq!((event.field("family"), event.field("source")), (Some("face_recovery"), Some("autopilot")));

        let (seen, ()) = recorded(|| autopilot_run(0));
        assert_eq!(only(&seen, "autopilot_run").field("count"), Some("0"));

        let (seen, ()) = recorded(|| export_started(4, "preserve", "auto"));
        let event = only(&seen, "export_started");
        assert_eq!(
            (event.field("file_count"), event.field("format"), event.field("processor")),
            (Some("4"), Some("preserve"), Some("auto"))
        );

        let (seen, ()) = recorded(|| export_finished(4, 3, false, 1_250));
        let event = only(&seen, "export_finished");
        assert_eq!(
            (
                event.field("file_count"),
                event.field("exported"),
                event.field("completed"),
                event.field("duration_ms")
            ),
            (Some("4"), Some("3"), Some("false"), Some("1250"))
        );
    }

    #[test]
    fn a_format_list_is_sorted_distinct_and_lowercase_and_names_no_file() {
        let paths = [Path::new("/x/Raw.CR2"), Path::new("b.jpg"), Path::new("c.JPG"), Path::new("noext")];

        assert_eq!(format_list(&paths), "cr2,jpg");
        assert_eq!(format_list(&[]), "");
    }

    #[test]
    fn formats_are_named_as_the_window_names_them() {
        assert_eq!(format_name(FormatChoice::Preserve), "preserve");
        assert_eq!(format_name(FormatChoice::Format(ExportFormat::Jpeg)), "jpeg");

        for format in ExportFormat::ALL {
            assert_eq!(export_format_of(format.into()), Some(format));
        }
    }

    #[test]
    fn families_are_spelled_as_the_window_spells_them() {
        let names: Vec<_> = [
            Family::Detection,
            Family::Denoise,
            Family::FaceRecovery,
            Family::LightAdjustment,
            Family::ColorBalance,
            Family::Colorization,
            Family::Sharpen,
            Family::Upscale,
        ]
        .map(family_name)
        .into();

        // `frontend/ipc/catalogue.ts`'s `Family`, in its order.
        assert_eq!(
            names,
            [
                "detection",
                "denoise",
                "face_recovery",
                "light_adjustment",
                "color_balance",
                "colorization",
                "sharpen",
                "upscale"
            ]
        );
    }
}
