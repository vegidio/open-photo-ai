//! The machine-readable report.

// Comparing two sweeps of one model across a change to it is the reason this binary exists, and comparing them
// through the rendered table means a person reading two tables.
//
// One document rather than a stream of lines, because the conditions are not per model and a comparison needs them.
// A **schema version** because the first consumer of this will be a script somebody writes six months from now
// against a shape that has since moved. Durations as numbers in milliseconds rather than the strings the table
// shows: `371.3ms` is for reading.

use std::time::Duration;

use opai::{CacheMode, ExecutionProvider, Family};
use serde::{Deserialize, Serialize};

use crate::report::Conditions;
use crate::stats::{self, Stats};
use crate::sweep::{Detected, Measured, ModelResult, Outcome, SweepResult, Verdict};

/// The shape of this document.
///
/// Bumped whenever a consumer written against the previous one would misread this one. A reader that does not
/// recognise the number it finds here should say so rather than guess.
const SCHEMA: u32 = 1;

/// The whole document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// The shape of this document. See [`SCHEMA`].
    pub schema: u32,
    /// The version of the binary that produced it.
    pub tool: String,
    /// What the sweep ran under.
    pub conditions: ConditionsJson,
    /// Every model the report covers, in the order they were measured.
    pub models: Vec<ModelJson>,
}

/// The conditions, carrying the same facts the rendered header states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionsJson {
    /// The image every model was measured against.
    pub image: ImageJson,
    /// What was asked for, beside what this machine can be asked for.
    pub provider: ProviderJson,
    /// The precision every model was built at.
    pub precision: String,
    /// How many runs were timed.
    pub runs: u32,
    /// How many warm-up runs preceded them.
    pub warmup: u32,
    /// The value of each parameter flag.
    pub parameters: ParametersJson,
    /// Whether this sweep used the run cache, and what is behind it.
    pub cache: CacheJson,
    /// Where the library's own records went, or `null` where the sink could not be installed.
    pub log: Option<String>,
    /// How many models were selected.
    pub models_selected: usize,
    /// How many the application declined and the sweep dropped, because the caller named none.
    pub models_skipped: usize,
    // One of the conditions rather than one of the models, for the reason `report::Conditions`'s `detected` gives.
    /// What the auxiliary detection run found, or `null` where the sweep made none.
    pub faces: Option<FacesJson>,
}

/// What the auxiliary detection run found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FacesJson {
    /// `found`, `none` or `failed`.
    pub outcome: String,
    /// How many faces were supplied to every face-recovery row; zero for the other two outcomes.
    pub count: usize,
    /// Why there are none, or `null` where there are.
    pub reason: Option<String>,
}

/// The measured image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageJson {
    /// `embedded sample`, or the path that was named.
    pub source: String,
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
    /// Its megapixels, which the throughput figure is computed over.
    pub megapixels: f64,
}

/// The provider request and what the machine offers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderJson {
    /// What every run asked for.
    pub requested: ExecutionProvider,
    /// What this machine can be asked to run on.
    pub supported: Vec<ExecutionProvider>,
}

/// The values of `--scale`, `--strength` and `--bias`, whichever families the selection covered. `--fidelity` is not
/// recorded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParametersJson {
    /// `--scale`.
    pub scale: f64,
    /// `--strength`.
    pub strength: f64,
    /// `--bias`.
    pub bias: f64,
}

/// Whether the run used the cache, and what is behind it — two different questions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheJson {
    /// Whether this sweep's runs read from and wrote to the cache.
    pub enabled: bool,
    // Serialized straight from `CacheMode`, so a mode added there reaches this document as itself rather than as a
    // word transcribed here.
    /// What is backing it: `disk`, `memory` or `none`, in the library's own spelling.
    pub store: CacheMode,
}

