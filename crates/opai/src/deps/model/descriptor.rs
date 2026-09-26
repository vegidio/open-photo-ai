//! Turning a model artifact's name into the descriptor the install pipeline works from.
//!
//! The counterpart of [`crate::deps::release`], which does the same for a pinned release archive.

// The difference is where the facts come from: an archive's URL, hash and size are `const` in this repository, while a
// model's are read from the published listing — so this takes one as an argument rather than reaching for a table.

use std::path::Path;

use crate::deps::ModelTrust;
use crate::deps::model::manifest::Listing;
use crate::deps::release::{Contents, Dependency, Source};
use crate::models::ArtifactId;
use crate::progress::Dependency as Which;

/// The subdirectory of the application's configuration directory every model installs beneath.
pub(crate) const MODELS_DIR: &str = "models";

// Here rather than in `sessions/caches.rs`, which is what *decides* the layout of that tree and owns everything else
// about its lifetime. The name has to be down here because the install layer needs it too — a replacement of a model's
// weights must discard the engine compiled from them — and `deps/` sits below `sessions/`, so naming it there would
// have the layer that puts bytes on disk importing from the layer that runs them.
/// The subdirectory of the application's configuration directory every compiled artifact lives beneath.
pub(crate) const ENGINES_DIR: &str = "engines";

/// Where the project publishes the model files. A base name is appended to it, so a URL reads
/// `.../resolve/main/models/up_kyoto_4x_fp32.onnx`.
pub(crate) const MODEL_BASE_URL: &str = "https://huggingface.co/vegidio/open-photo-ai/resolve/main/models";

/// Describes what installing `id` involves, against one base URL, or `None` where the listing publishes no file for
/// it.
///
/// Every file backing the artifact becomes a source of its own, each with its own hash: a model too large for the 2 GB
/// protobuf limit keeps its weights in a sibling `.onnx.data`, and both halves have to be transferred and verified for
/// the install to be complete.
pub(crate) fn descriptor_at(
    base_url: &str,
    listing: &Listing,
    id: &ArtifactId,
    app_dir: &Path,
    trust: ModelTrust,
) -> Option<Dependency> {
    // `None` is the refusal, and it is deliberately not an empty descriptor: an artifact the listing does not name is
    // one that is not published, so composing a URL for it would 404 anyway — and a source with no expected hash is the
    // unverified install this whole capability exists to make impossible.
    //
    // Which files back the artifact comes from the listing rather than from probing names on disk — probing would find
    // a file there is no hash for, which is not installable at all.
    //
    // The base URL is an argument rather than a constant so the suite can point a real install at a local server,
    // exactly as `Dependency::from_release_at` takes one.
    let files = listing.files_for(id.as_str());
    if files.is_empty() {
        return None;
    }

    let sources: Vec<Source> = files
        .iter()
        .map(|file| Source {
            url: format!("{}/{}", base_url.trim_end_matches('/'), file.name),
            sha256: file.sha256.clone(),
            size: file.size,
        })
        .collect();

    Some(Dependency {
        name: id.as_str().to_string(),
        // A model has no release tag, so what identifies *which* published bytes are installed is the graph's own
        // hash.
        version: files
            .iter()
            .find(|file| file.name == format!("{id}.onnx"))
            .map_or_else(|| files[0].sha256.clone(), |graph| graph.sha256.clone()),
        dir: model_dir(id),
        progress: Which::Model(id.clone()),
        // A model is not a library the loader is pointed at, and it unlocks no execution provider of its own — it is
        // what the providers already installed are asked to run.
        lib: None,
        webgpu: None,
        provides: None,
        // What a provider compiled from these weights, which a replacement of them must discard: new weights beside
        // an engine built from the old ones produce a wrong image with nothing reporting it.
        //
        // Filled in here rather than patched on afterwards by the one caller that happened to remember. `app_dir` is
        // a parameter for exactly this, as `base_url` is: a descriptor that left this empty would be valid-looking
        // and silently wrong, and the install reads an empty list as "nothing to discard".
        derived: vec![app_dir.join(engine_dir(id))],
        sources,
        // A bare `.onnx` and, where the model is split, a bare weights blob. There is nothing to expand, which is the
        // branch the pipeline has for this.
        contents: Contents::Bare,
        // The one place in this crate where anything but `Published` is ever written; see the comment on
        // `Dependency::trust`.
        trust,
    })
}

/// The directory holding what a provider compiled from `id`, relative to the application's configuration directory:
/// `engines/up_kyoto_4x_fp32`.
pub(crate) fn engine_dir(id: &ArtifactId) -> String {
    // A function rather than a `format!` at three call sites: the descriptor that records this directory as derived
    // from the weights, the installer that empties it on a replacement, and the builder that points a provider at it
    // must all name the same one.
    format!("{ENGINES_DIR}/{id}")
}

