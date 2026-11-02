//! Where the results go: `-o` read as a file or a directory before anything is installed, or its absence as "beside
//! each source", each image's destination, and the write through the library's export claim.
//!
//! ```text
//! no -o                       --> Beside (suffix _opai unless --prefix or --suffix is given; nothing created; another
//!                                 input is never overwritten)
//! existing directory          --> Directory
//! extension opai writes       --> File (one input file; no --format, --prefix or --suffix)
//! anything else               --> Directory, refused if a non-directory is there; created on the first write
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use opai::export::{self, FormatChoice, Naming, WriteOptions};
use opai::{ImageFormat, Picture};

use crate::args::Args;
use crate::error::CliError;
use crate::inputs::Source;

/// The suffix a result written beside its source takes when neither `--prefix` nor `--suffix` is given, so that its
/// name, the only thing telling it from its source, does.
pub const BESIDE_SUFFIX: &str = "_opai";

/// What `-o` names, or where the results go without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// One file, written in the format its extension names.
    File {
        /// The path, exactly as given.
        path: PathBuf,
        /// The format the path's extension names.
        format: ImageFormat,
    },
    /// A directory every result is written under, named by the library's export rule.
    Directory {
        /// The path, exactly as given. It may not exist yet.
        path: PathBuf,
        /// The format choice and the affixes, defaulting to Preserve and nothing.
        naming: Naming,
    },
    /// No `-o`: each result is written in the directory that holds its source, named by the library's export rule.
    Beside {
        /// The format choice and the affixes, defaulting to Preserve and the suffix [`BESIDE_SUFFIX`].
        naming: Naming,
        /// Every input of the run, canonicalised by [`keep_inputs`], none of which a result may replace but its own.
        inputs: HashSet<PathBuf>,
    },
}

/// What `args.output` names, checked against the inputs and the naming flags, or [`Target::Beside`] without one.
/// Nothing is created.
///
/// # Errors
///
/// [`CliError::Usage`], naming the path or the flag, where the output is a file but there is more than one input, the
/// input is a directory, or `--format`, `--prefix` or `--suffix` is given; and where the output is a directory but a
/// file that is not one already exists there.
pub fn target(args: &Args) -> Result<Target, CliError> {
    let Some(path) = &args.output else {
        // Beside its source, a result with no affix would take its source's own name. Either flag given, an empty one
        // included, is the user's choice of name, the other affix staying empty.
        let suffix = match (&args.prefix, &args.suffix) {
            (None, None) => BESIDE_SUFFIX.to_owned(),
            (_, suffix) => suffix.clone().unwrap_or_default(),
        };
        let naming = Naming {
            prefix: args.prefix.clone().unwrap_or_default(),
            suffix,
            format: args.format.unwrap_or(FormatChoice::Preserve),
        };
        return Ok(Target::Beside { naming, inputs: HashSet::new() });
    };

    // An existing directory wins whatever its name, so `-o shots.png` can still mean the directory the user made.
    if !path.is_dir()
        && let Some(format) = ImageFormat::from_path(path)
    {
        let directory_input = args.inputs.iter().any(|input| input.is_dir());
        if args.inputs.len() > 1 || directory_input {
            return Err(CliError::Usage(format!(
                "an output file takes exactly one input file; {} is a file because of its extension, so name a \
                 directory to write several results into",
                path.display()
            )));
        }

        for (flag, given) in [
            ("--format", args.format.is_some()),
            ("--prefix", args.prefix.is_some()),
            ("--suffix", args.suffix.is_some()),
        ] {
            if given {
                return Err(CliError::Usage(format!(
                    "{flag} needs an output directory; {} is a file, whose extension chooses the format",
                    path.display()
                )));
            }
        }

        return Ok(Target::File { path: path.clone(), format });
    }

    if path.exists() && !path.is_dir() {
        return Err(CliError::Usage(format!(
            "cannot write into {}: it is a file, not a directory, and its extension names no format opai writes",
            path.display()
        )));
    }

    let naming = Naming {
        prefix: args.prefix.clone().unwrap_or_default(),
        suffix: args.suffix.clone().unwrap_or_default(),
        format: args.format.unwrap_or(FormatChoice::Preserve),
    };
    Ok(Target::Directory { path: path.clone(), naming })
}

