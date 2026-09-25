//! Running the window's chain over an open photograph at its own depth, and writing the result to a file.
//!
//! [`export_with`] is the whole of what the `export` command does once the library handle has been taken off managed
//! state, and the order of what it does is its contract.
//!
//! Its records name no run: they are emitted inside the `export` command's span, which carries it, and the file writes
//! it onto each of them from there. A destination that cannot be claimed or committed is not recorded here either; the
//! command wrapper records it, once. See `crate::command`.

use std::path::PathBuf;

use opai::{Enhanced, InferenceError, OutputDepth, ProcessOptions};
use serde::Serialize;

use super::Exports;
use super::destination;
use super::format::{ExportFormat, encode_options};
use super::progress::ExportReporting;
use crate::command::{Answer, CommandError, Ended};
use crate::enhance::{Enhancer, Processor, Requested, UnknownOperation};
use crate::images::{Crop, Opened, load_framed};

/// How an export ended.
///
/// A stop is **not** a failure, exactly as a canvas run's is not: the window tells the two apart by this tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub(crate) enum Exported {
    /// The file was written.
    Exported {
        /// The path actually written: the destination asked for, or the numbered name beside it. What a reveal needs.
        path: String,
        /// How many bytes were written.
        bytes: u64,
    },
    /// The export was stopped before its file was written. Nothing is left at any name it tried.
    Stopped,
}

impl Answer for Exported {
    fn ended(&self) -> Ended<'_> {
        match self {
            Self::Exported { .. } => Ended::Finished,
            Self::Stopped => Ended::Stopped,
        }
    }
}

/// Why an export could not be asked for, or could not finish. A stop is deliberately not among these — see
/// [`Exported`].
///
/// The first five have the shapes [`EnhanceError`](crate::enhance::EnhanceError)'s do, so the window can describe
/// both with one formatter.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum ExportError {
    /// The window asked before the application had finished starting itself.
    #[error("the application is still starting and cannot export a photograph yet")]
    NotReady,

    /// The source names an identity this application never admitted — a bug in the interface.
    #[error("no image with identity `{identity}` has been opened")]
    UnknownSource {
        /// What was asked for.
        identity: String,
    },

    /// An operation in the chain names something this application cannot run.
    #[error("operation {index} cannot be run: {reason}")]
    UnknownOperation {
        /// Which position in the chain, counted from zero.
        index: usize,
        /// What could not be served.
        reason: UnknownOperation,
    },

    /// The source was admitted and could not be read: a photograph on a drive that has been ejected.
    #[error("`{identity}` was opened and can no longer be read: {message}")]
    UnreadableSource {
        /// Which admitted file.
        identity: String,
        /// The library's own sentence, untranslated, so it can be copied into a bug report.
        message: String,
    },

    /// The chain itself failed; `message` is [`InferenceError`]'s own sentence.
    #[error("{message}")]
    Enhance {
        /// The library's own sentence, untranslated and in full.
        message: String,
    },

    /// The encoded photograph could not be saved under the name claimed for it.
    #[error("`{path}` could not be written: {message}")]
    Write {
        /// The destination the window asked for, whatever numbered name was being tried.
        path: String,
        /// The reason, untranslated: what the failed row's tooltip copies to the clipboard.
        message: String,
    },

    // Its own variant rather than a second use of `Write`, because the two are recorded by different crates: `opai`
    // records a failed save, and nothing but the command wrapper sees a failed claim or commit. The window cannot tell
    // them apart and has no reason to, so it crosses under `Write`'s tag in `Write`'s shape.
    /// A name could not be claimed at the destination, or the written file could not be put in its place.
    #[error("`{path}` could not be written: {message}")]
    #[serde(rename = "write")]
    Unwritable {
        /// The destination the window asked for, whatever numbered name was being tried.
        path: String,
        /// The reason, untranslated: what the failed row's tooltip copies to the clipboard.
        message: String,
    },
}

impl CommandError for ExportError {
    fn ended(&self) -> Ended<'_> {
        match self {
            // Refusals and failures this crate makes, which nothing else sees.
            Self::NotReady | Self::UnknownSource { .. } | Self::UnknownOperation { .. } | Self::Unwritable { .. } => {
                Ended::Failed { error: self, recorded: false }
            }
            // `opai` records the failed decode, the failed chain and the failed save, where each happened.
            Self::UnreadableSource { .. } | Self::Enhance { .. } | Self::Write { .. } => {
                Ended::Failed { error: self, recorded: true }
            }
        }
    }
}

/// What one request to export carries, gathered so the seam below takes one argument rather than nine.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// The window's name for this export, minted before the call — what a stop and every report name.
    pub(crate) run: String,
    /// The identity of the image to export, as the window already addresses its pixels by.
    pub(crate) source: String,
    /// The chain to run, in order. Empty writes the framed photograph as it is.
    pub(crate) operations: Vec<Requested>,
    /// What to run on.
    pub(crate) processor: Processor,
    /// How the image is framed, or `None` to export the whole photograph.
    pub(crate) crop: Option<Crop>,
    /// Where to write. Its name does not decide the format; `format` does.
    pub(crate) destination: PathBuf,
    /// What to write.
    pub(crate) format: ExportFormat,
    /// The quality, for the formats that take one. Brought inside `1..=100` rather than refused.
    pub(crate) quality: f64,
    /// Whether a file already at `destination` may be replaced. Without it, a numbered name is written instead.
    pub(crate) overwrite: bool,
}

