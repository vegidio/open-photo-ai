//! The rendered report: the conditions, the table, and everything a reader must not assume.

// The header is part of the contract rather than decoration. A table of durations with no header is not quotable —
// pasted into an issue it cannot be told apart from one taken at a different precision, on a different provider, or
// with the cache on, and every one of those differences is larger than the differences the table exists to show.

use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use comfy_table::{ContentArrangement, Table, presets};
use opai::{CacheMode, Detection, ExecutionProvider};

use crate::cli::Options;
use crate::input::Input;
use crate::stats;
use crate::sweep::{Detected, Measured, ModelResult, Outcome, SweepResult, Verdict};

/// Every duration renders at this width, so the columns line up whether a model takes 371.3ms or 12.400s.
const DURATION_WIDTH: usize = 9;

// Fixed because the table crate's `tty` feature is off (see this crate's manifest), and `Dynamic` falls back to the
// content's own widths without one. A ceiling rather than a width: eleven columns of which six are a fixed-width
// duration need about 135, and a narrower cap wraps `1280x1280` across two lines.
/// The widest the table may get, fixed rather than probed.
const TABLE_WIDTH: u16 = 150;

/// Everything the header states, gathered where the sweep's caller can see all of it at once.
#[derive(Debug, Clone)]
pub struct Conditions<'a> {
    /// The flags the sweep ran under.
    pub options: &'a Options,
    /// The image every model was measured against.
    pub input: &'a Input,
    // A list rather than `SupportedProviders`, which no crate but the library can construct — so the whole header is
    // renderable under test, on a runner with no GPU.
    /// What this machine can be asked to run on, from `SupportedProviders::available`.
    ///
    /// Excludes `Auto` by construction — it is a request rather than a capability — so nothing downstream of here
    /// filters for it.
    pub supported: Vec<ExecutionProvider>,
    /// What is backing the run cache, from `Opai::cache_mode`.
    pub cache_mode: CacheMode,
    /// Where the library's own records went, or `None` where the sink could not be installed.
    pub log: Option<&'a Path>,
    /// How many models were selected.
    pub selected: usize,
    // One of the conditions the numbers were produced under, and reported for the reason the image's dimensions are: a
    // face-recovery model's timings scale with the number of faces, so a table without it cannot be compared with
    // another machine's.
    /// What the auxiliary detection run found, or `None` where the sweep made none.
    pub detected: Option<&'a Detected>,
}

/// Everything after the header: the table, the lists and the footer.
///
/// Separate from [`header`] because the binary prints the two apart — the header before the sweep, and this after.
///
/// Assembled as sections joined by a blank line, so that a report with nothing to say in one of them does not leave
/// a gap where it would have been — a sweep of a single declined model is one list and nothing else.
pub fn body(conditions: &Conditions<'_>, sweep: &SweepResult) -> String {
    let completed: Vec<&ModelResult> = sweep
        .results
        .iter()
        .filter(|result| !matches!(result.outcome, Outcome::Declined { .. }))
        .collect();

    let measured = completed.iter().any(|result| matches!(result.outcome, Outcome::Completed(_)));

    let mut sections: Vec<String> = Vec::new();

    if !completed.is_empty() {
        // The table is the one section the crate rendering it does not terminate with a newline, so it is given one
        // here rather than every other section being written without theirs.
        sections.push(format!("{}\n", table(&completed, conditions.input.megapixels())));
    }

    if conditions.options.verbose && measured {
        sections.push(timed_runs(&sweep.results));
    }

    sections.extend(failures(&sweep.results));
    sections.extend(declines(&sweep.results, sweep.skipped));
    sections.extend(downgrades(&sweep.results, conditions.options.provider));

    // The footer is about the numbers, so a report with none of them says none of it.
    if measured {
        sections.push(footer(conditions.options));
    }

    if sections.is_empty() {
        return String::new();
    }

    format!("{}\n", sections.join("\n"))
}

