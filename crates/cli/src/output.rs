//! Where the results go: `-o` read as a file or a directory before anything is installed, each image's destination
//! under it, and the write through the library's export claim.
//!
//! ```text
//! existing directory          --> Directory
//! extension opai writes       --> File (one input file; no --format, --prefix or --suffix)
//! anything else               --> Directory, refused if a non-directory is there; created on the first write
//! ```

use std::future::Future;
use std::path::{Path, PathBuf};

use opai::export::{self, FormatChoice, Naming};
use opai::image::{self, recommended_encode_options};
use opai::{ImageFormat, Picture};

use crate::args::Args;
use crate::error::CliError;
use crate::inputs::Source;

/// What `-o` names.
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
}

/// What `args.output` names, checked against the inputs and the naming flags. Nothing is created.
///
/// # Errors
///
/// [`CliError::Usage`], naming the path or the flag, where the output is a file but there is more than one input, the
/// input is a directory, or `--format`, `--prefix` or `--suffix` is given; and where the output is a directory but a
/// file that is not one already exists there.
pub fn target(args: &Args) -> Result<Target, CliError> {
    let path = &args.output;

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
/// directory itself, that is the source's own directory.
pub fn destination(target: &Target, source: &Source) -> (PathBuf, ImageFormat) {
    match target {
        Target::File { path, format } => (path.clone(), *format),
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

/// Writes `picture` to `destination` as `format`, at the library's recommended settings for it, and answers the path
/// written.
///
/// Under a directory target, the destination's directory is created first. Without `overwrite` a file already there is
/// kept and the result takes the first free `_1` … `_999` name beside it; with it, the file is replaced once the new
/// one is complete. A write that does not finish leaves nothing behind.
///
/// # Errors
///
/// [`CliError::Claim`] naming `destination` where its directory cannot be created or no name can be held or kept, and
/// [`CliError::Write`] where the encoder fails.
pub async fn write(
    target: &Target,
    picture: &Picture,
    destination: &Path,
    format: ImageFormat,
    overwrite: bool,
) -> Result<PathBuf, CliError> {
    let pixels = picture.shared_pixels();
    let options = recommended_encode_options(format, None);

    write_with(matches!(target, Target::Directory { .. }), destination, overwrite, async |path: PathBuf| {
        image::save(pixels, path, format, Some(options))
            .await
            .map(drop)
            .map_err(|source| CliError::Write(Box::new(source)))
    })
    .await
}

/// [`write`] with the encoding left to `save`, which is handed the path to write into.
async fn write_with<F>(
    create_parent: bool,
    destination: &Path,
    overwrite: bool,
    save: impl FnOnce(PathBuf) -> F,
) -> Result<PathBuf, CliError>
where
    F: Future<Output = Result<(), CliError>>,
{
    let unwritable = |source| CliError::Claim { path: destination.to_path_buf(), source };

    if create_parent && let Some(parent) = destination.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(unwritable)?;
    }

    // Blocking and called directly: it creates one file, and the runtime has nothing else to serve meanwhile. Taken
    // after enhancing, so no empty placeholder sits on disk through the inference.
    let claim = export::claim(destination, overwrite).map_err(unwritable)?;

    // A failure drops the claim, which gives its name back.
    save(claim.path().to_path_buf()).await?;

    // Overwriting, this is where the destination is replaced, and not before.
    claim.commit().map_err(unwritable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::parse_from;
    use opai::export::ExportFormat;

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
    async fn a_failed_save_leaves_nothing_behind() {
        for overwrite in [false, true] {
            let dir = tempfile::tempdir().expect("a temporary directory");
            let out = dir.path().join("out.png");
            std::fs::write(&out, "earlier").expect("written");

            let error = write_with(false, &out, overwrite, async |path: PathBuf| {
                std::fs::write(&path, "half").expect("the claim is writable");
                Err(CliError::Usage("the encoder failed".into()))
            })
            .await
            .expect_err("the save failed");

            assert_eq!(error.to_string(), "the encoder failed");
            assert_eq!(std::fs::read(&out).expect("kept"), b"earlier", "overwrite: {overwrite}");
            assert_eq!(names(dir.path()), ["out.png"], "overwrite: {overwrite}");
        }
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
