//! Where a model's files and what a provider compiled from them live, under the application's configuration directory.

use crate::models::ArtifactId;

/// The subdirectory of the application's configuration directory every model installs beneath.
pub(crate) const MODELS_DIR: &str = "models";

// Here rather than in `sessions/caches.rs`, which is what *decides* the layout of that tree and owns everything else
// about its lifetime. The name has to be down here because the install layer needs it too — a replacement of a model's
// weights must discard the engine compiled from them — and `deps/` sits below `sessions/`, so naming it there would
// have the layer that puts bytes on disk importing from the layer that runs them.
/// The subdirectory of the application's configuration directory every compiled artifact lives beneath.
pub(crate) const ENGINES_DIR: &str = "engines";

/// The directory holding what a provider compiled from `id`, relative to the application's configuration directory:
/// `engines/up_kyoto_4x_fp32`.
pub(crate) fn engine_dir(id: &ArtifactId) -> String {
    // A function rather than a `format!` at three call sites: the install that names this directory as derived from
    // the weights, `rust-sak`, which empties it on a replacement, and the builder that points a provider at it must all
    // name the same one.
    format!("{ENGINES_DIR}/{id}")
}

/// The directory `id` installs into, relative to the application's configuration directory:
/// `models/up_kyoto_4x_fp32`.
pub(crate) fn model_dir(id: &ArtifactId) -> String {
    // **A directory per model**, where the reference implementation installs every model flat into `models/`. The
    // installer removes everything in a model's directory that is not one of its files, so a shared directory would
    // make installing `up_kyoto_4x_fp32` delete the `up_kyoto_2x_fp32` beside it — which is the *other half of the same
    // 8x operation*. It also pre-empts a problem the reference documents and cannot fix: a provider's compiled engine is
    // dropped beside the weights and named after the graph inside the file rather than after the file, so nothing says
    // whose cache is whose. Here it goes into a directory of its own under `engines/`, named after the model it was
    // compiled from — see `engine_dir`.
    format!("{MODELS_DIR}/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::artifacts;
    use crate::models::{FloatPrecision, UpscaleVariant};

    #[test]
    fn each_model_installs_into_a_directory_of_its_own() {
        // The two halves of one 8x Kyoto. A shared `models/` would have the installer clear the directory for the
        // second and delete the first — the other half of the same operation.
        let required = artifacts(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 8.0);
        assert_eq!(required.len(), 2, "an 8x Kyoto runs two passes");

        let dirs: Vec<String> = required.iter().map(model_dir).collect();

        assert_eq!(dirs, vec!["models/up_kyoto_4x_fp32", "models/up_kyoto_2x_fp32"]);
        let engines: Vec<String> = required.iter().map(engine_dir).collect();
        assert_eq!(engines, vec!["engines/up_kyoto_4x_fp32", "engines/up_kyoto_2x_fp32"]);
    }
}