/// One model's outcome.
///
/// Tagged by `outcome`, which is what makes the four distinguishable to a reader that knows nothing else about them.
/// The measurement fields are absent on the three that took none, and `reason` is absent on the one that needs none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelJson {
    /// The variant's codename.
    pub codename: String,
    /// Its family.
    pub family: Family,
    /// The operation's own display name: `Kyoto 4x (FP32)`.
    pub name: String,
    /// One of `completed`, `declined`, `failed` or `interrupted`.
    pub outcome: String,

    /// Why, for the three outcomes that have a reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// The measurement, where there was one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement: Option<MeasurementJson>,
}

/// What a completed model measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementJson {
    /// Session construction plus one inference.
    pub cold_ms: f64,
    /// Every timed run, in run order.
    pub runs_ms: Vec<f64>,
    /// The distribution over them.
    pub statistics: StatisticsJson,
    /// Input megapixels over the median run.
    pub megapixels_per_second: f64,
    /// The width of the image the last run produced.
    pub output_width: u32,
    /// Its height.
    pub output_height: u32,
    /// What the runs were actually executed on.
    pub providers: VerdictJson,
}

/// The five figures over the timed runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatisticsJson {
    /// The fastest run.
    pub min_ms: f64,
    /// The middle run.
    pub median_ms: f64,
    /// The arithmetic mean.
    pub mean_ms: f64,
    /// The slowest run.
    pub max_ms: f64,
    /// The sample standard deviation, Bessel-corrected.
    pub std_dev_ms: f64,
}

/// What the folded provider report says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictJson {
    /// One of `as_requested`, `resolved`, `downgraded` or `nothing_executed`.
    pub verdict: String,
    /// Every provider a session was actually built on, in the order the runs first built on each. Empty where
    /// nothing was executed.
    pub actual: Vec<ExecutionProvider>,
}

/// The document for one sweep.
pub fn document(conditions: &Conditions<'_>, sweep: &SweepResult) -> Document {
    let options = conditions.options;
    let (width, height) = conditions.input.dimensions();

    Document {
        schema: SCHEMA,
        tool: env!("CARGO_PKG_VERSION").to_string(),
        conditions: ConditionsJson {
            image: ImageJson {
                source: conditions.input.label(),
                width,
                height,
                megapixels: conditions.input.megapixels(),
            },
            provider: ProviderJson { requested: options.provider, supported: conditions.supported.clone() },
            precision: options.precision().to_string(),
            runs: options.runs,
            warmup: options.warmup,
            parameters: ParametersJson { scale: options.scale, strength: options.strength, bias: options.bias },
            cache: CacheJson { enabled: options.cache, store: conditions.cache_mode },
            log: conditions.log.map(|path| path.display().to_string()),
            models_selected: conditions.selected,
            models_skipped: sweep.skipped,
            faces: conditions.detected.map(|detected| FacesJson {
                outcome: match detected {
                    Detected::Found(_) => "found",
                    Detected::Empty => "none",
                    Detected::Failed { .. } => "failed",
                }
                .to_string(),
                count: detected.count(),
                reason: detected.usable().err(),
            }),
        },
        models: sweep.results.iter().map(|result| model(result, conditions.input.megapixels())).collect(),
    }
}

/// One model's entry.
fn model(result: &ModelResult, megapixels: f64) -> ModelJson {
    let (outcome, reason, measurement) = match &result.outcome {
        Outcome::Completed(measured) => ("completed", None, Some(measurement(measured, megapixels))),
        Outcome::Declined { reason } => ("declined", Some(reason.clone()), None),
        Outcome::Failed { reason } => ("failed", Some(reason.clone()), None),
        // An interruption has no reason of the library's, and saying nothing at all would leave a consumer to infer
        // one from the tag alone.
        Outcome::Interrupted => ("interrupted", Some("the operator interrupted the sweep".to_string()), None),
    };

    ModelJson {
        codename: result.codename.to_string(),
        family: result.family,
        name: result.display_name.clone(),
        outcome: outcome.to_string(),
        reason,
        measurement,
    }
}

