//! The images a run enhances: every input expanded, checked and deduplicated before anything is installed.
//!
//! ```text
//! named file       --> probed; a failure refuses the run
//! named directory  --> listed (one level, or the tree with -r) by the extensions the library opens;
//!                      an unlistable tree refuses the run; each file found under the output directory is dropped,
//!                      each one that fails its probe is skipped and logged
//! every image      --> kept at its first occurrence, by canonical path
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use opai::image;
use rust_sak::fs::{ListOptions, list_path};

use crate::error::CliError;

/// One image the run enhances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The path, as given or as the directory listing spelled it.
    pub path: PathBuf,
    /// The directory input it was found under, which its result mirrors; `None` for a file named as an input.
    pub root: Option<PathBuf>,
    /// The width the chain is planned for.
    pub width: u32,
    /// The height the chain is planned for.
    pub height: u32,
}

/// Every image `inputs` lead to, in the order they were given, and in name order within each directory.
///
/// A directory contributes the files directly inside it, or its whole tree when `recursive`, whose extension the
/// library opens; links to directories are not descended into. A file found under `skip` (the output directory,
/// canonical) is dropped. An image reached twice is kept where it was first reached.
///
/// # Errors
///
/// [`CliError::Usage`], naming the path, where a named input does not exist or cannot be read, a named file is not an
/// image this application opens, or a directory input's tree cannot be listed; and where nothing is left to enhance.
pub fn expand(inputs: &[PathBuf], recursive: bool, skip: Option<&Path>) -> Result<Vec<Source>, CliError> {
    let mut seen = HashSet::new();
    // Found files whose probe failed: not reported twice when another directory input reaches them again, but not in
    // `seen` either, so naming one still probes it and refuses the run.
    let mut skipped = HashSet::new();
    let mut sources = Vec::new();

    for input in inputs {
        // Follows a link the user named: a link to a directory is that directory.
        let metadata = std::fs::metadata(input)
            .map_err(|error| CliError::Usage(format!("cannot read {}: {error}", input.display())))?;

        if !metadata.is_dir() {
            if seen.insert(identity(input)) {
                let (width, height) = check_input(input)?;
                sources.push(Source { path: input.clone(), root: None, width, height });
            }
            continue;
        }

        let options = ListOptions::new().recursive(recursive).extensions(image::input_extensions());
        let found = list_path(input, &options)
            .map_err(|error| CliError::Usage(format!("cannot list {}: {error}", input.display())))?;

        // Only an output directory strictly inside this input holds results to keep out. One that is the input itself,
        // or holds it, is writing in place or beside it, and skipping it would skip every image.
        let root = identity(input);
        let skip = skip.filter(|skip| !root.starts_with(skip));

        for path in found {
            let key = identity(&path);
            // Named files are not filtered: a user who names a file inside the output directory asked for it.
            if skip.is_some_and(|skip| key.starts_with(skip)) || seen.contains(&key) || skipped.contains(&key) {
                continue;
            }

            match check_input(&path) {
                Ok((width, height)) => {
                    seen.insert(key);
                    sources.push(Source { path, root: Some(input.clone()), width, height });
                }
                Err(error) => {
                    tracing::warn!("skipping {}: {error}", path.display());
                    skipped.insert(key);
                }
            }
        }
    }

    if sources.is_empty() {
        let named: Vec<_> = inputs.iter().map(|input| input.display().to_string()).collect();
        return Err(CliError::Usage(format!("no image to enhance in {}", named.join(", "))));
    }

    Ok(sources)
}

