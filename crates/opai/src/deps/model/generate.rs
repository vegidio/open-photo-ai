//! The generator for `published.json`, the listing compiled into the binary.
//!
//! # Running it
//!
//! ```text
//! cargo test -p opai --lib regenerate_the_compiled_in_listing -- --ignored --nocapture
//! ```
//!
//! It reads the live listing, keeps the entries this build can name, writes `published.json` beside this file, and
//! prints what it dropped. **Re-run it when models are published**, and commit the result — that is the whole of the
//! contract. Nothing regenerates it automatically.
//!
//! Compiled only under `cfg(test)` — it is a maintenance tool, not part of the library.

// Not a `build.rs`: one that fetched from Hugging Face would put the network on the build path, break offline and
// sandboxed builds, and make a binary's contents depend on when it was compiled rather than on what is checked in.
//
// Run by hand through an ignored test rather than through a binary of its own, because a binary in this package cannot
// see crate-internal items: the generator has to share `Listing::parse_tree_page` with the run-time parser, or the two
// could disagree about what the endpoint said, and exposing the parser publicly to regenerate a data file would widen a
// deliberately narrow API for a maintenance chore. The output is proved byte-identical to what is checked in by
// `tests::the_generator_reproduces_what_is_checked_in`.

use std::collections::BTreeSet;

use super::manifest::{Listing, Published};

/// Where the generated listing is written, relative to the crate's source root.
const OUTPUT_PATH: &str = "src/deps/model/published.json";

/// Renders the listing exactly as it is checked in: only the entries this build can name, ordered by name, pretty
/// printed, with a closing newline.
pub(crate) fn render(files: &[Published]) -> String {
    // Sorted and pretty printed so the checked-in file reviews as a diff rather than as one line — the endpoint's own
    // order is not something to depend on, and a hash changing has to be visible.
    //
    // Filtered to what the catalogue can name: a compiled-in fallback exists so a machine with no network can still
    // install the models **this build can ask for**, so an artifact no build can ask for is dead weight in it.
    let nameable = crate::models::test_support::every_artifact_name();

    let mut kept: Vec<Published> =
        files.iter().filter(|file| nameable.contains(artifact_of(&file.name))).cloned().collect();
    kept.sort_by(|a, b| a.name.cmp(&b.name));

    let mut json = serde_json::to_string_pretty(&Listing::new(kept)).expect("a listing serializes");
    json.push('\n');
    json
}

/// The artifact a published file backs: its name with the `.onnx` and anything after it removed.
///
/// `up_osaka_fp16.onnx` and `up_osaka_fp16.onnx.data` both answer `up_osaka_fp16`, which is what groups the two
/// halves of a split model under one artifact.
fn artifact_of(name: &str) -> &str {
    name.split_once(".onnx").map_or(name, |(stem, _)| stem)
}

/// The published files this build cannot name, for the generator to report.
fn unnameable(files: &[Published]) -> BTreeSet<&str> {
    let nameable = crate::models::test_support::every_artifact_name();

    files
        .iter()
        .map(|file| file.name.as_str())
        .filter(|name| !nameable.contains(artifact_of(name)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::model::manifest::{TREE_URL, tests::TREE_MODELS};

    #[test]
    fn the_generator_reproduces_what_is_checked_in() {
        // What keeps the generator and the run-time parser from drifting: both go through `parse_tree_page`, and the
        // bytes this produces for the recorded response are the bytes in the file. A change to either that the other
        // does not follow fails here rather than at a user's first offline install.
        let files = Listing::parse_tree_page(TREE_MODELS).unwrap();

        assert_eq!(
            render(&files),
            include_str!("published.json"),
            "`published.json` is not what the generator produces for the recorded response - re-run it"
        );
    }

    #[test]
    fn the_generator_drops_what_this_build_cannot_name_and_can_say_so() {
        // The signal for a newly published model, in place of a suite failure: the generator reports it, at the moment
        // a human runs the generator.
        let files = Listing::parse_tree_page(TREE_MODELS).unwrap();

        let dropped = unnameable(&files);
        let rendered: Listing = serde_json::from_str(&render(&files)).unwrap();

        assert!(!dropped.is_empty(), "the recorded response names nothing unnameable, so this passes vacuously");
        for name in &dropped {
            assert!(
                !rendered.files().iter().any(|file| &file.name == name),
                "{name} cannot be named by this build and was compiled in anyway"
            );
        }
    }

    #[test]
    fn an_artifact_is_taken_from_a_file_name_by_cutting_at_the_extension() {
        assert_eq!(artifact_of("up_kyoto_4x_fp32.onnx"), "up_kyoto_4x_fp32");
        // Both halves of a split model answer the same artifact, which is what groups them.
        assert_eq!(artifact_of("up_osaka_fp16.onnx"), "up_osaka_fp16");
        assert_eq!(artifact_of("up_osaka_fp16.onnx.data"), "up_osaka_fp16");
        // A name that is not a model file at all is its own answer rather than a panic.
        assert_eq!(artifact_of("README.md"), "README.md");
    }

    /// Regenerates `published.json` from the live listing. Not part of the suite — see the module documentation for
    /// the command and for when to run it.
    #[tokio::test]
    #[ignore = "the manifest generator: reaches the network and rewrites a checked-in file"]
    async fn regenerate_the_compiled_in_listing() {
        let listing = Listing::read_live(TREE_URL).await.expect("the live listing could not be read");
        let files = listing.files();

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(OUTPUT_PATH);
        std::fs::write(&path, render(files)).expect("the generated listing could not be written");

        println!("wrote {} entries to {}", render(files).matches("\"name\"").count(), path.display());
        for name in unnameable(files) {
            println!("dropped {name}: this build cannot name it");
        }
    }
}