/// The conditions that produced the numbers, before the numbers.
pub fn header(conditions: &Conditions<'_>) -> String {
    let options = conditions.options;
    let (width, height) = conditions.input.dimensions();

    let mut out = String::from("perftest — Open Photo AI inference benchmark\n\n");

    let mut line = |label: &str, value: String| {
        let _ = writeln!(out, "  {label:<10} {value}");
    };

    line(
        "image",
        format!("{}, {width}x{height} ({:.2} MPix)", conditions.input.label(), conditions.input.megapixels()),
    );

    // Requested beside what the machine supports, which is the answer to "why did this run on the CPU" on a machine
    // with no GPU provider — a question the requested provider alone cannot answer.
    let supported: Vec<&str> = conditions.supported.iter().map(|provider| provider.as_str()).collect();
    line("provider", format!("{} (this machine supports: {})", options.provider, supported.join(", ")));

    line("precision", options.precision().to_string());

    // The warning `performance-benchmark` requires: where no warm-up was asked for, the cold start may include
    // obtaining the model, and the number must not be presented as though it were comparable with one that did not.
    let warmup = if options.warmup == 0 {
        "⚠ no warm-up: the cold start may include obtaining the model".to_string()
    } else {
        "cold start measured after the warm-up, so it excludes obtaining the model".to_string()
    };
    line("runs", format!("{} timed, {} warm-up   ({warmup})", options.runs, options.warmup));

    // The scale, strength and bias flags, whichever families the selection happened to cover: which one moved a given
    // row is `perftest list`'s answer, and a header that printed only the ones in play would differ between two sweeps
    // that were otherwise identical. `--fidelity` is not recorded here.
    line(
        "params",
        format!("scale {}, strength {}, bias {}", options.scale, options.strength, options.bias),
    );

    let cache = if options.cache {
        "⚠ on — timings include what the cache costs: encoding the result and writing it"
    } else {
        "off — timings are inference, not encoding and storing the result"
    };
    line("cache", format!("{cache}   (store: {})", cache_mode(conditions.cache_mode)));

    // This project's own line rather than the reference's: `logging::init` returns the path, the per-tile and
    // per-session records are in that file, and `RUST_LOG` is what turns them up.
    line(
        "log",
        conditions
            .log
            .map_or_else(|| "none — the sink could not be installed".to_string(), |path| path.display().to_string()),
    );

    line("models", format!("{} selected", conditions.selected));

    // Only where the sweep made the run, so a sweep of upscalers says nothing about faces rather than saying zero —
    // which would read as a photograph with nobody in it.
    if let Some(detected) = conditions.detected {
        let found = match detected {
            Detected::Found(faces) => format!(
                "{} detected before the sweep, by {} — supplied to every face-recovery row, timed in none",
                faces.len(),
                Detection::for_face_recovery().display_name()
            ),
            Detected::Empty => {
                "none detected in this image — every face-recovery row is failed rather than measured".to_string()
            }
            Detected::Failed { reason } => {
                format!("detection failed ({reason}) — every face-recovery row is failed rather than measured")
            }
        };

        line("faces", found);
    }

    out.push('\n');
    out
}

/// The summary table.
fn table(results: &[&ModelResult], megapixels: f64) -> String {
    let mut table = Table::new();

    table
        .load_style(presets::UTF8_FULL)
        // A fixed width rather than the terminal's, so no TTY is probed and a pasted table is the same table.
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(TABLE_WIDTH)
        .set_header(vec![
            "MODEL", "TYPE", "COLD", "MIN", "MEDIAN", "MEAN", "MAX", "STDDEV", "MPIX/S", "OUTPUT", "NOTE",
        ]);

    for result in results {
        table.add_row(row(result, megapixels));
    }

    table.to_string()
}

/// One model's row.
fn row(result: &ModelResult, megapixels: f64) -> Vec<String> {
    let family = result.family.label().to_string();

    let Outcome::Completed(measured) = &result.outcome else {
        let note = match &result.outcome {
            Outcome::Interrupted => "INTERRUPTED",
            Outcome::Failed { .. } => "FAILED",
            // Declines never reach the table: they are not measurements, and the list below carries their reasons.
            _ => "-",
        };

        let mut row = vec![result.codename.to_string(), family];
        row.extend(std::iter::repeat_n("-".to_string(), 8));
        row.push(note.to_string());

        return row;
    };

    vec![
        result.codename.to_string(),
        family,
        duration(measured.cold),
        duration(measured.stats.min),
        duration(measured.stats.median),
        duration(measured.stats.mean),
        duration(measured.stats.max),
        duration(measured.stats.std_dev),
        format!("{:.2}", stats::megapixels_per_second(megapixels, measured.stats.median)),
        output(measured),
        note(measured),
    ]
}