/// The directory `id` installs into, relative to the application's configuration directory:
/// `models/up_kyoto_4x_fp32`.
pub(crate) fn model_dir(id: &ArtifactId) -> String {
    // **A directory per model**, where the reference implementation installs every model flat into `models/`. The
    // install pipeline empties a dependency's directory before writing and records everything under it afterwards, so a
    // shared directory would make installing `up_kyoto_4x_fp32` delete the `up_kyoto_2x_fp32` beside it — which is the
    // *other half of the same 8x operation*. It also pre-empts a problem the reference documents and cannot fix: a
    // provider's compiled engine is dropped beside the weights and named after the graph inside the file rather than
    // after the file, so nothing says whose cache is whose. Here it goes into a directory of its own under `engines/`,
    // named after the model it was compiled from — see `engine_dir`.
    format!("{MODELS_DIR}/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::model::manifest::{Published, tests::published};
    use crate::models::test_support::{artifact, artifacts};
    use crate::models::{FloatPrecision, OsakaPrecision, UpscaleVariant};

    /// An application directory to resolve `derived` against.
    fn app_root() -> &'static Path {
        // A literal rather than a temporary: `descriptor_at` only joins onto it and creates nothing, so what a test
        // needs is a path it can then assert against, not a directory on disk.
        Path::new("/app")
    }

    /// The real published listing, as recorded.
    fn listing() -> Listing {
        Listing::new(published())
    }

    /// A listing naming exactly `files`, each with a plausible hash and size.
    fn listing_of(files: &[&str]) -> Listing {
        Listing::new(
            files
                .iter()
                .enumerate()
                .map(|(index, name)| Published {
                    name: (*name).to_string(),
                    size: index as u64 + 1,
                    sha256: format!("{:064x}", index + 1),
                })
                .collect(),
        )
    }

    #[test]
    fn every_descriptor_names_the_engine_directory_its_weights_derive() {
        // The pairing the install must never leave behind: new weights beside an engine compiled from the old ones
        // produce a wrong image and report nothing. `install` discards what `derived` names, so a descriptor that
        // left it empty would read as "nothing to discard" — which is why this is filled in here, by the one function
        // that builds a model descriptor, rather than patched on by whichever caller remembers to.
        for scale in [2.0, 4.0] {
            let id = artifact(UpscaleVariant::Kyoto(FloatPrecision::Fp32), scale);

            let dependency = descriptor_at(MODEL_BASE_URL, &listing(), &id, app_root(), ModelTrust::Published).unwrap();

            assert_eq!(
                dependency.derived,
                vec![app_root().join("engines").join(id.as_str())],
                "{id} did not name the directory its engine is compiled into"
            );
        }
    }

    #[test]
    fn a_single_file_model_resolves_to_one_source_in_a_directory_of_its_own() {
        let id = artifact(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 4.0);

        let dependency = descriptor_at(MODEL_BASE_URL, &listing(), &id, app_root(), ModelTrust::Published).unwrap();

        assert_eq!(dependency.name, "up_kyoto_4x_fp32");
        assert_eq!(dependency.dir, "models/up_kyoto_4x_fp32");
        assert_eq!(dependency.sources.len(), 1);
        assert_eq!(dependency.sources[0].url, format!("{MODEL_BASE_URL}/up_kyoto_4x_fp32.onnx"));
        assert_eq!(dependency.sources[0].file_name(), "up_kyoto_4x_fp32.onnx");
        assert_eq!(dependency.sources[0].sha256.len(), 64);
        assert!(dependency.sources[0].size > 0);
        // Neither a library the loader is opened against nor a provider of its own.
        assert_eq!(dependency.lib, None);
        assert_eq!(dependency.provides, None);
        assert_eq!(dependency.progress, Which::Model(id));
    }

    #[test]
    fn a_model_published_as_a_graph_and_a_weights_blob_resolves_to_both() {
        // Osaka's weights do not fit in the 2 GB protobuf limit, so they sit beside the graph. A descriptor charging
        // only for the graph would transfer half a model and call it installed.
        let id = artifacts(UpscaleVariant::Osaka(OsakaPrecision::Fp16), 4.0)
            .into_iter()
            .find(|id| id.as_str() == "up_osaka_fp16")
            .expect("the Osaka pipeline names its base model");

        let dependency = descriptor_at(MODEL_BASE_URL, &listing(), &id, app_root(), ModelTrust::Published).unwrap();

        let names: Vec<&str> = dependency.sources.iter().map(Source::file_name).collect();
        assert_eq!(names, vec!["up_osaka_fp16.onnx", "up_osaka_fp16.onnx.data"]);
        // Each against its own published hash, rather than the graph's standing in for both.
        let hashes: std::collections::HashSet<&str> =
            dependency.sources.iter().map(|source| source.sha256.as_str()).collect();
        assert_eq!(hashes.len(), 2, "the two halves were given one hash");
        assert!(dependency.sources.iter().all(|source| source.size > 0));
    }

    #[test]
    fn two_artifacts_sharing_a_leading_portion_take_only_their_own_files() {
        // The prefix is `<id>.onnx`, not `<id>`. On `<id>` alone, `up_osaka_fp16` would swallow a future
        // `up_osaka_fp16_v2` — and the wrong weights would be verified against the right hashes.
        let listing = listing_of(&[
            "up_osaka_fp16.onnx",
            "up_osaka_fp16.onnx.data",
            "up_osaka_fp16_v2.onnx",
            "up_osaka_fp16_v2.onnx.data",
        ]);
        let files = |id: &str| listing.files_for(id).iter().map(|file| file.name.clone()).collect::<Vec<_>>();

        assert_eq!(files("up_osaka_fp16"), vec!["up_osaka_fp16.onnx", "up_osaka_fp16.onnx.data"]);
        assert_eq!(files("up_osaka_fp16_v2"), vec!["up_osaka_fp16_v2.onnx", "up_osaka_fp16_v2.onnx.data"]);
    }

    #[test]
    fn each_model_installs_into_a_directory_of_its_own() {
        // The two halves of one 8x Kyoto. A shared `models/` would have the pipeline empty the directory for the
        // second and delete the first — the other half of the same operation.
        let required = artifacts(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 8.0);
        assert_eq!(required.len(), 2, "an 8x Kyoto runs two passes");

        let dirs: Vec<String> = required.iter().map(model_dir).collect();

        assert_eq!(dirs, vec!["models/up_kyoto_4x_fp32", "models/up_kyoto_2x_fp32"]);
        assert!(dirs.iter().all(|dir| dir.starts_with("models/")), "a model escaped the models directory");
    }

    #[test]
    fn an_artifact_the_listing_does_not_name_resolves_to_no_descriptor_at_all() {
        // Not a descriptor with an empty hash: there is nothing here to install unverified against, which is the same
        // answer `from_release_at` gives for a platform no archive is published for.
        let id = artifact(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 4.0);
        let listing = listing_of(&["dn_stockholm_fp32.onnx"]);

        assert!(descriptor_at(MODEL_BASE_URL, &listing, &id, app_root(), ModelTrust::Published).is_none());
    }

    #[test]
    fn the_version_recorded_is_the_graphs_own_hash() {
        // A model has no release tag. The graph's hash is what says which published bytes are installed, and it is
        // taken from the entry named exactly `<id>.onnx` rather than from whichever file happened to sort first.
        let listing = listing_of(&["up_osaka_fp16.onnx.data", "up_osaka_fp16.onnx"]);
        let graph = listing
            .files_for("up_osaka_fp16")
            .iter()
            .find(|f| f.name.ends_with(".onnx"))
            .unwrap()
            .sha256
            .clone();
        let id = artifacts(UpscaleVariant::Osaka(OsakaPrecision::Fp16), 4.0)
            .into_iter()
            .find(|id| id.as_str() == "up_osaka_fp16")
            .unwrap();

        let dependency = descriptor_at(MODEL_BASE_URL, &listing, &id, app_root(), ModelTrust::Published).unwrap();

        assert_eq!(dependency.version, graph);
    }

    #[test]
    fn the_base_url_is_overridable_without_changing_the_file_name() {
        // What lets the pipeline tests serve real model files from a local listener.
        let id = artifact(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 4.0);

        let dependency =
            descriptor_at("http://127.0.0.1:9/models/", &listing(), &id, app_root(), ModelTrust::Published).unwrap();

        assert_eq!(dependency.sources[0].url, "http://127.0.0.1:9/models/up_kyoto_4x_fp32.onnx");
        assert_eq!(dependency.sources[0].file_name(), "up_kyoto_4x_fp32.onnx");
    }

    #[test]
    fn the_published_base_url_is_the_one_the_reference_implementation_uses() {
        // The files themselves are served from `resolve/main`, not from the API the listing is read through.
        assert_eq!(MODEL_BASE_URL, "https://huggingface.co/vegidio/open-photo-ai/resolve/main/models");
    }
}