/// What makes two paths the same image: the canonical path, so `./a.jpg`, `a.jpg` and a path through a linked parent
/// agree, or the path as given where it cannot be canonicalized. Hard links stay distinct.
fn identity(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The width and height of the image at `path`, read from its header without decoding it.
///
/// Camera RAW is recognised by extension and described by its own probe, which reports the size a load develops it
/// to.
///
/// # Errors
///
/// [`CliError::Usage`] where the file does not exist, cannot be read, or is not an image this application opens; the
/// message names the path.
pub fn check_input(path: &Path) -> Result<(u32, u32), CliError> {
    // The blocking forms: this runs before anything else, and reads a header.
    let dimensions = if image::is_raw(path) {
        image::probe_raw_blocking(path).map(|info| (info.width, info.height))
    } else {
        image::probe_blocking(path).map(|info| (info.width, info.height))
    };

    // Every refusal a probe makes already names the path.
    dimensions.map_err(|error| CliError::Usage(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repository's test photograph, a JPEG despite its extension.
    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/test.dat")
    }

    /// Copies the fixture to each of `names` under `dir`, creating their parents.
    fn photos(dir: &Path, names: &[&str]) {
        for name in names {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("the parent is created");
            std::fs::copy(fixture(), &path).expect("the fixture copies");
        }
    }

    /// The paths of `sources`, relative to `dir`.
    fn relative(sources: &[Source], dir: &Path) -> Vec<String> {
        sources
            .iter()
            .map(|source| source.path.strip_prefix(dir).expect("under dir").to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn a_missing_input_is_refused_naming_it() {
        let error = check_input(Path::new("missing.jpg")).expect_err("there is no such file");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("missing.jpg"), "{error}");
    }

    #[test]
    fn an_input_is_recognised_by_its_extension() {
        // `.dat` names no format, so the fixture is refused as it is, though it is a JPEG.
        let error = check_input(&fixture()).expect_err(".dat names no format");
        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("test.dat"), "{error}");

        let dir = tempfile::tempdir().expect("a temporary directory");
        let renamed = dir.path().join("test.jpg");
        std::fs::copy(fixture(), &renamed).expect("the fixture copies");

        let (width, height) = check_input(&renamed).expect("a JPEG is readable");
        let loaded = image::load_blocking(&renamed).expect("decodable");
        assert_eq!((width, height), loaded.dimensions());
    }

    #[test]
    fn a_missing_input_among_several_refuses_the_run() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["a.jpg"]);
        let missing = dir.path().join("missing.jpg");

        let error = expand(&[dir.path().join("a.jpg"), missing], false, None).expect_err("missing.jpg is missing");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("missing.jpg"), "{error}");
    }

    #[test]
    fn a_named_file_that_is_not_an_image_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let notes = dir.path().join("notes.txt");
        std::fs::write(&notes, "not a photograph").expect("written");

        let error = expand(&[notes], false, None).expect_err("a text file is no image");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("notes.txt"), "{error}");
    }

    #[test]
    fn a_named_file_is_a_source_with_no_root_and_its_own_size() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["a.jpg"]);
        let path = dir.path().join("a.jpg");

        let sources = expand(std::slice::from_ref(&path), false, None).expect("one image");

        let (width, height) = check_input(&path).expect("readable");
        assert_eq!(sources, [Source { path, root: None, width, height }]);
    }

    #[test]
    fn a_directory_gives_its_top_level_by_default_and_its_tree_with_recursive() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["photos/b.jpg", "photos/a.jpg", "photos/2024/c.jpg"]);
        let input = dir.path().join("photos");

        let top = expand(std::slice::from_ref(&input), false, None).expect("images");
        assert_eq!(relative(&top, dir.path()), ["photos/a.jpg", "photos/b.jpg"]);
        assert!(top.iter().all(|source| source.root.as_deref() == Some(input.as_path())));

        let tree = expand(std::slice::from_ref(&input), true, None).expect("images");
        assert_eq!(relative(&tree, dir.path()), ["photos/2024/c.jpg", "photos/a.jpg", "photos/b.jpg"]);
    }

    #[test]
    fn a_directory_keeps_the_extensions_the_library_opens_in_any_case() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["a.JPG"]);
        std::fs::write(dir.path().join("notes.txt"), "not a photograph").expect("written");

        let sources = expand(&[dir.path().to_path_buf()], false, None).expect("one image");

        assert_eq!(relative(&sources, dir.path()), ["a.JPG"]);
    }

    #[test]
    fn a_broken_image_inside_a_directory_is_skipped() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["a.jpg"]);
        std::fs::write(dir.path().join("b.jpg"), "not a photograph").expect("written");

        let sources = expand(&[dir.path().to_path_buf()], false, None).expect("a.jpg remains");

        assert_eq!(relative(&sources, dir.path()), ["a.jpg"]);
    }

    #[test]
    fn a_broken_image_skipped_in_a_directory_and_then_named_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["photos/a.jpg"]);
        let broken = dir.path().join("photos/b.jpg");
        std::fs::write(&broken, "not a photograph").expect("written");

        let error = expand(&[dir.path().join("photos"), broken], false, None).expect_err("b.jpg was named");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("b.jpg"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_directory_inside_an_input_is_not_descended_into() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["photos/a.jpg", "elsewhere/b.jpg"]);
        std::os::unix::fs::symlink(dir.path().join("elsewhere"), dir.path().join("photos/linked")).expect("linked");

        let sources = expand(&[dir.path().join("photos")], true, None).expect("images");

        assert_eq!(relative(&sources, dir.path()), ["photos/a.jpg"]);
    }

    #[test]
    fn an_image_reached_twice_is_kept_at_its_first_occurrence() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["photos/a.jpg", "photos/b.jpg"]);
        let photos_dir = dir.path().join("photos");
        let named = photos_dir.join("a.jpg");
        let dotted = photos_dir.join(".").join("a.jpg");

        let sources = expand(&[named.clone(), photos_dir.clone(), dotted, photos_dir], false, None).expect("images");

        assert_eq!(relative(&sources, dir.path()), ["photos/a.jpg", "photos/b.jpg"]);
        assert_eq!(sources[0].root, None, "a.jpg was reached first by name");
        assert_eq!(sources[0].path, named);
    }

    #[test]
    fn the_output_directory_is_not_taken_as_input() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["photos/a.jpg", "photos/out/a.jpg", "photos/out/deep/b.jpg", "photos/outer/c.jpg"]);
        let skip = std::fs::canonicalize(dir.path().join("photos/out")).expect("exists");

        let sources = expand(&[dir.path().join("photos")], true, Some(&skip)).expect("images");

        assert_eq!(relative(&sources, dir.path()), ["photos/a.jpg", "photos/outer/c.jpg"]);

        // A file named inside it is still taken: the user asked for it.
        let named = dir.path().join("photos/out/a.jpg");
        let sources = expand(std::slice::from_ref(&named), true, Some(&skip)).expect("named");
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn an_output_directory_that_is_the_input_or_holds_it_skips_nothing() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["photos/a.jpg", "photos/2024/b.jpg"]);
        let input = dir.path().join("photos");

        for output in [input.clone(), dir.path().to_path_buf()] {
            let skip = std::fs::canonicalize(&output).expect("exists");

            let sources = expand(std::slice::from_ref(&input), true, Some(&skip)).expect("in place");

            assert_eq!(relative(&sources, dir.path()), ["photos/2024/b.jpg", "photos/a.jpg"], "{}", output.display());
        }
    }

    #[test]
    fn a_directory_with_no_images_refuses_the_run() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).expect("created");
        std::fs::write(empty.join("notes.txt"), "not a photograph").expect("written");

        let error = expand(&[empty], true, None).expect_err("nothing to enhance");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("no image to enhance"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_tree_that_cannot_be_listed_refuses_the_run_naming_the_input() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("a temporary directory");
        photos(dir.path(), &["photos/a.jpg", "photos/locked/b.jpg"]);
        let locked = dir.path().join("photos/locked");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("locked");

        // Root reads anything, so there is nothing to refuse there.
        let unreadable = std::fs::read_dir(&locked).is_err();
        let result = expand(&[dir.path().join("photos")], true, None);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("unlocked");

        if unreadable {
            let error = result.expect_err("the tree cannot be listed");
            assert_eq!(error.exit_code(), 2);
            assert!(error.to_string().contains("photos"), "{error}");
        }
    }
}
