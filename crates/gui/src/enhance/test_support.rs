//! Fixtures the tests in more than one of this module's files need.

use opai::{Enhanced, Picture, Precision};

use super::operation::Requested;
use super::run::{Enhancer, Processor, Request};
use super::slot::Runs;
use crate::enhance::run::{EnhanceError, Enhancement, enhance_with};
use crate::images::{Crop, Opened};
use crate::test_support::admitted;

/// A request for `source`, naming the run `run`, over the whole photograph.
pub(super) fn request(run: &str, source: &str, operations: Vec<Requested>) -> Request {
    Request {
        run: run.to_string(),
        source: source.to_string(),
        operations,
        processor: Processor::Coreml,
        crop: None,
    }
}

/// The same request, over the photograph as `crop` frames it.
pub(super) fn framed_request(run: &str, source: &str, operations: Vec<Requested>, crop: Crop) -> Request {
    Request { crop: Some(crop), ..request(run, source, operations) }
}

/// One upscale the catalogue publishes, as the window would name it.
pub(super) fn kyoto() -> Requested {
    Requested::Upscale { codename: "kyoto".to_string(), precision: Precision::Fp32, scale: 2.0 }
}

/// A temporary directory, an empty pair of registries, and one admitted image's identity.
pub(super) fn fixture() -> (tempfile::TempDir, Opened, Runs, String) {
    // The four go together at every test in this module that runs anything: a run needs somewhere to have come
    // from (`Opened`), somewhere to be recorded (`Runs`), and an identity to name. Spelling them out per
    // test is four lines of preamble before the line that states what the test is actually about — and the
    // `TempDir` has to be returned rather than dropped, which is the one part of it that is easy to get wrong.
    let dir = tempfile::tempdir().expect("a temporary directory");
    let opened = Opened::default();
    let runs = Runs::default();
    let identity = admitted(&dir, &opened);

    (dir, opened, runs, identity)
}

/// What a fake [`Enhancer`] hands back: a picture of the given size at the given identity, on the source's
/// own path, reporting the automatic provider.
pub(super) fn enhanced(source: &Picture, width: u32, height: u32, identity: &str) -> Enhanced {
    // The provider report is the same at every fake because none of them is what any test is asserting about —
    // it is the shape `Enhanced` requires, and repeating it per fake is four lines that say nothing.
    Enhanced {
        picture: Picture::new(source.path(), image::DynamicImage::new_rgb8(width, height), identity),
        providers: opai::ProviderReport { requested: opai::ExecutionProvider::Auto, actual: Vec::new() },
    }
}

/// [`enhance_with`] driven to completion with no progress reporting, which is every run this module tests
/// except the two that are specifically about the reports.
pub(super) fn run_now<E: Enhancer>(
    enhancer: &E,
    opened: &Opened,
    runs: &Runs,
    request: Request,
) -> Result<Enhancement, EnhanceError> {
    tauri::async_runtime::block_on(enhance_with(enhancer, opened, runs, None, request))
}
