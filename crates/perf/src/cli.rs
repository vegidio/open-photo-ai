//! The argument surface: what `perftest` accepts, and where each flag's vocabulary comes from.

// Nothing here restates a spelling or a bound the library already publishes. The provider is parsed by
// `ExecutionProvider`'s own `FromStr`, so this binary cannot accept a name the library does not know or refuse one it
// does; the four parameter flags are checked by calling each parameter's own constructor, so none can offer a value
// the library would refuse.

use std::path::PathBuf;
use std::str::FromStr;

use clap::{Parser, Subcommand, ValueEnum};
use opai::{Bias, ExecutionProvider, Fidelity, ParameterEntry, ParameterKind, Precision, Scale, Strength};

/// Benchmarks one inference pass per model against a sample image.
#[derive(Debug, Parser)]
#[command(
    name = "perftest",
    version,
    about = "Benchmark Open Photo AI inference",
    long_about = "Benchmarks one inference pass per model against an embedded 640x640 sample image, reporting the \
cold start (session build plus one inference) and the steady-state distribution over --runs.\n\n\
With no model named, every variant the catalogue publishes is attempted and the ones the library has no pipeline \
for are skipped. Start with \"perftest list\" and a single model name.",
    // Without this, a positional model name and the `list` subcommand are ambiguous to the parser rather than to the
    // reader: `list` is a subcommand, and it takes none of the sweep's flags with it.
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    /// What to do instead of a sweep, where anything was asked for.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Everything a sweep is configured by.
    #[command(flatten)]
    pub options: Options,
}

/// The one thing this binary does that is not a sweep.
#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum Command {
    /// List the models the catalogue publishes.
    List,
}

/// A sweep's fully resolved configuration.
///
/// Every field is past its validator by the time this exists, which is what lets the sweep read them without asking
/// again — and is why the parameter flags' bounds live on the flags rather than in the loop.
#[derive(Debug, Clone, clap::Args)]
pub struct Options {
    /// The models to measure. With none named, every variant the catalogue publishes is attempted.
    #[arg(value_name = "MODEL")]
    pub models: Vec<String>,

    /// Number of timed runs per model.
    #[arg(short = 'n', long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..))]
    pub runs: u32,

    /// Number of untimed warm-up runs; also what downloads a model that is not on disk.
    #[arg(short = 'w', long, default_value_t = 1)]
    pub warmup: u32,

    /// What to run on: auto, cpu, coreml, cuda or tensorrt.
    #[arg(short = 'p', long, default_value = "auto", value_parser = parse_provider)]
    pub provider: ExecutionProvider,

    /// The precision to build every selected model at; not every variant publishes every one.
    #[arg(long, value_enum, default_value_t = PrecisionArg::Fp32)]
    pub precision: PrecisionArg,

    /// Upscale factor, for the families whose published parameter is the scale.
    // `allow_negative_numbers` is on all four parameter flags below: without it clap reads `-0.5` as an unknown
    // short flag, which puts the whole lower half of the bias's published range out of reach and turns an
    // out-of-range value for the other three into "unknown argument" instead of the message naming the range.
    #[arg(short = 's', long, default_value_t = 4.0, value_parser = parse_scale, allow_negative_numbers = true)]
    pub scale: f64,

    /// Blend strength, for the families whose published parameter is the strength.
    #[arg(long, default_value_t = 1.0, value_parser = parse_strength, allow_negative_numbers = true)]
    pub strength: f64,

    /// Blend bias, for the families whose published parameter is the bias.
    #[arg(long, default_value_t = 0.0, value_parser = parse_bias, allow_negative_numbers = true)]
    pub bias: f64,

    /// How closely a face recovery run keeps to the face it was given, for the variant that publishes a fidelity.
    ///
    /// Defaults to the maximum, which is what an untouched control offers — so a sweep that does not name it measures
    /// the run a user gets by default.
    // The maximum is also the reference implementation's hard-coded value.
    #[arg(long, default_value_t = 1.0, value_parser = parse_fidelity, allow_negative_numbers = true)]
    pub fidelity: f64,

    /// Measure a photograph of your own instead of the embedded sample.
    #[arg(long, value_name = "PATH")]
    pub image: Option<PathBuf>,

    /// Keep the run cache on, so a timing covers what a run costs an ordinary caller.
    #[arg(long)]
    pub cache: bool,

    /// Use the model files already on disk without checking them against the published hashes.
    #[arg(long)]
    pub skip_verify: bool,