/// One measurement, in milliseconds.
fn measurement(measured: &Measured, megapixels: f64) -> MeasurementJson {
    let Stats { min, max, mean, median, std_dev } = measured.stats;

    MeasurementJson {
        cold_ms: millis(measured.cold),
        runs_ms: measured.runs.iter().map(|run| millis(*run)).collect(),
        statistics: StatisticsJson {
            min_ms: millis(min),
            median_ms: millis(median),
            mean_ms: millis(mean),
            max_ms: millis(max),
            std_dev_ms: millis(std_dev),
        },
        megapixels_per_second: stats::megapixels_per_second(megapixels, median),
        output_width: measured.output.0,
        output_height: measured.output.1,
        providers: verdict(&measured.providers),
    }
}

/// The folded provider report, as one tag and the set it was folded from.
fn verdict(verdict: &Verdict) -> VerdictJson {
    let (tag, actual) = match verdict {
        Verdict::AsRequested(provider) => ("as_requested", vec![*provider]),
        Verdict::Resolved(providers) => ("resolved", providers.clone()),
        Verdict::Downgraded { actual, .. } => ("downgraded", actual.clone()),
        // Empty, and tagged as such: a consumer that read `actual` alone must not be able to mistake this for a run
        // on the requested provider.
        Verdict::NothingExecuted => ("nothing_executed", Vec::new()),
    };

    VerdictJson { verdict: tag.to_string(), actual }
}

/// One duration as milliseconds.
fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