/// What a completed row's NOTE column says.
///
/// What the run actually executed on, which is the one fact about a row that cannot be read off its numbers.
fn note(measured: &Measured) -> String {
    match &measured.providers {
        Verdict::AsRequested(provider) => provider.to_string(),
        Verdict::Resolved(providers) => joined(providers, "+"),
        Verdict::Downgraded { actual, .. } => {
            format!("⚠ ran on {}", joined(actual, "+"))
        }
        Verdict::NothingExecuted => "⚠ nothing executed".to_string(),
    }
}

/// The dimensions the model's last run produced.
fn output(measured: &Measured) -> String {
    let (width, height) = measured.output;

    if width == 0 || height == 0 { "-".to_string() } else { format!("{width}x{height}") }
}

/// Every timed run in run order, which is how a thermal ramp the median hides becomes visible.
fn timed_runs(results: &[ModelResult]) -> String {
    let mut out = String::from("Timed runs (in order):\n");

    for result in results {
        let Outcome::Completed(measured) = &result.outcome else { continue };

        let runs: Vec<String> = measured.runs.iter().map(|run| duration(*run)).collect();

        let _ = writeln!(out, "  {:<12} {}", result.codename, runs.join(" "));
    }

    out
}

/// `items` under `title`, numbered from one.
fn numbered(title: &str, items: &[String]) -> String {
    // The one place the report's list shape is decided. Three lists are rendered this way — failures, declines and
    // warnings — and written out three times the `index + 1` would have three places to be got wrong and the indent
    // three places to drift.
    let mut out = format!("{title}:\n");

    for (index, item) in items.iter().enumerate() {
        let _ = writeln!(out, "  {}. {item}", index + 1);
    }

    out
}

/// Every outcome `pick` names a reason for, rendered as `codename: reason`.
fn reasons(results: &[ModelResult], pick: impl Fn(&Outcome) -> Option<&String>) -> Vec<String> {
    results
        .iter()
        .filter_map(|result| pick(&result.outcome).map(|reason| format!("{}: {reason}", result.codename)))
        .collect()
}

/// A list of providers, as a cell or a sentence spells one.
fn joined(providers: &[ExecutionProvider], separator: &str) -> String {
    providers.iter().map(|provider| provider.as_str()).collect::<Vec<_>>().join(separator)
}

/// The genuine failures, with the reason each gave.
///
/// Interruptions are left out: the table already marks them, and repeating a cancellation once per model buries the
/// real errors.
fn failures(results: &[ModelResult]) -> Option<String> {
    let failed = reasons(results, |outcome| match outcome {
        Outcome::Failed { reason } => Some(reason),
        _ => None,
    });

    if failed.is_empty() {
        return None;
    }

    Some(numbered("Failures", &failed))
}

/// The models the library declined, and how many a sweep that named none dropped.
fn declines(results: &[ModelResult], skipped: usize) -> Option<String> {
    let declined = reasons(results, |outcome| match outcome {
        Outcome::Declined { reason } => Some(reason),
        _ => None,
    });

    if declined.is_empty() && skipped == 0 {
        return None;
    }

    let mut out = if declined.is_empty() { String::new() } else { numbered("Declined", &declined) };

    if skipped > 0 {
        if !declined.is_empty() {
            out.push('\n');
        }

        let _ = writeln!(
            out,
            "  {skipped} model(s) skipped: the application has no pipeline for them yet, so there was nothing to \
             measure. Name one to see why."
        );
    }

    Some(out)
}

/// The downgrade warnings, which are the whole reason the provider report is folded.
fn downgrades(results: &[ModelResult], requested: ExecutionProvider) -> Option<String> {
    // Without this the most misleading outcome available is silent: a model that could not be opened on a GPU
    // completes on the CPU, returns the same image in the same way, and is several times slower — which is
    // indistinguishable, in a table, from a GPU that is slow.
    let mut warnings: Vec<String> = Vec::new();

    for result in results {
        let Outcome::Completed(measured) = &result.outcome else { continue };

        match &measured.providers {
            // The run's own requested provider rather than the sweep's, which are the same value — read off the
            // row so the sentence cannot describe a different request from the one that produced it.
            Verdict::Downgraded { requested: asked, actual } => warnings.push(format!(
                "{}: {asked} was requested but sessions were built on {}",
                result.codename,
                joined(actual, ", ")
            )),
            Verdict::NothingExecuted => warnings.push(format!(
                "{}: no session was built at all, so nothing was executed — this row is not a timing of {requested}",
                result.codename
            )),
            _ => {}
        }
    }

    if warnings.is_empty() {
        return None;
    }

    let mut out = numbered("Warnings", &warnings);

    let _ = writeln!(out, "  Those rows are timings of what actually ran, not of what was asked for.");

    Some(out)
}