    /// Report progress as one line per model rather than as a live view.
    ///
    /// Chosen automatically where stderr is not a terminal, and moot under `--json`, which reports no progress at
    /// all, so this is for the case the automatic choice gets wrong: a terminal whose handling of scrolling regions
    /// the view is not welcome on.
    #[arg(long)]
    pub plain: bool,

    /// Write the report as one JSON document to stdout instead of the rendered one.
    #[arg(long)]
    pub json: bool,

    /// Print every timed run in run order.
    #[arg(short = 'v', long)]
    pub verbose: bool,
}

impl Options {
    /// The precision every selected model is built at.
    pub fn precision(&self) -> Precision {
        self.precision.into()
    }

    /// The value of the flag named after `parameter`, or `None` where the parameter is not one a flag can carry.
    ///
    /// `Ok(None)` for a parameter whose kind is not a range.
    ///
    /// # Errors
    ///
    /// [`UnknownParameter`] where the catalogue publishes a range this binary has no flag for.
    pub fn parameter(&self, parameter: &ParameterEntry) -> Result<Option<f64>, UnknownParameter> {
        // A flag carries a number a user types, and the catalogue says of a parameter whose kind is not a range that
        // another operation's result supplies it — a set of pre-detected faces, which no command line can hold. That is
        // not a gap in this binary's flags, so it is not the loud failure below.
        if !matches!(parameter.kind, ParameterKind::Range { .. }) {
            return Ok(None);
        }

        // By the catalogue's own published name rather than by family, which is what makes a family added later with a
        // parameter this binary has no flag for a loud, specific failure instead of a silently wrong number: the four
        // names below are the library's own constants, so a fifth cannot be mistaken for one of them.
        match parameter.name {
            Scale::NAME => Ok(Some(self.scale)),
            Strength::NAME => Ok(Some(self.strength)),
            Bias::NAME => Ok(Some(self.bias)),
            Fidelity::NAME => Ok(Some(self.fidelity)),
            name => Err(UnknownParameter { name: name.to_string() }),
        }
    }
}

/// A parameter the catalogue publishes that this binary has no flag for.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the catalogue publishes a parameter this harness has no flag for: `{name}`")]
pub struct UnknownParameter {
    /// The published name, as the catalogue spells it.
    pub name: String,
}

/// The three precisions, as a flag accepts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PrecisionArg {
    /// 32-bit floating point.
    Fp32,
    /// 16-bit floating point.
    Fp16,
    /// 8-bit integer, which the diffusion upscaler alone publishes.
    Int8,
}

impl From<PrecisionArg> for Precision {
    fn from(argument: PrecisionArg) -> Self {
        // A `ValueEnum` rather than a parse through the library, because `Precision` publishes no `FromStr`. The three
        // spellings are mapped onto its values in this one place, so the flag and the library cannot drift into two
        // vocabularies — and a fourth precision is a compile error here rather than a value this never offers.
        match argument {
            PrecisionArg::Fp32 => Self::Fp32,
            PrecisionArg::Fp16 => Self::Fp16,
            PrecisionArg::Int8 => Self::Int8,
        }
    }
}

/// The provider `text` names, through the library's own parse.
///
/// Its refusal is extended with the published spellings.
fn parse_provider(text: &str) -> Result<ExecutionProvider, String> {
    // Read off `ExecutionProvider::ALL` rather than written down: a sixth provider is offered here by adding it there.
    ExecutionProvider::from_str(text).map_err(|error| {
        let published: Vec<&str> = ExecutionProvider::ALL.iter().map(|provider| provider.as_str()).collect();
        format!("{error} (one of: {})", published.join(", "))
    })
}

/// `text` as a value the parameter's own constructor accepts.
fn parse_scale(text: &str) -> Result<f64, String> {
    // The bound is enforced by calling the constructor rather than by restating its range, which is what keeps the
    // promise this flag makes: a value it accepts is a value the library accepts. A check written again here is a
    // second rule that drifts from the first: `Scale::new` refuses `NaN` by testing containment, while
    // `value < min || value > max` is `false` for it, so a restated range accepts `--scale nan` and quantizes it into
    // an arbitrary integer. And widening a range in the library widens the flag with nothing here edited.
    Scale::new(number(text)?).map(Scale::get).map_err(|error| error.to_string())
}

fn parse_strength(text: &str) -> Result<f64, String> {
    Strength::new(number(text)?).map(Strength::get).map_err(|error| error.to_string())
}

fn parse_bias(text: &str) -> Result<f64, String> {
    Bias::new(number(text)?).map(Bias::get).map_err(|error| error.to_string())
}