/// The document as the one thing written to stdout, pretty-printed.
///
/// # Errors
///
/// [`serde_json::Error`] where the document could not be serialized, which nothing in this shape can produce today.
pub fn render(document: &Document) -> Result<String, serde_json::Error> {
    // Pretty-printed, because the first thing anyone does with it is read it, and a diff between two sweeps is easier
    // on lines than on one of them. Reported rather than unwrapped, because a sweep's numbers should not be lost to an
    // `unwrap`.
    serde_json::to_string_pretty(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Options};
    use crate::input::Input;
    use crate::report::Conditions;
    use crate::test_support::{face, sample};
    use clap::Parser;
    use opai::{Faces, Family};
    use std::path::Path;

    fn options(arguments: &[&str]) -> Options {
        Cli::try_parse_from(std::iter::once("perftest").chain(arguments.iter().copied()))
            .expect("the arguments parse")
            .options
    }

    fn result(codename: &'static str, outcome: Outcome) -> ModelResult {
        ModelResult { codename, family: Family::Upscale, display_name: format!("{codename} 4x (FP32)"), outcome }
    }

    fn measured(providers: Verdict) -> Outcome {
        let runs = vec![Duration::from_millis(400), Duration::from_millis(420)];

        Outcome::Completed(Box::new(Measured {
            cold: Duration::from_millis(2_400),
            stats: stats::compute(&runs),
            runs,
            output: (2560, 2560),
            providers,
        }))
    }

    /// A sweep carrying one of each of the four outcomes, which is what the document has to keep apart.
    fn all_four() -> SweepResult {
        SweepResult {
            results: vec![
                result("kyoto", measured(Verdict::AsRequested(ExecutionProvider::Cpu))),
                result(
                    "osaka",
                    Outcome::Declined {
                        reason: "no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph"
                            .to_string(),
                    },
                ),
                result("saitama", Outcome::Failed { reason: "cold-start run: out of memory".to_string() }),
                result("tokyo", Outcome::Interrupted),
            ],
            skipped: 3,
        }
    }

    async fn documented(options: &Options, sweep: &SweepResult) -> (Document, Input) {
        with_faces_documented(options, sweep, None).await
    }

    async fn with_faces_documented(
        options: &Options,
        sweep: &SweepResult,
        detected: Option<&Detected>,
    ) -> (Document, Input) {
        let input = sample().await;

        let document = {
            let conditions = Conditions {
                options,
                input: &input,
                supported: vec![ExecutionProvider::Cpu, ExecutionProvider::CoreMl],
                cache_mode: CacheMode::Disk,
                log: Some(Path::new("/config/logs/opai.log")),
                selected: 4,
                detected,
            };

            document(&conditions, sweep)
        };

        (document, input)
    }

    #[tokio::test]
    async fn the_document_round_trips_through_serde_json() {
        let options = options(&["--runs", "2"]);
        let (document, _input) = documented(&options, &all_four()).await;

        let rendered = render(&document).expect("the document serializes");
        let parsed: Document = serde_json::from_str(&rendered).expect("the document parses back");

        assert_eq!(parsed, document);
    }

    #[tokio::test]
    async fn the_document_carries_the_conditions_the_header_states() {
        let options = options(&["--runs", "2", "--warmup", "3", "--precision", "fp16", "--cache"]);
        let (document, _input) = documented(&options, &all_four()).await;

        assert_eq!(document.schema, SCHEMA);
        assert_eq!(document.tool, env!("CARGO_PKG_VERSION"));

        let conditions = &document.conditions;
        assert_eq!(conditions.image.source, "embedded sample");
        assert_eq!((conditions.image.width, conditions.image.height), (640, 640));
        assert_eq!(conditions.precision, "fp16");
        assert_eq!((conditions.runs, conditions.warmup), (2, 3));
        assert_eq!(conditions.provider.requested, ExecutionProvider::Auto);
        assert_eq!(conditions.provider.supported, vec![ExecutionProvider::Cpu, ExecutionProvider::CoreMl]);
        assert!(conditions.cache.enabled);
        assert_eq!(conditions.cache.store, CacheMode::Disk);
        assert_eq!(conditions.log.as_deref(), Some("/config/logs/opai.log"));
        assert_eq!(conditions.models_selected, 4);
        // The count a rendered report states in a sentence, as a number a script can compare.
        assert_eq!(conditions.models_skipped, 3);
    }

    #[tokio::test]
    async fn each_of_the_four_outcomes_is_distinguishable_and_carries_its_reason() {
        let options = options(&["--runs", "2"]);
        let (document, _input) = documented(&options, &all_four()).await;

        let entry = |codename: &str| {
            document
                .models
                .iter()
                .find(|model| model.codename == codename)
                .expect("the model is in the document")
        };

        assert_eq!(entry("kyoto").outcome, "completed");
        assert_eq!(entry("kyoto").reason, None);
        assert!(entry("kyoto").measurement.is_some());

        assert_eq!(entry("osaka").outcome, "declined");
        assert_eq!(
            entry("osaka").reason.as_deref(),
            Some("no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph")
        );
        // A refusal took no measurement, so it claims none.
        assert!(entry("osaka").measurement.is_none());

        assert_eq!(entry("saitama").outcome, "failed");
        assert_eq!(entry("saitama").reason.as_deref(), Some("cold-start run: out of memory"));

        assert_eq!(entry("tokyo").outcome, "interrupted");
        assert!(entry("tokyo").reason.is_some(), "an interruption says why it has no numbers");
    }

    #[tokio::test]
    async fn the_durations_are_numbers_in_milliseconds_rather_than_the_strings_the_table_shows() {
        let options = options(&["--runs", "2"]);
        let (document, _input) = documented(&options, &all_four()).await;

        let measurement = document.models[0].measurement.as_ref().expect("kyoto completed").clone();

        assert_eq!(measurement.cold_ms, 2_400.0);
        assert_eq!(measurement.runs_ms, vec![400.0, 420.0]);
        assert_eq!(measurement.statistics.min_ms, 400.0);
        assert_eq!(measurement.statistics.max_ms, 420.0);
        assert_eq!(measurement.statistics.median_ms, 410.0);
        assert_eq!((measurement.output_width, measurement.output_height), (2560, 2560));

        let rendered = render(&document).expect("the document serializes");
        let value: serde_json::Value = serde_json::from_str(&rendered).expect("the document parses");
        let measured = &value["models"][0]["measurement"];

        assert!(measured["cold_ms"].is_number(), "{measured}");
        assert!(measured["statistics"]["median_ms"].is_number(), "{measured}");
        assert!(measured["runs_ms"].as_array().expect("an array").iter().all(serde_json::Value::is_number));
    }

    #[tokio::test]
    async fn a_downgrade_and_an_empty_provider_set_are_told_apart_in_the_document() {
        let options = options(&["--runs", "2", "--provider", "cuda"]);

        let sweep = SweepResult {
            results: vec![
                result(
                    "kyoto",
                    measured(Verdict::Downgraded {
                        requested: ExecutionProvider::Cuda,
                        actual: vec![ExecutionProvider::Cpu],
                    }),
                ),
                result("tokyo", measured(Verdict::NothingExecuted)),
            ],
            skipped: 0,
        };

        let (document, _input) = documented(&options, &sweep).await;

        let providers = |index: usize| document.models[index].measurement.as_ref().expect("measured").providers.clone();

        assert_eq!(
            providers(0),
            VerdictJson { verdict: "downgraded".to_string(), actual: vec![ExecutionProvider::Cpu] }
        );
        // Empty and tagged, so a consumer reading `actual` alone cannot mistake it for a CUDA run.
        assert_eq!(providers(1), VerdictJson { verdict: "nothing_executed".to_string(), actual: Vec::new() });
    }

    #[tokio::test]
    async fn auto_is_reported_as_what_it_resolved_to() {
        let options = options(&["--runs", "2"]);

        let sweep = SweepResult {
            results: vec![result("kyoto", measured(Verdict::Resolved(vec![ExecutionProvider::CoreMl])))],
            skipped: 0,
        };

        let (document, _input) = documented(&options, &sweep).await;

        assert_eq!(
            document.models[0].measurement.as_ref().expect("measured").providers,
            VerdictJson { verdict: "resolved".to_string(), actual: vec![ExecutionProvider::CoreMl] }
        );
    }

    #[tokio::test]
    async fn the_document_states_what_the_auxiliary_detection_run_found() {
        let options = options(&["--runs", "2"]);
        let sweep = all_four();

        let (none, _input) = with_faces_documented(&options, &sweep, None).await;
        assert_eq!(none.conditions.faces, None, "a sweep that looked for no faces reported a count");

        let faces = Faces::new([face(10.0, 20.0, 90.0, 110.0), face(200.0, 40.0, 280.0, 130.0)]);
        let detected = Detected::Found(faces);
        let (found, _input) = with_faces_documented(&options, &sweep, Some(&detected)).await;

        assert_eq!(found.conditions.faces, Some(FacesJson { outcome: "found".to_string(), count: 2, reason: None }));

        // The two answers that are not a set of faces keep the reason, so a consumer can say why those rows failed
        // without parsing a row's own message.
        let detected = Detected::Empty;
        let (empty, _input) = with_faces_documented(&options, &sweep, Some(&detected)).await;
        let reported = empty.conditions.faces.expect("an empty pass is reported");

        assert_eq!(reported.outcome, "none");
        assert_eq!(reported.count, 0);
        assert!(reported.reason.is_some_and(|reason| reason.contains("no face was detected")));

        let detected = Detected::Failed { reason: "no session could be built".to_string() };
        let (failed, _input) = with_faces_documented(&options, &sweep, Some(&detected)).await;
        let reported = failed.conditions.faces.expect("a failed pass is reported");

        assert_eq!(reported.outcome, "failed");
        assert!(reported.reason.is_some_and(|reason| reason.contains("no session could be built")));
    }
}