/// Runs a chain over an open photograph at its own depth, writes the result, and answers where and how much.
///
/// # The order of what happens is the contract
///
/// ```text
///   1. judge the whole chain       --> refused: nothing fetched, nothing read
///   2. resolve the source          --> refused: unknown source
///   3. register the export         --> a stop that arrived first answers `stopped`, nothing read
///   4. decode and frame it         --> refused: unreadable source
///   5. run the chain, at the source's depth, reporting as `enhancing`
///   6. release the held-back report; stopped?  --> `stopped`. The last point a stop takes effect.
///   7. report `writing`
///   8. claim the destination       --> refused: unwritable, naming it
///   9. write                       --> refused: unwritable; the claim gives the name back
///  10. commit the claim             --> refused: unwritable; an overwrite's destination left as it was
///  11. answer its path and bytes
/// ```
///
/// **No slot is written.** [`Runs`](crate::enhance::Runs) is not touched, so an export can neither stop the run the
/// window is drawing nor drop the result it holds, and a canvas run cannot stop an export.
///
/// **The name is claimed after the chain**, not before it, so a long export holds no empty file a user could mistake
/// for a finished one, and a stop during the chain has nothing to clean up.
///
/// **A stop once writing has begun is not honoured.** The write is short beside a chain, and a half-written file a
/// stop then had to clean up is the worse outcome. See design.md D3.
///
/// # Errors
///
/// [`ExportError`]. A stop is not one of them: it is [`Exported::Stopped`].
pub(crate) async fn export_with<E: Enhancer>(
    enhancer: &E,
    opened: &Opened,
    exports: &Exports,
    reporting: Option<ExportReporting>,
    request: Request,
) -> Result<Exported, ExportError> {
    // The whole chain, before any of it begins, so a bad second operation doesn't first fetch a model for the first.
    let operations = request
        .operations
        .iter()
        .enumerate()
        .map(|(index, requested)| requested.resolve().map_err(|reason| ExportError::UnknownOperation { index, reason }))
        .collect::<Result<Vec<_>, _>>()?;

    let path = opened
        .resolve(&request.source)
        .ok_or_else(|| ExportError::UnknownSource { identity: request.source.clone() })?;

    let Some(registration) = exports.register(&request.run) else {
        tracing::debug!("an export was stopped before it started");

        return Ok(Exported::Stopped);
    };

    // Not logged here, nor the chain's failure or the save's below: `opai` records each once, where it happened.
    let picture = load_framed(path, request.crop).await.map_err(|error| ExportError::UnreadableSource {
        identity: request.source.clone(),
        message: error.to_string(),
    })?;

    // `OutputDepth::Source`, unlike the canvas's `Eight`: this result is written to a file rather than drawn, and a
    // 16-bit photograph written to a format that holds 16 bits keeps them.
    let options = ProcessOptions {
        provider: request.processor.into(),
        depth: OutputDepth::Source,
        on_progress: reporting.as_ref().map(|reporting| std::sync::Arc::clone(&reporting.enhancing.on_progress)),
        cancel: registration.cancel().clone(),
        ..Default::default()
    };

    let outcome = enhancer.process(&picture, &operations, options).await;

    // Released whatever the chain did, so the row lands where the chain actually got to.
    if let Some(reporting) = &reporting {
        reporting.enhancing.release();
    }

    // Cancellation is cooperative, so a chain whose every operation was already known can answer `Ok` without having
    // noticed a stop — and a decode, which is not cancellable, can have absorbed one. Either way the window has
    // withdrawn the request, and this is the last point it can.
    if registration.cancel().is_cancelled() {
        tracing::debug!("an export was stopped before it was written");

        return Ok(Exported::Stopped);
    }

    let picture = match outcome {
        Ok(Enhanced { picture, .. }) => picture,
        Err(InferenceError::Cancelled | InferenceError::Shutdown) => {
            tracing::debug!("an export was stopped while its chain ran");

            return Ok(Exported::Stopped);
        }
        Err(error) => return Err(ExportError::Enhance { message: error.to_string() }),
    };

    if let Some(reporting) = &reporting {
        reporting.writing();
    }

    // Claiming and committing the destination are this application's own failures, which the command wrapper records;
    // the save between them is `opai`'s, which records it itself. See `ExportError::ended`.
    let path = || request.destination.display().to_string();
    let unwritten = |message: String| ExportError::Write { path: path(), message };
    let unwritable = |message: String| ExportError::Unwritable { path: path(), message };

    // Off the runtime's own threads: up to a thousand exclusive creates, on whatever drive the user chose.
    let requested = request.destination.clone();
    let claim = crate::task::spawn_blocking(move || destination::claim(&requested, request.overwrite))
        .await
        .expect("claiming a name cannot panic, and nothing cancels its thread")
        .map_err(|error| unwritable(error.to_string()))?;

    let options = encode_options(request.format, request.quality);
    let bytes = opai::image::save(picture.shared_pixels(), claim.path(), request.format.into(), Some(options))
        .await
        .map_err(|error| unwritten(error.to_string()))?;

    // Overwriting, this is where the destination is replaced, and not before: a failed write above left it untouched.
    let written = claim.commit().map_err(|error| unwritable(error.to_string()))?;
    tracing::info!(path = %written.display(), bytes, "a photograph was exported");

    Ok(Exported::Exported { path: written.to_string_lossy().into_owned(), bytes })
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use opai::{ExecutionProvider, Opai, Picture, Precision};

    use super::*;
    use crate::export::progress::{ExportProgress, collecting};
    use crate::test_support::admitted;

    /// What a fake enhancer hands back: `pixels` at `identity`, on the source's own path.
    fn enhanced(source: &Picture, pixels: image::DynamicImage, identity: &str) -> Enhanced {
        Enhanced {
            picture: Picture::new(source.path(), pixels, identity),
            providers: opai::ProviderReport { requested: ExecutionProvider::Auto, actual: Vec::new() },
        }
    }

    /// An enhancer that doubles the picture for a chain and answers the source itself for an empty one — `opai`'s own
    /// rule — recording what it was handed and reporting as a real run does.
    #[derive(Default)]
    struct Recording {
        /// The length of each chain, the provider and the depth, one entry per call.
        seen: Mutex<Vec<(usize, ExecutionProvider, OutputDepth)>>,
    }

    impl Recording {
        fn seen(&self) -> Vec<(usize, ExecutionProvider, OutputDepth)> {
            self.seen.lock().expect("unpoisoned").clone()
        }
    }

    impl Enhancer for Recording {
        fn process(
            &self,
            source: &Picture,
            operations: &[opai::Operation],
            options: ProcessOptions,
        ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
            self.seen.lock().expect("unpoisoned").push((operations.len(), options.provider, options.depth));

            if let Some(on_progress) = &options.on_progress {
                for (index, operation) in operations.iter().enumerate() {
                    let subject = Arc::new(opai::Subject::Enhancement(operation.clone()));

                    for step in 0..=10 {
                        let chain = (index as f64 + f64::from(step) / 10.0) / operations.len() as f64;

                        on_progress(&opai::InferenceProgress {
                            subject: Arc::clone(&subject),
                            stage: opai::Stage::Running,
                            operation_fraction: f64::from(step) / 10.0,
                            chain_fraction: chain,
                        });
                    }
                }
            }

            let result = if operations.is_empty() {
                enhanced(source, (*source.shared_pixels()).clone(), source.identity())
            } else {
                let (width, height) = source.dimensions();

                enhanced(source, image::DynamicImage::new_rgb8(width * 2, height * 2), "cafebabecafebabe")
            };

            async move { Ok(result) }
        }
    }

    /// An enhancer whose chain fails for a reason that is not a stop.
    struct Failing;

    impl Enhancer for Failing {
        async fn process(
            &self,
            _source: &Picture,
            _operations: &[opai::Operation],
            _options: ProcessOptions,
        ) -> Result<Enhanced, InferenceError> {
            Err(InferenceError::Untileable { width: 0, height: 0 })
        }
    }

    /// An enhancer that stops its own export while working, then answers anyway — the fully-cached chain that never
    /// looked at its token.
    struct StoppedMidway<'a>(&'a Exports, &'static str, Recording);

    impl Enhancer for StoppedMidway<'_> {
        fn process(
            &self,
            source: &Picture,
            operations: &[opai::Operation],
            options: ProcessOptions,
        ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
            self.0.stop(self.1);

            self.2.process(source, operations, options)
        }
    }

    /// One upscale the catalogue publishes, as the window would name it.
    fn kyoto() -> Requested {
        Requested::Upscale { codename: "kyoto".to_string(), precision: Precision::Fp32, scale: 2.0 }
    }

    /// The opened photograph, its identity, and an empty directory to export into.
    struct Fixture {
        source: tempfile::TempDir,
        opened: Opened,
        exports: Exports,
        identity: String,
        out: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let source = tempfile::tempdir().expect("a temporary directory");
            let opened = Opened::default();
            let identity = admitted(&source, &opened);

            Self {
                source,
                opened,
                exports: Exports::default(),
                identity,
                out: tempfile::tempdir().expect("a temporary directory"),
            }
        }

        /// A request to export the fixture through `operations` to `name` in the output directory, as PNG, without
        /// overwriting.
        fn request(&self, run: &str, operations: Vec<Requested>, name: &str) -> Request {
            Request {
                run: run.to_string(),
                source: self.identity.clone(),
                operations,
                processor: Processor::Coreml,
                crop: None,
                destination: self.out.path().join(name),
                format: ExportFormat::Png,
                quality: 90.0,
                overwrite: false,
            }
        }

        /// [`export_with`] driven to completion.
        fn export<E: Enhancer>(
            &self,
            enhancer: &E,
            reporting: Option<ExportReporting>,
            request: Request,
        ) -> Result<Exported, ExportError> {
            tauri::async_runtime::block_on(export_with(enhancer, &self.opened, &self.exports, reporting, request))
        }

        /// The names in the output directory, sorted.
        fn written(&self) -> Vec<String> {
            let mut names: Vec<_> = std::fs::read_dir(self.out.path())
                .expect("listable")
                .map(|entry| entry.expect("readable").file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();

            names
        }
    }

    /// `pixels` written to `path` as `format` and admitted beside the fixture's photograph, answering its identity.
    fn admit(fixture: &Fixture, path: &Path, pixels: &image::DynamicImage, format: opai::ImageFormat) -> String {
        opai::image::save_blocking(pixels, path, format, None).expect("writable");
        tauri::async_runtime::block_on(crate::images::files::describe_and_admit(
            vec![path.to_path_buf()],
            &fixture.opened,
        ))
        .expect("admissible");

        opai::image::identity_blocking(path).expect("a written file has an identity")
    }

    /// The dimensions and colour type of the image file at `path`.
    fn decoded(path: impl AsRef<Path>) -> ((u32, u32), image::ColorType) {
        let picture = opai::image::load_blocking(path.as_ref()).expect("the export is a readable image");

        (picture.dimensions(), picture.pixels().color())
    }

    // ── What is written ───────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_chain_is_run_at_the_sources_depth_and_its_result_written() {
        let fixture = Fixture::new();
        let enhancer = Recording::default();

        let answer = fixture
            .export(&enhancer, None, fixture.request("export-1", vec![kyoto()], "photo-opai.png"))
            .expect("an admitted source and a published chain export");

        let destination = fixture.out.path().join("photo-opai.png");
        assert_eq!(
            answer,
            Exported::Exported {
                path: destination.to_string_lossy().into_owned(),
                bytes: std::fs::metadata(&destination).expect("written").len(),
            }
        );
        assert_eq!(decoded(&destination).0, (16, 12), "the chain's result was not what was written");
        assert_eq!(enhancer.seen(), [(1, ExecutionProvider::CoreMl, OutputDepth::Source)]);
    }

    #[test]
    fn an_empty_chain_writes_the_framed_photograph_as_it_is() {
        let fixture = Fixture::new();
        let crop = Crop::new(1, 1, 4, 3, 0, false, false).expect("a rectangle with area");

        fixture
            .export(
                &Recording::default(),
                None,
                Request { crop: Some(crop), ..fixture.request("export-1", Vec::new(), "photo-opai.png") },
            )
            .expect("an empty chain is a request, not a mistake");

        assert_eq!(decoded(fixture.out.path().join("photo-opai.png")).0, (4, 3), "the framing was not applied");
    }

    #[test]
    fn the_format_decides_the_bytes_whatever_the_name_ends_in() {
        let fixture = Fixture::new();

        fixture
            .export(
                &Recording::default(),
                None,
                Request { format: ExportFormat::Jpeg, ..fixture.request("export-1", Vec::new(), "photo-opai.png") },
            )
            .expect("exportable");

        let bytes = std::fs::read(fixture.out.path().join("photo-opai.png")).expect("written");
        assert_eq!(&bytes[..3], [0xFF, 0xD8, 0xFF], "a JPEG export named .png was not written as JPEG");
    }

    #[test]
    fn a_sixteen_bit_photograph_keeps_its_depth_where_the_format_holds_it() {
        let fixture = Fixture::new();
        let deep = fixture.source.path().join("deep.png");
        let pixels = image::DynamicImage::ImageRgb16(image::ImageBuffer::from_fn(6, 4, |x, y| {
            image::Rgb([(x * 10_000) as u16, (y * 15_000) as u16, 40_000])
        }));
        let identity = admit(&fixture, &deep, &pixels, opai::ImageFormat::Png);

        for (format, name, color) in [
            (ExportFormat::Png, "deep-opai.png", image::ColorType::Rgb16),
            (ExportFormat::Jpeg, "deep-opai.jpg", image::ColorType::Rgb8),
        ] {
            fixture
                .export(
                    &Recording::default(),
                    None,
                    Request { source: identity.clone(), format, ..fixture.request("export-1", Vec::new(), name) },
                )
                .expect("exportable");

            assert_eq!(decoded(fixture.out.path().join(name)).1, color, "{format:?}");
        }
    }

    #[test]
    fn an_empty_chain_over_a_jpeg_writes_a_webp_at_the_photographs_own_dimensions() {
        let fixture = Fixture::new();
        let jpeg = fixture.source.path().join("holiday.jpg");
        let pixels = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(10, 7, |x, y| {
            image::Rgb([(x * 20) as u8, (y * 30) as u8, 128])
        }));
        let identity = admit(&fixture, &jpeg, &pixels, opai::ImageFormat::Jpeg);

        fixture
            .export(
                &Recording::default(),
                None,
                Request {
                    source: identity,
                    format: ExportFormat::Webp,
                    ..fixture.request("export-1", Vec::new(), "holiday-opai.webp")
                },
            )
            .expect("an empty chain is a request, not a mistake");

        let written = fixture.out.path().join("holiday-opai.webp");
        let bytes = std::fs::read(&written).expect("written");
        assert_eq!((&bytes[..4], &bytes[8..12]), (&b"RIFF"[..], &b"WEBP"[..]), "the export was not written as WebP");
        assert_eq!(decoded(&written).0, (10, 7));
    }

    #[test]
    fn a_lower_quality_writes_a_smaller_lossy_file_and_changes_nothing_for_a_lossless_one() {
        let fixture = Fixture::new();
        // Textured and large enough that the encoded data, not the headers, decides the size.
        let textured = fixture.source.path().join("textured.png");
        let pixels = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(128, 96, |x, y| {
            let noise = (x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)).wrapping_mul(2_246_822_519) >> 24;
            image::Rgb([noise as u8, (x * 2) as u8, (y * 2) as u8])
        }));
        let identity = admit(&fixture, &textured, &pixels, opai::ImageFormat::Png);

        let size = |format: ExportFormat, quality: f64, name: &str| {
            fixture
                .export(
                    &Recording::default(),
                    None,
                    Request {
                        source: identity.clone(),
                        format,
                        quality,
                        ..fixture.request("export-1", Vec::new(), name)
                    },
                )
                .expect("exportable");

            let written = fixture.out.path().join(name);
            assert_eq!(decoded(&written).0, (128, 96), "{name}");

            std::fs::metadata(written).expect("written").len()
        };

        assert!(
            size(ExportFormat::Jpeg, 30.0, "q30.jpg") < size(ExportFormat::Jpeg, 90.0, "q90.jpg"),
            "a JPEG at quality 30 was not smaller than one at 90"
        );
        assert_eq!(
            size(ExportFormat::Png, 30.0, "q30.png"),
            size(ExportFormat::Png, 90.0, "q90.png"),
            "the quality changed a PNG"
        );
    }

    #[test]
    fn a_taken_destination_is_numbered_and_the_answer_names_what_was_written() {
        let fixture = Fixture::new();
        std::fs::write(fixture.out.path().join("photo-opai.png"), b"an earlier export").expect("writable");

        let answer = fixture
            .export(&Recording::default(), None, fixture.request("export-1", vec![kyoto()], "photo-opai.png"))
            .expect("exportable");

        let numbered = fixture.out.path().join("photo-opai_1.png");
        assert_eq!(
            answer,
            Exported::Exported {
                path: numbered.to_string_lossy().into_owned(),
                bytes: std::fs::metadata(&numbered).expect("written").len(),
            }
        );
        assert_eq!(
            std::fs::read(fixture.out.path().join("photo-opai.png")).expect("readable"),
            b"an earlier export"
        );
    }

    #[test]
    fn overwriting_replaces_the_destination_and_numbers_nothing() {
        let fixture = Fixture::new();
        std::fs::write(fixture.out.path().join("photo-opai.png"), b"an earlier export").expect("writable");

        fixture
            .export(
                &Recording::default(),
                None,
                Request { overwrite: true, ..fixture.request("export-1", vec![kyoto()], "photo-opai.png") },
            )
            .expect("exportable");

        assert_eq!(fixture.written(), ["photo-opai.png"]);
        assert_eq!(decoded(fixture.out.path().join("photo-opai.png")).0, (16, 12), "the earlier export was kept");
    }

    // ── What is reported ──────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn writing_is_reported_after_the_chains_last_report() {
        let fixture = Fixture::new();
        let (reporting, sent) = collecting("export-1");

        fixture
            .export(&Recording::default(), Some(reporting), fixture.request("export-1", vec![kyoto()], "a.png"))
            .expect("exportable");

        let sent = sent.lock().expect("unpoisoned");
        let (last, chain) = sent.split_last().expect("reports were sent");

        assert_eq!(*last, ExportProgress::Writing { run: "export-1".to_string() });
        assert!(!chain.is_empty(), "the chain's own reports were not sent");
        assert!(
            chain
                .iter()
                .all(|report| matches!(report, ExportProgress::Enhancing(run) if run.run == "export-1")),
            "a report before writing was not the chain's, under the export's name: {chain:?}"
        );

        let ExportProgress::Enhancing(final_report) = chain.last().expect("not empty") else {
            unreachable!()
        };
        assert_eq!(final_report.chain_fraction, 1.0, "the chain's final report was held back past writing");
    }

    #[test]
    fn an_empty_chain_reports_only_that_it_is_writing() {
        let fixture = Fixture::new();
        let (reporting, sent) = collecting("export-1");

        fixture
            .export(&Recording::default(), Some(reporting), fixture.request("export-1", Vec::new(), "a.png"))
            .expect("exportable");

        assert_eq!(*sent.lock().expect("unpoisoned"), [ExportProgress::Writing { run: "export-1".to_string() }]);
    }

    // ── What is refused ───────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn an_identity_this_application_never_admitted_is_refused_before_anything_runs() {
        let fixture = Fixture::new();
        let enhancer = Recording::default();

        let refused = fixture
            .export(
                &enhancer,
                None,
                Request {
                    source: "0123456789abcdef".to_string(),
                    ..fixture.request("export-1", vec![kyoto()], "a.png")
                },
            )
            .expect_err("an unopened identity is not exportable");

        assert_eq!(refused, ExportError::UnknownSource { identity: "0123456789abcdef".to_string() });
        assert!(enhancer.seen().is_empty(), "a refused request still reached the library");
        assert!(fixture.written().is_empty(), "a refused request left a file behind");
    }

    #[test]
    fn an_unpublished_model_is_refused_by_position_before_anything_is_fetched() {
        let fixture = Fixture::new();
        let enhancer = Recording::default();
        let chain = vec![
            kyoto(),
            Requested::Upscale { codename: "berlin".to_string(), precision: Precision::Fp32, scale: 2.0 },
        ];

        let refused = fixture
            .export(&enhancer, None, fixture.request("export-1", chain, "a.png"))
            .expect_err("a chain naming an unpublished model is refused");

        assert!(matches!(refused, ExportError::UnknownOperation { index: 1, .. }), "{refused:?}");
        assert!(enhancer.seen().is_empty(), "a model was fetched for a refused chain");
        assert!(fixture.written().is_empty(), "a refused request left a file behind");
        assert!(fixture.exports.is_idle());
    }

    #[test]
    fn an_admitted_file_that_can_no_longer_be_read_is_refused_as_unreadable() {
        let fixture = Fixture::new();
        std::fs::remove_file(fixture.source.path().join("holiday.png")).expect("the fixture exists");

        let refused = fixture
            .export(&Recording::default(), None, fixture.request("export-1", vec![kyoto()], "a.png"))
            .expect_err("a deleted file cannot be exported");

        assert!(matches!(&refused, ExportError::UnreadableSource { identity, .. } if *identity == fixture.identity));
        assert!(fixture.written().is_empty(), "a refused request left a file behind");
        assert!(fixture.exports.is_idle(), "a refused export left its token in the table");
    }

    #[test]
    fn a_chain_that_fails_carries_the_librarys_sentence_and_writes_nothing() {
        let fixture = Fixture::new();

        let refused = fixture
            .export(&Failing, None, fixture.request("export-1", vec![kyoto()], "a.png"))
            .expect_err("this chain fails");

        assert_eq!(
            refused,
            ExportError::Enhance { message: InferenceError::Untileable { width: 0, height: 0 }.to_string() }
        );
        assert!(fixture.written().is_empty(), "a failed export left a file behind");
    }

    #[test]
    fn a_destination_that_cannot_be_written_is_refused_naming_it_and_creates_nothing() {
        let fixture = Fixture::new();
        let destination = fixture.out.path().join("missing").join("photo-opai.png");

        let refused = fixture
            .export(
                &Recording::default(),
                None,
                Request { destination: destination.clone(), ..fixture.request("export-1", vec![kyoto()], "") },
            )
            .expect_err("nothing can be written into a missing directory");

        let ExportError::Unwritable { path, message } = &refused else {
            panic!("an unwritable destination was refused as something else: {refused:?}");
        };
        assert_eq!(*path, destination.display().to_string());
        assert!(!message.is_empty(), "the refusal carried no reason");
        assert!(fixture.written().is_empty(), "an unwritable export created something");
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_this_application_may_not_write_to_is_refused_naming_the_destination() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new();
        let locked = fixture.out.path().join("locked");
        std::fs::create_dir(&locked).expect("writable");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).expect("chmod");

        // Root writes through the mode bits, so there is nothing to refuse and nothing to check.
        let probe = locked.join("probe");
        if std::fs::write(&probe, b"").is_ok() {
            std::fs::remove_file(probe).expect("removable");
            return;
        }

        let destination = locked.join("photo-opai.png");
        let refused = fixture
            .export(
                &Recording::default(),
                None,
                Request { destination: destination.clone(), ..fixture.request("export-1", vec![kyoto()], "") },
            )
            .expect_err("nothing can be written into a read-only directory");

        let ExportError::Unwritable { path, message } = &refused else {
            panic!("a read-only destination was refused as something else: {refused:?}");
        };
        assert_eq!(*path, destination.display().to_string());
        assert!(!message.is_empty(), "the refusal carried no reason");
        assert_eq!(std::fs::read_dir(&locked).expect("listable").count(), 0, "a refused export created something");

        // So the temporary directory can be removed.
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    /// An enhancer whose result is too wide for a GIF, which holds at most 65,535 pixels a side, so the encoder refuses
    /// it after the name was claimed — the one failure that reaches the write rather than the claim.
    struct TooWide;

    impl Enhancer for TooWide {
        fn process(
            &self,
            source: &Picture,
            _operations: &[opai::Operation],
            _options: ProcessOptions,
        ) -> impl Future<Output = Result<Enhanced, InferenceError>> + Send {
            let result = enhanced(source, image::DynamicImage::new_rgb8(70_000, 1), "cafebabecafebabe");

            async move { Ok(result) }
        }
    }

    #[test]
    fn a_write_that_fails_gives_its_claimed_name_back() {
        let fixture = Fixture::new();

        let refused = fixture
            .export(
                &TooWide,
                None,
                Request { format: ExportFormat::Gif, ..fixture.request("export-1", vec![kyoto()], "a.gif") },
            )
            .expect_err("a GIF this wide cannot be encoded");

        assert!(matches!(refused, ExportError::Write { .. }), "{refused:?}");
        assert!(fixture.written().is_empty(), "a failed write left its placeholder behind");
    }

    #[test]
    fn a_failed_overwrite_leaves_the_file_it_would_have_replaced_as_it_was() {
        let fixture = Fixture::new();
        std::fs::write(fixture.out.path().join("a.gif"), b"an earlier export").expect("writable");

        let refused = fixture
            .export(
                &TooWide,
                None,
                Request {
                    format: ExportFormat::Gif,
                    overwrite: true,
                    ..fixture.request("export-1", vec![kyoto()], "a.gif")
                },
            )
            .expect_err("a GIF this wide cannot be encoded");

        assert!(matches!(refused, ExportError::Write { .. }), "{refused:?}");
        assert_eq!(fixture.written(), ["a.gif"], "a failed overwrite left a file beside its destination");
        assert_eq!(
            std::fs::read(fixture.out.path().join("a.gif")).expect("readable"),
            b"an earlier export",
            "a failed overwrite destroyed the file it would have replaced"
        );
    }

    #[test]
    fn every_answer_and_refusal_crosses_under_the_tag_the_window_matches_on() {
        let answers = [
            (Exported::Exported { path: "/a.png".to_string(), bytes: 3 }, "exported"),
            (Exported::Stopped, "stopped"),
        ];
        for (answer, outcome) in answers {
            assert_eq!(serde_json::to_value(&answer).expect("serializable")["outcome"], outcome);
        }

        let refusals = [
            (ExportError::NotReady, "notReady"),
            (ExportError::UnknownSource { identity: "a".to_string() }, "unknownSource"),
            (
                ExportError::UnknownOperation {
                    index: 0,
                    reason: UnknownOperation::Model { family: opai::Family::Upscale, codename: "b".to_string() },
                },
                "unknownOperation",
            ),
            (
                ExportError::UnreadableSource { identity: "a".to_string(), message: "b".to_string() },
                "unreadableSource",
            ),
            (ExportError::Enhance { message: "b".to_string() }, "enhance"),
            (ExportError::Write { path: "a".to_string(), message: "b".to_string() }, "write"),
            (ExportError::Unwritable { path: "a".to_string(), message: "b".to_string() }, "write"),
        ];
        for (error, kind) in refusals {
            assert_eq!(serde_json::to_value(&error).expect("serializable")["kind"], kind, "{error:?}");
        }

        assert_eq!(
            serde_json::to_value(Exported::Exported { path: "/a.png".to_string(), bytes: 3 }).expect("serializable"),
            serde_json::json!({ "outcome": "exported", "path": "/a.png", "bytes": 3 })
        );
    }

    // ── Stopping ──────────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_stop_that_arrives_first_answers_stopped_without_reading_the_photograph() {
        let fixture = Fixture::new();
        let enhancer = Recording::default();

        // Deleted, so a read would be refused as unreadable rather than answered as stopped.
        std::fs::remove_file(fixture.source.path().join("holiday.png")).expect("the fixture exists");
        fixture.exports.stop("export-1");

        let answer = fixture
            .export(&enhancer, None, fixture.request("export-1", vec![kyoto()], "a.png"))
            .expect("a stop is not a failure");

        assert_eq!(answer, Exported::Stopped);
        assert!(enhancer.seen().is_empty(), "an export a stop overtook still ran");
    }

    /// An enhancer whose export is stopped while it works, which it notices and reports as `opai` does.
    struct StoppedWhileWorking<'a>(&'a Exports, &'static str);

    impl Enhancer for StoppedWhileWorking<'_> {
        async fn process(
            &self,
            _source: &Picture,
            _operations: &[opai::Operation],
            options: ProcessOptions,
        ) -> Result<Enhanced, InferenceError> {
            assert!(self.0.stop(self.1), "the export was not in flight while its chain ran");

            options.cancel.cancelled().await;

            Err(InferenceError::Cancelled)
        }
    }

    #[test]
    fn a_stop_during_the_chain_answers_stopped_and_leaves_no_file_at_any_name() {
        let fixture = Fixture::new();
        std::fs::write(fixture.out.path().join("photo-opai.png"), b"an earlier export").expect("writable");

        let answer = fixture
            .export(
                &StoppedWhileWorking(&fixture.exports, "export-1"),
                None,
                fixture.request("export-1", vec![kyoto()], "photo-opai.png"),
            )
            .expect("a stop is not a failure");

        assert_eq!(answer, Exported::Stopped);
        assert_eq!(fixture.written(), ["photo-opai.png"], "a stopped export left a file at some name");
        assert!(fixture.exports.is_idle(), "a stopped export left its token in the table");
    }

    #[test]
    fn a_chain_ended_by_the_application_shutting_down_answers_stopped_and_writes_nothing() {
        struct ShuttingDown;

        impl Enhancer for ShuttingDown {
            async fn process(
                &self,
                _source: &Picture,
                _operations: &[opai::Operation],
                _options: ProcessOptions,
            ) -> Result<Enhanced, InferenceError> {
                Err(InferenceError::Shutdown)
            }
        }

        let fixture = Fixture::new();

        let answer = fixture
            .export(&ShuttingDown, None, fixture.request("export-1", vec![kyoto()], "a.png"))
            .expect("a shutdown is not the export's failure");

        assert_eq!(answer, Exported::Stopped);
        assert!(fixture.written().is_empty(), "an export ended by a shutdown still wrote");
        assert!(fixture.exports.is_idle(), "an export ended by a shutdown left its token in the table");
    }

    #[test]
    fn a_stop_the_chain_never_noticed_still_answers_stopped_and_writes_nothing() {
        let fixture = Fixture::new();

        let answer = fixture
            .export(
                &StoppedMidway(&fixture.exports, "export-1", Recording::default()),
                None,
                fixture.request("export-1", vec![kyoto()], "a.png"),
            )
            .expect("a stop is not a failure");

        assert_eq!(answer, Exported::Stopped);
        assert!(fixture.written().is_empty(), "an export stopped before writing still wrote");
    }

    #[test]
    fn a_stop_once_writing_has_begun_lets_the_write_finish() {
        let fixture = Fixture::new();
        let exports = Arc::new(Exports::default());
        let stopper = Arc::clone(&exports);

        // Stopped the moment the window hears that the export is writing.
        let reporting = ExportReporting::new("export-1", move |report| {
            if matches!(report, ExportProgress::Writing { .. }) {
                stopper.stop("export-1");
            }
        });

        let answer = tauri::async_runtime::block_on(export_with(
            &Recording::default(),
            &fixture.opened,
            &exports,
            Some(reporting),
            fixture.request("export-1", vec![kyoto()], "a.png"),
        ))
        .expect("exportable");

        assert!(matches!(answer, Exported::Exported { .. }), "a stop during writing was honoured: {answer:?}");
        assert_eq!(fixture.written(), ["a.png"]);
    }

    #[test]
    fn an_export_neither_displaces_nor_releases_the_canvas_run() {
        use crate::enhance::{Resident, Runs};

        let fixture = Fixture::new();
        let runs = Runs::default();

        // A canvas run in flight, while one export finishes and another is stopped.
        let token = runs.start("enhance-1", "5ec0ffee5ec0ffee");

        fixture
            .export(&Recording::default(), None, fixture.request("export-1", vec![kyoto()], "a.png"))
            .expect("exportable");
        fixture.exports.stop("export-2");
        fixture.exports.stop("enhance-1");

        assert!(!token.is_cancelled(), "an export, or a stop for one, stopped the canvas run");
        assert!(
            runs.finish(
                "enhance-1",
                Picture::new("/pictures/holiday.jpg", image::DynamicImage::new_rgb8(2, 2), "aaaaaaaaaaaaaaaa")
            ),
            "an export displaced the canvas run"
        );
        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Picture(_))),
            "the canvas result is gone"
        );
    }

    // ── Against the real models ───────────────────────────────────────────────────────────────────────────────────

    #[test]
    #[ignore = "installs the runtime and the Kyoto model into the real configuration directory; run by hand with the \
                GUI closed"]
    fn the_committed_photograph_exports_through_a_real_upscale() {
        tauri::async_runtime::block_on(async {
            let opai = Opai::initialize(opai::APP_NAME, None).await.expect("the application must start");

            let dir = tempfile::tempdir().expect("a temporary directory");
            let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/test.dat");
            let bytes = std::fs::read(&fixture).expect("the committed photograph is readable");
            let opened = Opened::default();

            for processor in [Processor::Cpu, Processor::Coreml] {
                // The run store is on disk and outlives the process, so the committed photograph as it stands would be
                // served from an earlier run and the check would pass having run nothing. A nonce after the JPEG's end
                // marker makes a file the store has never seen, whose pixels are the fixture's.
                let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("after 1970");
                let path = dir.path().join(format!("test-{processor:?}.jpg"));
                std::fs::write(&path, [bytes.as_slice(), format!("{processor:?}-{nonce:?}").as_bytes()].concat())
                    .expect("writable");

                crate::images::files::describe_and_admit(vec![path.clone()], &opened).await.expect("admissible");
                let identity = opai::image::identity(&path).await.expect("a written file has an identity");
                let (width, height) = opai::image::load(&path).await.expect("decodable").dimensions();

                for (format, name) in [(ExportFormat::Png, "test-opai.png"), (ExportFormat::Jpeg, "test-opai.jpg")] {
                    let run = format!("export-{processor:?}-{format:?}");
                    let (reporting, sent) = crate::export::progress::collecting(&run);
                    let started = std::time::Instant::now();

                    let answer = export_with(
                        &opai,
                        &opened,
                        &Exports::default(),
                        Some(reporting),
                        Request {
                            run: run.clone(),
                            source: identity.clone(),
                            operations: vec![kyoto()],
                            processor,
                            crop: None,
                            destination: dir.path().join(format!("{processor:?}-{name}")),
                            format,
                            quality: 90.0,
                            overwrite: false,
                        },
                    )
                    .await
                    .expect("exportable");

                    let Exported::Exported { path: written, bytes } = answer else {
                        panic!("nothing stopped this export, and it answered stopped");
                    };

                    assert_eq!(bytes, std::fs::metadata(&written).expect("written").len(), "{processor:?} {format:?}");
                    assert_eq!(decoded(&written).0, (width * 2, height * 2), "{processor:?} {format:?}");
                    assert_eq!(
                        sent.lock().expect("unpoisoned").last(),
                        Some(&ExportProgress::Writing { run }),
                        "{processor:?} {format:?}"
                    );

                    eprintln!(
                        "{processor:?} {format:?}: {width}x{height} -> {}x{}, {bytes} bytes, {:.1?}",
                        width * 2,
                        height * 2,
                        started.elapsed()
                    );
                }
            }
        });
    }

    #[test]
    fn each_ending_is_recorded_by_whoever_saw_it() {
        let s = String::new;
        let reason = UnknownOperation::Model { family: opai::Family::Upscale, codename: s() };
        let cells = [
            (ExportError::NotReady, "recorded by the wrapper"),
            (ExportError::UnknownSource { identity: s() }, "recorded by the wrapper"),
            (ExportError::UnknownOperation { index: 0, reason }, "recorded by the wrapper"),
            (ExportError::Unwritable { path: s(), message: s() }, "recorded by the wrapper"),
            (ExportError::UnreadableSource { identity: s(), message: s() }, "recorded by opai"),
            (ExportError::Enhance { message: s() }, "recorded by opai"),
            (ExportError::Write { path: s(), message: s() }, "recorded by opai"),
        ];
        for (error, cell) in cells {
            assert_eq!(CommandError::ended(&error).cell(), cell, "{error:?}");
        }

        let answer = Exported::Exported { path: s(), bytes: 1 };
        assert_eq!(Answer::ended(&answer).cell(), "finished");
        assert_eq!(Answer::ended(&Exported::Stopped).cell(), "stopped");
    }

    #[test]
    fn a_destination_that_could_not_be_claimed_crosses_as_a_failed_write_did() {
        let path = "/exports/a.png".to_string();
        let message = "permission denied".to_string();

        let unwritable = ExportError::Unwritable { path: path.clone(), message: message.clone() };
        let written = ExportError::Write { path, message };

        // The shape the window's formatter has always read, so it needs no edit.
        let shape = serde_json::json!({ "kind": "write", "path": "/exports/a.png", "message": "permission denied" });
        assert_eq!(serde_json::to_value(&unwritable).expect("serializable"), shape);
        assert_eq!(serde_json::to_value(&written).expect("serializable"), shape);
        assert_eq!(unwritable.to_string(), written.to_string());
    }

    /// [`export_with`] inside the `export` command's span, as the command runs it, and what that recorded.
    fn traced_export<E: Enhancer>(
        fixture: &Fixture,
        enhancer: &E,
        request: Request,
    ) -> (crate::test_support::Recorded, Result<Exported, ExportError>) {
        crate::test_support::recorded(|| {
            tauri::async_runtime::block_on(crate::command::traced(
                crate::command::command_span!(
                    "export",
                    crate::command::Traceparent::default(),
                    run = request.run.clone()
                ),
                export_with(enhancer, &fixture.opened, &fixture.exports, None, request),
            ))
        })
    }

    #[test]
    fn a_destination_that_cannot_be_claimed_is_recorded_once_by_the_wrapper_and_never_as_an_error() {
        let fixture = Fixture::new();
        let destination = fixture.out.path().join("missing").join("photo-opai.png");
        let request = Request { destination: destination.clone(), ..fixture.request("export-1", vec![kyoto()], "") };

        let (recorded, answer) = traced_export(&fixture, &Recording::default(), request);
        assert!(matches!(answer, Err(ExportError::Unwritable { .. })), "{answer:?}");

        assert!(recorded.at(tracing::Level::ERROR).is_empty(), "a claim failure was recorded as an error");

        let [warning] = recorded.at(tracing::Level::WARN)[..] else {
            panic!("expected exactly one warning: {:#?}", recorded.events);
        };
        assert_eq!(warning.target, crate::command::TARGET);
        assert_eq!(warning.field("command"), Some("export"));
        let error = warning.field("error").expect("the warning says why");
        assert!(
            error.contains(&destination.display().to_string()),
            "the warning did not name the destination: {error}"
        );
        assert_eq!(recorded.only("export").field("outcome"), Some("failed"));
    }

    #[test]
    fn a_photograph_that_cannot_be_read_is_left_to_the_librarys_own_record() {
        let fixture = Fixture::new();
        std::fs::remove_file(fixture.source.path().join("holiday.png")).expect("the fixture exists");

        let (recorded, answer) =
            traced_export(&fixture, &Recording::default(), fixture.request("export-1", vec![kyoto()], "a.png"));
        assert!(matches!(answer, Err(ExportError::UnreadableSource { .. })), "{answer:?}");

        let wrapper: Vec<_> = recorded.events.iter().filter(|event| event.target == crate::command::TARGET).collect();
        assert!(wrapper.is_empty(), "the wrapper recorded a failure `opai` already had: {wrapper:#?}");
        assert_eq!(recorded.only("export").field("outcome"), Some("failed"));
    }
}