/// `text` as a fidelity, refused with the message naming the range the catalogue publishes.
fn parse_fidelity(text: &str) -> Result<f64, String> {
    Fidelity::new(number(text)?).map(Fidelity::get).map_err(|error| error.to_string())
}

/// `text` as a number, before any parameter has an opinion about its value.
fn number(text: &str) -> Result<f64, String> {
    text.parse().map_err(|_| format!("`{text}` is not a number"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use opai::catalogue;

    /// The range the catalogue publishes for the parameter called `name`, or `None` where it publishes none.
    fn bounds(name: &str) -> Option<(f64, f64)> {
        // Read from the catalogue rather than from the constructor the flag calls, so these tests cannot pass by
        // agreeing with the same source the code under test used.
        //
        // Every family publishing a given name publishes it over one range — which is what the library's own
        // catalogue test asserts — so the first row found answers for all of them.
        catalogue()
            .iter()
            .flat_map(|entry| &entry.variants)
            .flat_map(|variant| variant.parameters)
            .find(|parameter| parameter.name == name)
            .and_then(|parameter| match parameter.kind {
                ParameterKind::Range { min, max, .. } => Some((min, max)),
                ParameterKind::Faces => None,
            })
    }

    /// The parse of one command line, as the binary would see it.
    fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("perftest").chain(arguments.iter().copied()))
    }

    /// The options of a command line that parses.
    fn options(arguments: &[&str]) -> Options {
        parse(arguments).expect("the arguments parse").options
    }

    #[test]
    fn the_declared_surface_is_internally_consistent() {
        // clap's own audit of the derive: a duplicated short flag or a default that its own value parser would
        // refuse is a panic here rather than a runtime surprise for whoever runs the binary.
        Cli::command().debug_assert();
    }

    #[test]
    fn the_defaults_are_the_ones_the_readme_documents() {
        let options = options(&[]);

        assert_eq!(options.runs, 5);
        assert_eq!(options.warmup, 1);
        assert_eq!(options.provider, ExecutionProvider::Auto);
        assert_eq!(options.precision(), Precision::Fp32);
        assert_eq!(options.scale, 4.0);
        assert!(options.models.is_empty());
        // Off by default: the library's default is right for an application and wrong for a measurement.
        assert!(!options.cache);
        assert!(!options.skip_verify);
        assert!(!options.json);
        // Off by default, which is what leaves the renderer choice to the terminal test rather than to a flag.
        assert!(!options.plain);
    }

    #[test]
    fn every_provider_the_library_publishes_is_a_spelling_the_flag_accepts() {
        for provider in ExecutionProvider::ALL {
            // Lowercased, which is what anyone actually types: `CoreML` and `TensorRT` are the library's rendering,
            // not a hand position.
            let text = provider.as_str().to_lowercase();

            assert_eq!(options(&["--provider", &text]).provider, provider, "{text}");
        }
    }

    #[test]
    fn a_provider_spelling_the_library_does_not_publish_is_refused_naming_the_published_ones() {
        let error = parse(&["--provider", "openvino"]).expect_err("an unpublished provider is refused");

        let message = error.to_string();
        assert!(message.contains("openvino"), "{message}");
        // The published list, read off the library rather than written down here — which is what makes this fail if
        // a sixth provider is added and the flag's help stops covering it.
        for provider in ExecutionProvider::ALL {
            assert!(message.contains(provider.as_str()), "{message} does not name {provider}");
        }
    }

    #[test]
    fn the_three_precision_spellings_map_onto_the_librarys_values() {
        assert_eq!(options(&["--precision", "fp32"]).precision(), Precision::Fp32);
        assert_eq!(options(&["--precision", "fp16"]).precision(), Precision::Fp16);
        assert_eq!(options(&["--precision", "int8"]).precision(), Precision::Int8);

        parse(&["--precision", "fp64"]).expect_err("a precision the library does not publish is refused");
    }

    #[test]
    fn each_parameter_flag_is_bounded_by_what_the_catalogue_publishes_for_it() {
        let published = |name: &str| bounds(name).unwrap_or_else(|| panic!("the catalogue publishes {name}"));

        for (flag, name) in [("--scale", Scale::NAME), ("--strength", Strength::NAME), ("--bias", Bias::NAME)] {
            let (min, max) = published(name);

            parse(&[flag, &min.to_string()]).unwrap_or_else(|_| panic!("{flag} accepts its published minimum"));
            parse(&[flag, &max.to_string()]).unwrap_or_else(|_| panic!("{flag} accepts its published maximum"));

            parse(&[flag, &(min - 0.5).to_string()]).expect_err("below the published minimum is refused");
            parse(&[flag, &(max + 0.5).to_string()]).expect_err("above the published maximum is refused");
        }
    }

    #[test]
    fn a_value_that_is_not_a_number_is_refused_rather_than_quantized() {
        // The value a restated range lets through — see [`parse_scale`]. The constructor tests containment, so it
        // does not.
        for flag in ["--scale", "--strength", "--bias"] {
            parse(&[flag, "nan"]).unwrap_err();
        }

        // Infinities are out of range like any other value that is not in it.
        parse(&["--scale", "inf"]).unwrap_err();
    }

    #[test]
    fn a_refused_parameter_value_names_the_range_the_catalogue_published() {
        let (min, max) = bounds(Strength::NAME).expect("the catalogue publishes a strength");

        let message = parse(&["--strength", "99"]).expect_err("out of range").to_string();

        assert!(message.contains(Strength::NAME), "{message}");
        assert!(message.contains(&min.to_string()) && message.contains(&max.to_string()), "{message}");
    }

    #[test]
    fn the_parameter_value_is_chosen_by_the_published_name_rather_than_by_the_family() {
        let options = options(&["--scale", "2", "--strength", "1.5", "--bias", "-0.5"]);

        let entry = |name: &'static str| ParameterEntry {
            name,
            kind: ParameterKind::Range { min: 0.0, max: 0.0, default: 0.0 },
        };

        assert_eq!(options.parameter(&entry(Scale::NAME)), Ok(Some(2.0)));
        assert_eq!(options.parameter(&entry(Strength::NAME)), Ok(Some(1.5)));
        assert_eq!(options.parameter(&entry(Bias::NAME)), Ok(Some(-0.5)));
        assert_eq!(options.parameter(&entry(Fidelity::NAME)), Ok(Some(1.0)), "the default is the reference's value");
    }

    #[test]
    fn a_published_parameter_that_is_not_a_range_is_not_a_flag_and_is_not_a_failure() {
        // A set of faces comes from a detection run rather than from a command line, which the catalogue says by
        // publishing its kind. Answering `None` is what lets the selection build such a variant without this binary
        // pretending a flag could carry one.
        let faces = ParameterEntry { name: "faces", kind: ParameterKind::Faces };

        assert_eq!(options(&[]).parameter(&faces), Ok(None));
    }

    #[test]
    fn a_published_parameter_this_harness_has_no_flag_for_fails_loudly() {
        let radius = ParameterEntry { name: "radius", kind: ParameterKind::Range { min: 0.0, max: 1.0, default: 0.0 } };

        let error = options(&[]).parameter(&radius).expect_err("a parameter with no flag is not silently defaulted");

        assert_eq!(error, UnknownParameter { name: "radius".to_string() });
        assert!(error.to_string().contains("radius"), "{error}");
    }

    #[test]
    fn every_parameter_the_catalogue_publishes_today_has_a_flag() {
        // The other half of the test above: the loud failure is a safety net, and this is the assertion that it is
        // not currently firing for anything the library actually publishes.
        for entry in catalogue() {
            for variant in &entry.variants {
                for parameter in variant.parameters {
                    options(&[]).parameter(parameter).unwrap_or_else(|error| {
                        panic!("{:?}'s {} publishes a parameter with no flag: {error}", entry.family, variant.codename)
                    });
                }
            }
        }
    }

    #[test]
    fn a_sweep_of_no_timed_runs_is_refused() {
        // At least one timed run is what the statistics are over; zero would report a distribution of nothing.
        parse(&["--runs", "0"]).expect_err("zero timed runs is refused");
        parse(&["--runs", "1"]).expect("one timed run is the minimum, not a refusal");
    }

    #[test]
    fn a_negative_warm_up_count_is_refused_and_zero_is_not() {
        parse(&["--warmup", "-1"]).expect_err("a negative warm-up count is refused");
        // Zero is a legitimate request — and the one the report has to warn about, since the cold start may then
        // include a download.
        assert_eq!(options(&["--warmup", "0"]).warmup, 0);
    }

    #[test]
    fn listing_is_a_subcommand_and_takes_none_of_the_sweeps_flags() {
        let parsed = parse(&["list"]).expect("list parses");

        assert_eq!(parsed.command, Some(Command::List));
        assert!(parsed.options.models.is_empty());

        // A sweep, by contrast, names its models positionally.
        let sweep = parse(&["kyoto", "osaka"]).expect("model names parse");
        assert_eq!(sweep.command, None);
        assert_eq!(sweep.options.models, vec!["kyoto".to_string(), "osaka".to_string()]);
    }
}
