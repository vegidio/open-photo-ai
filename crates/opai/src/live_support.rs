//! The harness the live checks share: a real runtime, installed into a directory of the test's own.

// A file at the crate root because its callers are in different modules — the session layer's live checks and the
// inference layer's both need a started runtime before they can open anything. Two copies of the initialization body
// is two things to update whenever that body changes, which defeats the point of these tests: what they exist to prove
// is that the *real* path works, so the harness driving it has to be the real path and not a hand-built approximation
// of it that has drifted.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use rust_sak::image::ImageFormat;
use tempfile::TempDir;

use crate::deps::artifact::ONNX_RUNTIME;
use crate::deps::release::{Dependency as Descriptor, RELEASE_BASE_URL};
use crate::models::filter::GUARDED;
use crate::models::precision::FloatPrecision;
use crate::models::{Operation, Strength};
use crate::providers::ExecutionProvider;
use crate::setup::Plan;
use crate::{Opai, ProcessOptions, runtime};
use imaging::tensor::Sampler;

/// The environment variable that overrides the committed fixture with a photograph of the runner's own.
pub(crate) const PHOTO: &str = "OPAI_LIVE_PHOTO";

/// The committed photograph, at the workspace root.
pub(crate) fn fixture() -> PathBuf {
    // At the workspace root rather than under this crate so the benchmark crate reaches the same file rather than a
    // second copy of it.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/test.dat")
}

/// The photograph to enhance: the committed fixture, or whatever [`PHOTO`] names instead.
pub(crate) async fn photograph() -> crate::Picture {
    // Here rather than in each live module: spelled out per module, moving the fixture or renaming the variable leaves
    // one of them reading the old location — and because these checks are `#[ignore]`d, nothing would catch it until
    // someone ran them by hand and got a panic, or worse a measurement over the wrong photograph.
    let path = std::env::var_os(PHOTO).map_or_else(fixture, PathBuf::from);

    // Through the loader rather than decoded here, which also makes this the check that a file the loader produced is
    // a file the pipeline accepts — including that `test.dat`'s extension, which names no format, does not stop it
    // being recognised as the JPEG it is.
    crate::image::load(&path)
        .await
        .unwrap_or_else(|err| panic!("{} did not decode: {err}", path.display()))
}

/// Installs the pinned runtime beneath a temporary directory named `name`, starts it, and returns a handle to the
/// application alongside the root keeping that directory alive.
///
/// The body of [`Opai::initialize`] against a directory of this test's own: the real one resolves the platform's
/// configuration directory, which a test must not write into.
pub(crate) async fn live_application(name: &str) -> (TempDir, Opai) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let opai = live_application_at(name, &root.path().join(name)).await;

    (root, opai)
}

/// [`live_application`] against a directory the caller chose and keeps.
///
/// For the checks whose weights are too large to transfer again per run: Osaka's three FP16 graphs are ~7.2 GB, so a
/// temporary directory would mean re-downloading them for every test in the module and again on every re-run. A
/// caller that supplies a directory it keeps pays that once. Everything else is identical, install path included.
pub(crate) async fn live_application_at(name: &str, app_dir: &Path) -> Opai {
    let app_dir = app_dir.to_path_buf();

    let descriptor =
        Descriptor::from_release_at(RELEASE_BASE_URL, &ONNX_RUNTIME, std::env::consts::OS, std::env::consts::ARCH)
            .expect("the runtime is published for this platform");
    let lib = descriptor.lib.expect("every platform the runtime is published for names its library");

    let claim = crate::instance::tests::claimed(&app_dir);
    let (opai, installed) =
        Opai::install(name, app_dir, claim, Plan::runtime_only(descriptor), crate::ModelTrust::Published, None)
            .await
            .expect("the pinned runtime must install");

    // From the directory the install reported, as `Opai::initialize` does it. `start` is once per process, so a
    // second call from another check in the same binary is the no-op it reports as success.
    let library = runtime::library_path(&installed.runtime, lib);
    runtime::start(name, &library).expect("the pinned runtime must load and start");

    opai
}

/// A directory live checks install into and keep, named for `what`, under the system temporary directory unless
/// [`LIVE_DIR`] names somewhere else.
///
/// Created rather than merely composed, because the claim taken over it is taken before anything else.
pub(crate) fn kept_dir(what: &str) -> PathBuf {
    let root = std::env::var_os(LIVE_DIR).map_or_else(std::env::temp_dir, PathBuf::from);
    let dir = root.join(what);

    std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("{} is not creatable: {err}", dir.display()));

    dir
}

/// The environment variable that overrides where [`kept_dir`] puts a kept installation — for a runner whose temporary
/// directory is a RAM disk, or one where several gigabytes belong on another volume.
pub(crate) const LIVE_DIR: &str = "OPAI_LIVE_DIR";

/// Whether `left` and `right` are the same picture, pixel for pixel.
pub(crate) fn identical(left: &image::DynamicImage, right: &image::DynamicImage) -> bool {
    if (left.width(), left.height()) != (right.width(), right.height()) {
        return false;
    }

    let (width, height) = (left.width(), left.height());
    let (left, right) = (Sampler::new(left), Sampler::new(right));

    // Every pixel rather than a sample of them. This is the check that nothing reached the result by a route the
    // bias does not gate, and a sample would pass over a corner the extension leaked into — which is exactly the
    // region where the padding is.
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .all(|(x, y)| left.rgb(x, y) == right.rgb(x, y))
}