/// The two things a reader must not assume.
fn footer(options: &Options) -> String {
    let mut out = String::from("  MPix/s is input megapixels over the median run.\n");

    out.push_str(
        "  Comparable within one invocation, not across machines or processes: nothing here controls for CPU \
         pinning or thermal throttling.\n",
    );

    if options.cache {
        out.push_str("  --cache is on, so the timings include encoding each result and writing it to the store.\n");
    }

    out
}

/// What is backing the run cache, as the header spells it: the library's own name for the mode, with a sentence added
/// only for the two modes that are a degradation.
fn cache_mode(mode: CacheMode) -> String {
    // The name comes from `CacheMode::as_str` rather than being transcribed, so the header and the `--json` document
    // cannot spell one mode two ways. A reader seeing `disk` needs no explanation, and one seeing `none` does.
    //
    // `CacheMode` is `#[non_exhaustive]`, so the tail below cannot be matched exhaustively from this crate. The
    // fallback is an empty explanation rather than the word `unknown`: a mode added to the library still reaches the
    // header under its own name, unexplained, instead of as a value this binary claims not to recognise.
    let explanation = match mode {
        CacheMode::Memory => " — results are kept for this process only",
        CacheMode::None => " — every operation of every run is computed",
        _ => "",
    };

    format!("{mode}{explanation}")
}