/// Where `source`'s result is asked to be written, and in which format.
///
/// A file target is its own path. Under a directory, an image found in a directory input keeps its path relative to
/// that input, and a named file goes directly inside; the name is the library's export rule's. With `-o` the input
/// directory itself, that is the source's own directory. Beside its source, it is in the directory that holds the
/// source, whether the source was named or found.
pub fn destination(target: &Target, source: &Source) -> (PathBuf, ImageFormat) {
    match target {
        Target::File { path, format } => (path.clone(), *format),
        Target::Beside { naming, .. } => {
            let (destination, named) = export::destination_for(&source.path, naming, None);
            (destination, named.format.into())
        }
        Target::Directory { path, naming } => {
            let relative = source
                .root
                .as_deref()
                .and_then(|root| source.path.strip_prefix(root).ok())
                .and_then(Path::parent)
                .unwrap_or(Path::new(""));
            let folder = path.join(relative);

            let (destination, named) = export::destination_for(&source.path, naming, Some(&folder));
            (destination, named.format.into())
        }
    }
}

/// Records `sources` as the inputs a result written beside its source must never replace. Only a [`Target::Beside`]
/// keeps them: under `-o` the inputs are the user's to place the output over, as upstream has always let them.
///
/// Canonicalised, so a source reached through another spelling or a link is still recognised; one that cannot be is
/// kept as given.
pub fn keep_inputs<'a>(target: &mut Target, sources: impl IntoIterator<Item = &'a Path>) {
    if let Target::Beside { inputs, .. } = target {
        inputs.extend(sources.into_iter().map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned())));
    }
}

/// Whether `--overwrite` may replace what is at `destination`, the result of `source`: never where that is another
/// input of a run beside its sources, which would destroy a photograph the user gave, or one not yet read. Its own
/// source may be replaced, which takes both `--suffix ""` (or affixes naming it) and `--overwrite`.
pub fn may_overwrite(target: &Target, source: &Path, destination: &Path) -> bool {
    let Target::Beside { inputs, .. } = target else {
        return true;
    };
    // A destination that does not exist yet replaces nothing, and so is no input.
    let Ok(destination) = std::fs::canonicalize(destination) else {
        return true;
    };

    !inputs.contains(&destination) || std::fs::canonicalize(source).is_ok_and(|source| source == destination)
}