/// The top-left `width` by `height` of `source`, as a photograph of its own.
pub(crate) fn cropped(source: &crate::Picture, width: u32, height: u32) -> crate::Picture {
    // A distinct identity per crop, which is the one thing `Picture::new` leaves to its caller: these are different
    // photographs, and two sharing one identity would have the second served the first's cached result — which would
    // make a check pass over a pipeline that never ran.
    crate::Picture::new(
        source.path(),
        source.pixels().crop_imm(0, 0, width, height),
        format!("{}-crop-{width}x{height}", source.identity()),
    )
}

/// How many distinct green levels `picture` carries — more than 256 is what separates a 16-bit result from an 8-bit
/// answer widened into one.
pub(crate) fn green_levels(picture: &image::DynamicImage) -> usize {
    let sampler = Sampler::new(picture);

    (0..picture.height())
        .flat_map(|y| (0..picture.width()).map(move |x| (x, y)))
        .map(|(x, y)| sampler.rgb(x, y)[1])
        .collect::<std::collections::BTreeSet<u16>>()
        .len()
}

/// A strength family — denoise or sharpen — as its live check drives it: three variants through the shared
/// guarded-filter pipeline, one of which has the guard.
pub(crate) struct StrengthFamily {
    /// The application name the family's checks install under.
    pub(crate) name: &'static str,
    /// The family's three codenames, in the order the results are compared.
    pub(crate) variants: [&'static str; 3],
    /// The one codename whose guard may keep a tile; the other two keeping one is a failure.
    pub(crate) guarded: &'static str,
    /// The operation running a codename at a precision and strength.
    pub(crate) operation: fn(&str, FloatPrecision, Strength) -> Operation,
}

impl StrengthFamily {
    // The family's own tests exercise everything else against a fake backend on every CI platform — the tile grid at
    // scale 1, the guard, the blend, the progress schedule, the depth dispatch, the cancellation and the refusal. What
    // no CI platform can prove is that a real session produces a photograph at its own size, that the strength gates
    // every route to the pixels, and whether the guarded variant's guard fires on a real photograph at all.

    /// `provider` against both precisions of all three variants, and the three against one another at each precision.
    pub(crate) async fn everywhere(&self, provider: ExecutionProvider) {
        // A delta rather than a reset, as each run's own count is, so the total below is this check's own even in a
        // process that ran something else guarded before it.
        let guarded_before = GUARDED.load(Ordering::Relaxed);

        for precision in FloatPrecision::ALL {
            let mut results = Vec::with_capacity(self.variants.len());

            for codename in self.variants {
                results.push(self.on(codename, precision, provider).await);
            }

            // Three sets of weights at one strength: two identical results would mean a seam handed two variants one
            // graph.
            for (left, right) in [(0, 1), (0, 2), (1, 2)] {
                assert!(
                    !identical(results[left].pixels(), results[right].pixels()),
                    "{} and {} at {precision:?} on {provider} produced the same photograph",
                    self.variants[left],
                    self.variants[right]
                );
            }
        }

        println!(
            "\n{provider}: {} tile(s) kept by {}'s guard across both precisions",
            GUARDED.load(Ordering::Relaxed) - guarded_before,
            self.guarded
        );
    }

    /// Puts the committed photograph through `codename` at `precision` on `provider`, at a strength of 0 and of 1,
    /// states the properties, and hands back the result at 1.
    async fn on(&self, codename: &str, precision: FloatPrecision, provider: ExecutionProvider) -> crate::Picture {
        // A fresh application per call, so no result is served from another provider's run of the same operation.
        let (_root, opai) = live_application(self.name).await;
        let source = photograph().await;
        let (width, height) = source.dimensions();

        println!("\n{codename} at {precision:?} on {provider}: source {width}x{height}");

        let mut kept = None;

        for strength in [1.0, 0.0] {
            let operation = (self.operation)(
                codename,
                precision,
                Strength::new(strength).expect("a live check supplies a strength in range"),
            );
            let options = ProcessOptions { provider, ..Default::default() };
            let guarded_before = GUARDED.load(Ordering::Relaxed);
            let started = std::time::Instant::now();

            let enhanced = opai
                .process(&source, &[operation], Some(options))
                .await
                .unwrap_or_else(|err| panic!("{codename} at {precision:?} on {provider}, strength {strength}: {err}"));

            let guarded = GUARDED.load(Ordering::Relaxed) - guarded_before;
            let produced = enhanced.picture;

            println!(
                "  strength {strength}: {:?} on {:?}, {guarded} tile(s) kept by the guard",
                started.elapsed(),
                enhanced.providers.actual
            );

            assert_eq!(
                produced.dimensions(),
                (width, height),
                "{codename} at strength {strength} did not return the photograph's own dimensions"
            );

            if codename != self.guarded {
                assert_eq!(guarded, 0, "{codename} is not guarded, yet a tile was kept");
            }

            if strength == 0.0 {
                // After every tile has run and been stitched, and then multiplied by nothing: anything that reached
                // the pixels by another route shows here.
                assert!(
                    identical(produced.pixels(), source.pixels()),
                    "{codename} at a strength of 0 did not return the photograph unchanged"
                );
            } else {
                assert!(
                    !identical(produced.pixels(), source.pixels()),
                    "{codename} at a strength of 1 returned the photograph untouched"
                );

                // Kept where it can be looked at: a wrong normalisation or a seam is a photograph rather than a number.
                let keep =
                    std::env::temp_dir().join(format!("opai-live-{codename}-{precision:?}-{provider}-t{strength}.png"));

                crate::image::save(produced.shared_pixels(), &keep, ImageFormat::Png, None)
                    .await
                    .expect("a result must encode to PNG");
                println!("  inspect at {}", keep.display());

                kept = Some(produced);
            }
        }

        kept.expect("the run at a strength of 1 keeps its result")
    }
}