/// One duration, at a fixed width so the columns line up.
///
/// Sub-second in milliseconds to one decimal, a second or more in seconds to three. Zero renders as `0.0ms` rather
/// than as a dash.
pub fn duration(value: Duration) -> String {
    // By hand because `Duration`'s own `Debug` yields `842.318417ms`, whose width moves with the value and makes a
    // column unreadable.
    //
    // Zero rather than a dash, because the only cell that can legitimately hold it is the deviation of a single timed
    // run — which is zero, not absent. A row that took no measurement at all writes its own dashes; see `row`.
    let rendered = if value < Duration::from_secs(1) {
        format!("{:.1}ms", value.as_secs_f64() * 1_000.0)
    } else {
        format!("{:.3}s", value.as_secs_f64())
    };

    format!("{rendered:>DURATION_WIDTH$}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::sweep::Measured;
    use crate::test_support::{face, sample};
    use clap::Parser;
    use opai::{Faces, Family};

    fn options(arguments: &[&str]) -> Options {
        Cli::try_parse_from(std::iter::once("perftest").chain(arguments.iter().copied()))
            .expect("the arguments parse")
            .options
    }

    fn measured(providers: Verdict) -> Outcome {
        let runs = vec![Duration::from_millis(400), Duration::from_millis(420), Duration::from_millis(410)];

        Outcome::Completed(Box::new(Measured {
            cold: Duration::from_millis(2_400),
            stats: stats::compute(&runs),
            runs,
            output: (2560, 2560),
            providers,
        }))
    }

    fn result(codename: &'static str, outcome: Outcome) -> ModelResult {
        ModelResult { codename, family: Family::Upscale, display_name: codename.to_string(), outcome }
    }

    #[tokio::test]
    async fn the_header_states_every_condition_that_would_change_the_numbers() {
        let input = sample().await;
        let options = options(&["--runs", "7", "--warmup", "2", "--provider", "coreml", "--precision", "fp16"]);

        let rendered = header(&Conditions {
            options: &options,
            input: &input,
            supported: vec![ExecutionProvider::Cpu, ExecutionProvider::CoreMl],
            cache_mode: CacheMode::Disk,
            log: Some(Path::new("/config/logs/opai.log")),
            selected: 3,
            detected: None,
        });

        // The image and its size, so a sweep of the 0.41 MPix sample cannot be confused with one of a photograph.
        assert!(rendered.contains("embedded sample"), "{rendered}");
        assert!(rendered.contains("640x640"), "{rendered}");
        assert!(rendered.contains("0.41 MPix"), "{rendered}");
        assert!(rendered.contains("CoreML"), "{rendered}");
        assert!(rendered.contains("this machine supports: CPU, CoreML"), "{rendered}");
        // The precision, in the library's own spelling.
        assert!(rendered.contains("fp16"), "{rendered}");
        assert!(rendered.contains("7 timed, 2 warm-up"), "{rendered}");
        assert!(rendered.contains("scale 4"), "{rendered}");
        assert!(rendered.contains("strength 1"), "{rendered}");
        assert!(rendered.contains("bias 0"), "{rendered}");
        assert!(rendered.contains("off"), "{rendered}");
        assert!(rendered.contains("store: disk"), "{rendered}");
        assert!(rendered.contains("/config/logs/opai.log"), "{rendered}");
        assert!(rendered.contains("3 selected"), "{rendered}");
    }

    #[tokio::test]
    async fn the_header_states_what_the_auxiliary_detection_run_found() {
        let input = sample().await;
        let options = options(&["athens", "--precision", "fp16"]);

        fn conditions<'a>(options: &'a Options, input: &'a Input, detected: Option<&'a Detected>) -> Conditions<'a> {
            Conditions {
                options,
                input,
                supported: vec![ExecutionProvider::Cpu],
                cache_mode: CacheMode::Disk,
                log: None,
                selected: 1,
                detected,
            }
        }
        let conditions = |detected| conditions(&options, &input, detected);

        let detected = Detected::Found(Faces::new([face(10.0, 20.0, 90.0, 110.0), face(200.0, 40.0, 280.0, 130.0)]));
        let found = header(&conditions(Some(&detected)));

        assert!(found.contains("faces"), "{found}");
        assert!(found.contains("2 detected before the sweep"), "{found}");
        // By the application's own detector, whatever the sweep's precision — the other half of what makes the number
        // comparable, and the one detection every face recovery in the application is fed by.
        assert!(found.contains("New York (FP32)"), "{found}");
        // And that it cost no model anything, which is the claim a reader would otherwise have to take on trust.
        assert!(found.contains("timed in none"), "{found}");

        // A photograph with nobody in it, and a pass that failed: both say the rows were failed rather than
        // measured, which is the outcome the requirement asks for and the opposite of a sub-millisecond success.
        let detected = Detected::Empty;
        let empty = header(&conditions(Some(&detected)));
        assert!(empty.contains("none detected in this image"), "{empty}");
        assert!(empty.contains("failed rather than measured"), "{empty}");

        let detected = Detected::Failed { reason: "no session could be built".to_string() };
        let failed = header(&conditions(Some(&detected)));
        assert!(failed.contains("detection failed"), "{failed}");
        assert!(failed.contains("no session could be built"), "{failed}");
        assert!(failed.contains("failed rather than measured"), "{failed}");
    }

    #[tokio::test]
    async fn a_sweep_that_made_no_auxiliary_run_says_nothing_about_faces() {
        // Rather than saying zero, which would read as a photograph with nobody in it — and a sweep of upscalers has
        // not looked.
        let input = sample().await;

        let rendered = header(&Conditions {
            options: &options(&["kyoto"]),
            input: &input,
            supported: vec![ExecutionProvider::Cpu],
            cache_mode: CacheMode::Disk,
            log: None,
            selected: 1,
            detected: None,
        });

        assert!(!rendered.contains("faces"), "{rendered}");
    }

    #[tokio::test]
    async fn a_machine_with_no_gpu_provider_says_which_providers_it_supports() {
        let input = sample().await;

        let rendered = header(&Conditions {
            options: &options(&[]),
            input: &input,
            supported: vec![ExecutionProvider::Cpu],
            cache_mode: CacheMode::Disk,
            log: None,
            selected: 1,
            detected: None,
        });

        assert!(rendered.contains("this machine supports: CPU"), "{rendered}");
        assert!(!rendered.contains("CUDA"), "{rendered}");
    }

    #[tokio::test]
    async fn a_sweep_with_no_warm_up_says_the_cold_start_may_include_obtaining_the_model() {
        let input = sample().await;

        let conditions = |options| Conditions {
            options,
            input: &input,
            supported: vec![ExecutionProvider::Cpu],
            cache_mode: CacheMode::Disk,
            log: None,
            selected: 1,
            detected: None,
        };

        let warned = options(&["--warmup", "0"]);
        assert!(header(&conditions(&warned)).contains("no warm-up"), "the zero-warm-up warning is missing");

        let ordinary = options(&[]);
        assert!(!header(&conditions(&ordinary)).contains("no warm-up"), "the warning fired with a warm-up");
    }

    #[tokio::test]
    async fn the_header_says_which_of_the_two_things_was_measured() {
        let input = sample().await;

        let conditions = |options| Conditions {
            options,
            input: &input,
            supported: vec![ExecutionProvider::Cpu],
            cache_mode: CacheMode::Memory,
            log: None,
            selected: 1,
            detected: None,
        };

        let off = options(&[]);
        let rendered = header(&conditions(&off));
        assert!(rendered.contains("timings are inference"), "{rendered}");

        let on = options(&["--cache"]);
        let rendered = header(&conditions(&on));
        assert!(rendered.contains("timings include what the cache costs"), "{rendered}");
        // And what is behind the cache, which is a different fact from whether this run used it.
        assert!(rendered.contains("results are kept for this process only"), "{rendered}");
    }

    #[test]
    fn a_duration_renders_at_one_width_in_both_forms() {
        let sub_second = duration(Duration::from_micros(371_300));
        let multi_second = duration(Duration::from_millis(12_400));
        let zero = duration(Duration::ZERO);

        assert_eq!(sub_second.trim(), "371.3ms");
        assert_eq!(multi_second.trim(), "12.400s");
        assert_eq!(zero.trim(), "0.0ms");

        for rendered in [&sub_second, &multi_second, &zero] {
            assert_eq!(rendered.chars().count(), DURATION_WIDTH, "{rendered:?}");
        }
    }

    #[test]
    fn a_duration_of_exactly_one_second_renders_in_seconds() {
        assert_eq!(duration(Duration::from_secs(1)).trim(), "1.000s");
        assert_eq!(duration(Duration::from_millis(999)).trim(), "999.0ms");
    }

    #[test]
    fn a_completed_row_carries_its_statistics_its_output_and_what_it_ran_on() {
        let row = row(&result("kyoto", measured(Verdict::AsRequested(ExecutionProvider::CoreMl))), 0.4096);

        assert_eq!(row[0], "kyoto");
        assert_eq!(row[1], "Upscale");
        assert_eq!(row[2].trim(), "2.400s");
        assert_eq!(row[9], "2560x2560");
        assert_eq!(row[10], "CoreML");
        // Throughput over the input megapixels and the median, not over the 6.5 MPix this row produced.
        assert_eq!(row[8], format!("{:.2}", 0.4096 / 0.410));
    }

    #[test]
    fn a_failed_and_an_interrupted_row_are_told_apart_in_the_table() {
        let failed = row(&result("osaka", Outcome::Failed { reason: "out of memory".to_string() }), 0.4096);
        let stopped = row(&result("tokyo", Outcome::Interrupted), 0.4096);

        assert_eq!(failed.last().map(String::as_str), Some("FAILED"));
        assert_eq!(stopped.last().map(String::as_str), Some("INTERRUPTED"));
        // Neither claims a number.
        assert!(failed[2..10].iter().all(|cell| cell == "-"), "{failed:?}");
        assert!(stopped[2..10].iter().all(|cell| cell == "-"), "{stopped:?}");
    }

    #[test]
    fn a_downgraded_model_is_named_beside_both_providers() {
        let downgraded =
            measured(Verdict::Downgraded { requested: ExecutionProvider::Cuda, actual: vec![ExecutionProvider::Cpu] });

        let rendered = downgrades(&[result("kyoto", downgraded)], ExecutionProvider::Cuda).expect("a warning");

        assert!(rendered.contains("kyoto"), "{rendered}");
        assert!(rendered.contains("CUDA was requested"), "{rendered}");
        assert!(rendered.contains("CPU"), "{rendered}");
        // And that the row is a timing of what ran, which is the whole point of saying anything.
        assert!(rendered.contains("not of what was asked for"), "{rendered}");
    }

    #[test]
    fn a_model_served_on_what_it_asked_for_produces_no_warning() {
        let served = measured(Verdict::AsRequested(ExecutionProvider::CoreMl));

        assert_eq!(downgrades(&[result("kyoto", served)], ExecutionProvider::CoreMl), None);

        // Nor does `Auto`, against which "downgraded" is not a meaningful judgement.
        let resolved = measured(Verdict::Resolved(vec![ExecutionProvider::Cpu]));
        assert_eq!(downgrades(&[result("kyoto", resolved)], ExecutionProvider::Auto), None);
    }

    #[test]
    fn a_row_that_executed_nothing_says_so_rather_than_claiming_the_requested_provider() {
        let nothing = measured(Verdict::NothingExecuted);

        let rendered = downgrades(&[result("kyoto", nothing)], ExecutionProvider::CoreMl).expect("a warning");

        assert!(rendered.contains("nothing was executed"), "{rendered}");
        assert!(rendered.contains("not a timing of CoreML"), "{rendered}");
    }

    #[test]
    fn the_failures_are_listed_with_their_reasons_and_interruptions_are_not() {
        let results = [
            result("osaka", Outcome::Failed { reason: "cold-start run: out of memory".to_string() }),
            result("tokyo", Outcome::Interrupted),
        ];

        let rendered = failures(&results).expect("a failure list");

        assert!(rendered.contains("osaka: cold-start run: out of memory"), "{rendered}");
        assert!(!rendered.contains("tokyo"), "{rendered}");
    }

    #[test]
    fn a_named_decline_is_reported_with_its_reason_and_an_unnamed_one_is_a_count() {
        let named = declines(
            &[result(
                "osaka",
                Outcome::Declined {
                    reason: "no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph"
                        .to_string(),
                },
            )],
            0,
        )
        .expect("a decline list");
        assert!(
            named.contains("osaka: no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph"),
            "{named}"
        );

        let unnamed = declines(&[], 20).expect("a skipped count");
        assert!(unnamed.contains("20 model(s) skipped"), "{unnamed}");
        assert!(unnamed.contains("Name one to see why"), "{unnamed}");
        // And no row for any of them: a refusal is not a measurement and does not belong in a table nobody asked to
        // see it in.
        assert!(!unnamed.contains("osaka"), "{unnamed}");
    }

    #[test]
    fn the_footer_names_the_two_things_a_reader_must_not_assume() {
        let rendered = footer(&options(&[]));

        assert!(rendered.contains("input megapixels over the median run"), "{rendered}");
        assert!(rendered.contains("not across machines or processes"), "{rendered}");
        assert!(rendered.contains("thermal throttling"), "{rendered}");

        // And a third, only when it applies.
        assert!(footer(&options(&["--cache"])).contains("--cache is on"), "the cache footnote is missing");
    }

    #[tokio::test]
    async fn verbose_prints_every_timed_run_in_run_order() {
        let input = sample().await;
        let options = options(&["--verbose"]);

        let sweep = SweepResult {
            results: vec![result("kyoto", measured(Verdict::AsRequested(ExecutionProvider::Cpu)))],
            skipped: 0,
        };

        let conditions = Conditions {
            options: &options,
            input: &input,
            supported: vec![ExecutionProvider::Cpu],
            cache_mode: CacheMode::Disk,
            log: None,
            selected: 1,
            detected: None,
        };
        let rendered = format!("{}{}", header(&conditions), body(&conditions, &sweep));

        assert!(rendered.contains("Timed runs (in order):"), "{rendered}");

        // In the order they were run — 400, 420, 410 — rather than sorted.
        let line = rendered.lines().find(|line| line.trim_start().starts_with("kyoto")).expect("a kyoto line");
        let order: Vec<&str> = line.split_whitespace().skip(1).collect();
        assert_eq!(order, vec!["400.0ms", "420.0ms", "410.0ms"], "{line}");
    }

    #[tokio::test]
    async fn a_declined_model_never_reaches_the_table() {
        let input = sample().await;
        let options = options(&["osaka"]);

        let sweep = SweepResult {
            results: vec![result(
                "osaka",
                Outcome::Declined {
                    reason: "no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph"
                        .to_string(),
                },
            )],
            skipped: 0,
        };

        let conditions = Conditions {
            options: &options,
            input: &input,
            supported: vec![ExecutionProvider::Cpu],
            cache_mode: CacheMode::Disk,
            log: None,
            selected: 1,
            detected: None,
        };
        let rendered = format!("{}{}", header(&conditions), body(&conditions, &sweep));

        // Its reason is reported, which is what naming a model is for...
        assert!(
            rendered.contains("no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph"),
            "{rendered}"
        );
        // ...and it is not a row of dashes in a table of measurements.
        assert!(!rendered.contains("MPIX/S"), "{rendered}");
    }
}