/// Writes `picture` to `destination` as `format`, at the library's recommended settings for it, and answers the path
/// written.
///
/// Under a directory target, the destination's directory is created first; beside its source, it exists already.
/// Without `overwrite` a file already there is
/// kept and the result takes the first free `_1` … `_999` name beside it; with it, the file is replaced once the new
/// one is complete. A write that does not finish leaves nothing behind.
///
/// # Errors
///
/// [`CliError::Write`], naming `destination` where its directory cannot be created or no name can be held or kept,
/// or the output path where the encoder fails.
pub async fn write(
    target: &Target,
    picture: &Picture,
    destination: &Path,
    format: ImageFormat,
    overwrite: bool,
) -> Result<PathBuf, CliError> {
    let options = WriteOptions { quality: None, overwrite, create_parent: matches!(target, Target::Directory { .. }) };

    export::write(picture, destination, format, options)
        .await
        .map(|written| written.path)
        .map_err(|error| CliError::Write(Box::new(error)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::parse_from;
    use opai::export::ExportFormat;
    use opai::image;

    fn args(line: &str) -> Args {
        parse_from(line.split_whitespace()).expect("parses")
    }

    /// `line` with `{dir}` replaced by `dir`.
    fn args_in(dir: &Path, line: &str) -> Args {
        args(&line.replace("{dir}", &dir.display().to_string()))
    }

    fn unaffixed(format: FormatChoice) -> Naming {
        Naming { prefix: String::new(), suffix: String::new(), format }
    }

    fn named(path: &str) -> Source {
        Source { path: path.into(), root: None, width: 1, height: 1 }
    }

    fn found(root: &str, path: &str) -> Source {
        Source { path: path.into(), root: Some(root.into()), width: 1, height: 1 }
    }

    fn directory(path: &str, naming: Naming) -> Target {
        Target::Directory { path: path.into(), naming }
    }

    #[test]
    fn a_writable_extension_is_a_file_whatever_its_case() {
        for (name, format) in
            [("out.webp", ImageFormat::WebP), ("out.PNG", ImageFormat::Png), ("OUT.JPG", ImageFormat::Jpeg)]
        {
            let target = target(&args(&format!("opai photo.jpg --up -o {name}"))).expect(name);

            assert_eq!(target, Target::File { path: name.into(), format }, "{name}");
        }
    }

    #[test]
    fn any_other_path_is_a_directory_with_the_default_naming() {
        for name in ["out", "out.xyz"] {
            let target = target(&args(&format!("opai photo.jpg --up -o {name}"))).expect(name);

            assert_eq!(target, directory(name, unaffixed(FormatChoice::Preserve)), "{name}");
            assert!(!Path::new(name).exists(), "{name} was created");
        }
    }

    #[test]
    fn an_existing_directory_is_a_directory_whatever_its_name() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(dir.path().join("shots.png")).expect("created");

        let target = target(&args_in(dir.path(), "opai photo.jpg --up -o {dir}/shots.png")).expect("a directory");

        assert!(matches!(target, Target::Directory { .. }), "{target:?}");
    }

    #[test]
    fn a_file_where_a_directory_is_asked_for_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(dir.path().join("out"), "a file").expect("written");

        let error = target(&args_in(dir.path(), "opai photo.jpg --up -o {dir}/out")).expect_err("out is a file");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("not a directory"), "{error}");
    }

    #[test]
    fn a_file_output_takes_exactly_one_input_file() {
        let error = target(&args("opai a.jpg b.jpg --up -o out.png")).expect_err("two inputs");
        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("an output file takes exactly one input file"), "{error}");

        let dir = tempfile::tempdir().expect("a temporary directory");
        let error = target(&args_in(dir.path(), "opai {dir} --up -o out.png")).expect_err("a directory input");
        assert!(error.to_string().contains("an output file takes exactly one input file"), "{error}");
    }

    #[test]
    fn the_naming_flags_need_an_output_directory() {
        for (flags, flag) in
            [("--format png", "--format"), ("--prefix new-", "--prefix"), ("--suffix -big", "--suffix")]
        {
            let error = target(&args(&format!("opai photo.jpg --up {flags} -o out.png"))).expect_err(flags);

            assert_eq!(error.exit_code(), 2);
            assert!(error.to_string().contains(&format!("{flag} needs an output directory")), "{error}");
        }
    }

    #[test]
    fn the_naming_flags_reach_a_directory() {
        let target = target(&args("opai photo.jpg --up --prefix new- --suffix -opai --format JPG -o out")).expect("ok");

        let naming = Naming { prefix: "new-".into(), suffix: "-opai".into(), format: ExportFormat::Jpeg.into() };
        assert_eq!(target, directory("out", naming));
    }

    #[test]
    fn a_file_target_is_written_where_it_was_named() {
        let target = Target::File { path: "out.webp".into(), format: ImageFormat::WebP };

        assert_eq!(destination(&target, &named("photos/a.jpg")), (PathBuf::from("out.webp"), ImageFormat::WebP));
    }

    #[test]
    fn a_tree_is_mirrored_and_named_files_go_flat() {
        let target = directory("out", unaffixed(FormatChoice::Preserve));

        assert_eq!(destination(&target, &found("photos", "photos/a.jpg")).0, Path::new("out/a.jpg"));
        assert_eq!(
            destination(&target, &found("photos", "photos/2024/summer/b.jpg")).0,
            Path::new("out/2024/summer/b.jpg")
        );
        assert_eq!(destination(&target, &named("trips/2024/a.jpg")).0, Path::new("out/a.jpg"));
        assert_eq!(destination(&target, &named("home/b.jpg")).0, Path::new("out/b.jpg"));
    }

    #[test]
    fn the_input_directory_as_output_is_in_place() {
        let target = directory("photos", unaffixed(FormatChoice::Preserve));

        let source = found("photos", "photos/2024/a.jpg");
        assert_eq!(destination(&target, &source).0, source.path);
    }

    #[test]
    fn preserve_writes_each_source_as_its_own_format_and_raw_as_tiff() {
        let target = directory("out", unaffixed(FormatChoice::Preserve));

        assert_eq!(destination(&target, &named("a.jpg")), (PathBuf::from("out/a.jpg"), ImageFormat::Jpeg));
        assert_eq!(destination(&target, &named("b.nef")), (PathBuf::from("out/b.tiff"), ImageFormat::Tiff));
    }

    #[test]
    fn the_affixes_and_the_format_compose_the_name() {
        let naming = Naming { prefix: "new-".into(), suffix: "-opai".into(), format: ExportFormat::Webp.into() };
        let target = directory("out", naming);

        assert_eq!(
            destination(&target, &found("photos", "photos/2024/beach.jpg")),
            (PathBuf::from("out/2024/new-beach-opai.webp"), ImageFormat::WebP)
        );
    }

    fn beside(prefix: &str, suffix: &str, format: FormatChoice) -> Target {
        Target::Beside {
            naming: Naming { prefix: prefix.into(), suffix: suffix.into(), format },
            inputs: HashSet::new(),
        }
    }

    #[test]
    fn no_output_is_beside_each_source_with_the_suffix_opai() {
        let target = target(&args("opai photo.jpg --ap")).expect("no -o");

        assert_eq!(target, beside("", "_opai", FormatChoice::Preserve));
    }

    #[test]
    fn a_prefix_or_a_suffix_replaces_the_default_suffix() {
        assert_eq!(
            target(&args("opai photo.jpg --up --suffix -big")).expect("ok"),
            beside("", "-big", FormatChoice::Preserve)
        );
        assert_eq!(
            target(&args("opai photo.jpg --up --prefix new-")).expect("ok"),
            beside("new-", "", FormatChoice::Preserve)
        );

        let empty = parse_from(["opai", "photo.jpg", "--dn", "--suffix", ""]).expect("parses");
        assert_eq!(target(&empty).expect("ok"), beside("", "", FormatChoice::Preserve));
    }

    #[test]
    fn a_format_is_accepted_without_an_output() {
        let target = target(&args("opai shot.nef --dn --format webp")).expect("ok");

        assert_eq!(target, beside("", "_opai", ExportFormat::Webp.into()));
        assert_eq!(destination(&target, &named("shot.nef")), (PathBuf::from("shot_opai.webp"), ImageFormat::WebP));
    }

    #[test]
    fn beside_writes_into_each_sources_own_directory() {
        let target = beside("", "_opai", FormatChoice::Preserve);

        assert_eq!(destination(&target, &named("photo.jpg")), (PathBuf::from("photo_opai.jpg"), ImageFormat::Jpeg));
        assert_eq!(destination(&target, &named("pictures/beach.jpg")).0, Path::new("pictures/beach_opai.jpg"));
        assert_eq!(destination(&target, &found("photos", "photos/a.jpg")).0, Path::new("photos/a_opai.jpg"));
        assert_eq!(
            destination(&target, &found("photos", "photos/2024/b.jpg")).0,
            Path::new("photos/2024/b_opai.jpg")
        );
        assert_eq!(destination(&target, &named("/abs/c.nef")).0, Path::new("/abs/c_opai.tiff"));
    }

    #[test]
    fn the_default_suffix_never_names_the_source_itself() {
        let target = beside("", "_opai", FormatChoice::Preserve);

        for path in ["photo.jpg", "photo_opai.jpg", "a.b.jpg", ".hidden.jpg", "noext", "dir/PHOTO.JPG"] {
            let (destination, _) = destination(&target, &named(path));

            assert_ne!(destination.to_string_lossy().to_lowercase(), path.to_lowercase(), "{path}");
            assert_eq!(destination.parent(), Path::new(path).parent(), "{path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_source_whose_name_is_not_utf8_is_named_beside_it() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = Path::new("photos").join(OsStr::from_bytes(b"caf\xe9.jpg"));
        let source = Source { path, root: None, width: 1, height: 1 };

        let (destination, _) = destination(&beside("", "_opai", FormatChoice::Preserve), &source);

        assert_eq!(destination, Path::new("photos").join(OsStr::from_bytes(b"caf\xe9_opai.jpg")));
    }

    /// The repository's test photograph, loaded.
    fn picture() -> Picture {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/test.dat");
        let dir = tempfile::tempdir().expect("a temporary directory");
        let renamed = dir.path().join("test.jpg");
        std::fs::copy(fixture, &renamed).expect("the fixture copies");
        image::load_blocking(&renamed).expect("decodable")
    }

    /// The names in `dir`, sorted.
    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .expect("listable")
            .map(|entry| entry.expect("readable").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn an_existing_file_is_kept_and_the_result_numbered() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("out.png");
        std::fs::write(&out, "earlier").expect("written");
        let target = Target::File { path: out.clone(), format: ImageFormat::Png };

        let written = write(&target, &picture(), &out, ImageFormat::Png, false).await.expect("written");

        assert_eq!(written, dir.path().join("out_1.png"));
        assert_eq!(std::fs::read(&out).expect("kept"), b"earlier");
        assert!(image::probe_blocking(&written).is_ok(), "the numbered file is an image");
    }

    #[tokio::test]
    async fn overwrite_replaces_the_file_and_leaves_nothing_beside_it() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("out.png");
        std::fs::write(&out, "earlier").expect("written");
        let target = Target::File { path: out.clone(), format: ImageFormat::Png };

        let written = write(&target, &picture(), &out, ImageFormat::Png, true).await.expect("written");

        assert_eq!(written, out);
        assert!(image::probe_blocking(&out).is_ok(), "the file was replaced by an image");
        assert_eq!(names(dir.path()), ["out.png"]);
    }

    #[tokio::test]
    async fn a_directory_target_creates_the_mirrored_folder() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("out");
        let target = directory(&out.display().to_string(), unaffixed(FormatChoice::Preserve));
        let destination = out.join("2024/a.jpg");

        let written = write(&target, &picture(), &destination, ImageFormat::Jpeg, false).await.expect("written");

        assert_eq!(written, destination);
        assert!(image::probe_blocking(&written).is_ok());
    }

    #[tokio::test]
    async fn beside_keeps_an_earlier_result_and_numbers_the_new_one() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let source = dir.path().join("photo.jpg");
        let earlier = dir.path().join("photo_opai.jpg");
        std::fs::write(&source, "source").expect("written");
        std::fs::write(&earlier, "earlier").expect("written");
        let target = beside("", "_opai", FormatChoice::Preserve);
        let (destination, format) = destination(&target, &named(&source.display().to_string()));

        let written = write(&target, &picture(), &destination, format, false).await.expect("written");

        assert_eq!(written, dir.path().join("photo_opai_1.jpg"));
        assert_eq!(std::fs::read(&earlier).expect("kept"), b"earlier");
        assert_eq!(std::fs::read(&source).expect("kept"), b"source");
        assert!(image::probe_blocking(&written).is_ok(), "the numbered file is an image");
    }

    #[tokio::test]
    async fn beside_with_overwrite_replaces_the_earlier_result_and_not_the_source() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let source = dir.path().join("photo.jpg");
        let earlier = dir.path().join("photo_opai.jpg");
        std::fs::write(&source, "source").expect("written");
        std::fs::write(&earlier, "earlier").expect("written");
        let target = beside("", "_opai", FormatChoice::Preserve);
        let (destination, format) = destination(&target, &named(&source.display().to_string()));

        let written = write(&target, &picture(), &destination, format, true).await.expect("written");

        assert_eq!(written, earlier);
        assert!(image::probe_blocking(&earlier).is_ok(), "the earlier result was replaced by an image");
        assert_eq!(std::fs::read(&source).expect("kept"), b"source");
        assert_eq!(names(dir.path()), ["photo.jpg", "photo_opai.jpg"]);
    }

    /// A directory holding `names`, each a small file, and the beside target that keeps them all as inputs.
    fn inputs_in(names: &[&str]) -> (tempfile::TempDir, Target) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        for name in names {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("created");
            std::fs::write(path, *name).expect("written");
        }
        let mut target = beside("", "_opai", FormatChoice::Preserve);
        let paths: Vec<PathBuf> = names.iter().map(|name| dir.path().join(name)).collect();
        keep_inputs(&mut target, paths.iter().map(PathBuf::as_path));
        (dir, target)
    }

    #[test]
    fn overwrite_never_replaces_another_input_in_either_order() {
        let (dir, target) = inputs_in(&["a.jpg", "a_opai.jpg"]);
        let (a, a_opai) = (dir.path().join("a.jpg"), dir.path().join("a_opai.jpg"));

        // `a.jpg`'s result is named `a_opai.jpg`, which is an input, whichever of the two is given first.
        assert!(!may_overwrite(&target, &a, &a_opai));
        // `a_opai.jpg`'s own result, `a_opai_opai.jpg`, is no input.
        assert!(may_overwrite(&target, &a_opai, &dir.path().join("a_opai_opai.jpg")));
    }

    #[test]
    fn a_rerun_over_a_tree_does_not_replace_the_earlier_results_it_takes_as_inputs() {
        let (dir, target) = inputs_in(&["a.jpg", "a_opai.jpg", "2024/b.jpg", "2024/b_opai.jpg"]);

        assert!(!may_overwrite(&target, &dir.path().join("a.jpg"), &dir.path().join("a_opai.jpg")));
        assert!(!may_overwrite(&target, &dir.path().join("2024/b.jpg"), &dir.path().join("2024/b_opai.jpg")));
    }

    #[test]
    fn overwrite_may_replace_a_result_that_is_no_input_or_the_images_own_source() {
        let (dir, target) = inputs_in(&["a.jpg"]);
        let a = dir.path().join("a.jpg");
        std::fs::write(dir.path().join("a_opai.jpg"), "earlier").expect("written");

        assert!(may_overwrite(&target, &a, &dir.path().join("a_opai.jpg")), "an earlier result that is no input");
        assert!(may_overwrite(&target, &a, &dir.path().join("missing.jpg")), "nothing there");
        assert!(may_overwrite(&target, &a, &a), "in place, asked for with --suffix \"\"");
    }

    #[test]
    fn an_output_directory_keeps_no_inputs() {
        let mut target = directory("out", unaffixed(FormatChoice::Preserve));
        keep_inputs(&mut target, [Path::new("a.jpg")]);

        assert_eq!(target, directory("out", unaffixed(FormatChoice::Preserve)));
        assert!(may_overwrite(&target, Path::new("b.jpg"), Path::new("a.jpg")));
    }

    #[tokio::test]
    async fn beside_creates_no_directory() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let destination = dir.path().join("gone/photo_opai.png");
        let target = beside("", "_opai", FormatChoice::Preserve);

        let error = write(&target, &picture(), &destination, ImageFormat::Png, false)
            .await
            .expect_err("no directory");

        assert_eq!(error.exit_code(), 1);
        assert!(!dir.path().join("gone").exists());
    }

    #[tokio::test]
    async fn a_missing_directory_for_a_file_target_names_the_destination() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("missing/out.png");
        let target = Target::File { path: out.clone(), format: ImageFormat::Png };

        let error = write(&target, &picture(), &out, ImageFormat::Png, false).await.expect_err("no directory");

        assert_eq!(error.exit_code(), 1);
        assert!(error.to_string().contains("missing/out.png"), "{error}");
        assert!(!dir.path().join("missing").exists());
    }
}
