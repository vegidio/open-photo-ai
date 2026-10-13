//! The name and the destination a source is exported under.
//!
//! **Pure**: nothing here stats the source or the folder, so it cannot fail and is cheap enough to ask on every
//! keystroke. What it gives is the destination *asked for*; [`claim`](super::claim()) decides what is written, a
//! numbered name beside it when it is taken.
//!
//! Names are [`OsString`], so a source whose name is not UTF-8 names its export correctly on disk. A front end that
//! shows one converts it for display.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::format::{ExportFormat, FormatChoice};

/// What decides the name of each file written: the affixes around the source's name, and the format choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Naming {
    /// Written before the source's name.
    pub prefix: String,
    /// Written after the source's name, before the extension.
    pub suffix: String,
    /// The format to write, or Preserve.
    pub format: FormatChoice,
}

/// What a source is exported as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    /// The file name: the prefix, the source's name without its last extension, the suffix, a dot and
    /// [`extension`](Self::extension).
    pub name: OsString,
    /// The format the file is written in.
    pub format: ExportFormat,
    /// The extension the file is written with, without its dot.
    ///
    /// A chosen format's own [`extension`](ExportFormat::extension). Under Preserve, the source's own extension in
    /// lower case where its format is written back — aliases included, so `.JPEG` stays `jpeg` — and `tiff` for the
    /// rest.
    pub extension: String,
}

/// What `source` is exported as under `naming`.
///
/// ```
/// use std::path::Path;
///
/// use opai::export::{ExportFormat, FormatChoice, Naming, name_for};
///
/// let naming = Naming { prefix: "new-".into(), suffix: "-opai".into(), format: FormatChoice::Preserve };
/// let named = name_for(Path::new("/photos/shot.nef"), &naming);
///
/// assert_eq!(named.name, "new-shot-opai.tiff");
/// assert_eq!(named.format, ExportFormat::Tiff);
/// ```
pub fn name_for(source: &Path, naming: &Naming) -> Named {
    let (format, extension) = match naming.format {
        FormatChoice::Format(format) => (format, format.extension().to_owned()),
        FormatChoice::Preserve => {
            let own = source.extension().and_then(|extension| extension.to_str()).map(str::to_lowercase);

            match own.as_deref().and_then(ExportFormat::preserving) {
                Some(format) => (format, own.unwrap_or_default()),
                None => (FormatChoice::FALLBACK, FormatChoice::FALLBACK.extension().to_owned()),
            }
        }
    };

    // `file_stem` removes only the last extension, so `2026.09.24 beach.jpg` keeps its other dots.
    let mut name = OsString::from(&naming.prefix);
    name.push(source.file_stem().unwrap_or_default());
    name.push(&naming.suffix);
    name.push(".");
    name.push(&extension);

    Named { name, format, extension }
}

/// The path the export of `source` asks to write, and what it is exported as: [`name_for`] inside `folder`, or inside
/// the directory `source` is in where no folder is chosen.
///
/// A bare relative name (`beach.jpg`) has no directory, so it gives a bare name.
///
/// ```
/// use std::path::Path;
///
/// use opai::export::{ExportFormat, Naming, destination_for};
///
/// let naming = Naming { prefix: String::new(), suffix: String::new(), format: ExportFormat::Png.into() };
///
/// assert_eq!(destination_for(Path::new("/photos/beach.jpg"), &naming, None).0, Path::new("/photos/beach.png"));
/// ```
pub fn destination_for(source: &Path, naming: &Naming, folder: Option<&Path>) -> (PathBuf, Named) {
    let named = name_for(source, naming);
    let folder = folder.or_else(|| source.parent()).unwrap_or(Path::new(""));

    (folder.join(&named.name), named)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naming(prefix: &str, suffix: &str, format: FormatChoice) -> Naming {
        Naming { prefix: prefix.to_owned(), suffix: suffix.to_owned(), format }
    }

    fn unaffixed(format: FormatChoice) -> Naming {
        naming("", "", format)
    }

    fn chosen(format: ExportFormat) -> FormatChoice {
        FormatChoice::Format(format)
    }

    #[test]
    fn preserve_keeps_a_writable_source_as_its_own_format_and_extension_lower_cased() {
        for (path, format, extension) in [
            ("/photos/beach.jpg", ExportFormat::Jpeg, "jpg"),
            ("/photos/beach.JPEG", ExportFormat::Jpeg, "jpeg"),
            ("/photos/beach.png", ExportFormat::Png, "png"),
            ("/photos/beach.heif", ExportFormat::Heic, "heif"),
            ("/photos/beach.heic", ExportFormat::Heic, "heic"),
            ("/photos/beach.tif", ExportFormat::Tiff, "tif"),
        ] {
            let named = name_for(Path::new(path), &unaffixed(FormatChoice::Preserve));

            assert_eq!((named.format, named.extension.as_str()), (format, extension), "{path}");
        }
    }

    #[test]
    fn preserve_writes_a_raw_source_as_tiff() {
        let named = name_for(Path::new("/photos/shot.nef"), &unaffixed(FormatChoice::Preserve));

        assert_eq!((named.format, named.extension.as_str()), (ExportFormat::Tiff, "tiff"));
        assert_eq!(named.name, "shot.tiff");
    }

    #[test]
    fn preserve_writes_a_source_with_no_extension_as_tiff() {
        let named = name_for(Path::new("/photos/shot"), &unaffixed(FormatChoice::Preserve));

        assert_eq!((named.format, named.extension.as_str()), (ExportFormat::Tiff, "tiff"));
        assert_eq!(named.name, "shot.tiff");
    }

    #[test]
    fn a_chosen_format_writes_its_own_format_and_extension_whatever_the_source() {
        for (format, extension) in [
            (ExportFormat::Avif, "avif"),
            (ExportFormat::Bmp, "bmp"),
            (ExportFormat::Gif, "gif"),
            (ExportFormat::Heic, "heic"),
            (ExportFormat::Jpeg, "jpg"),
            (ExportFormat::Png, "png"),
            (ExportFormat::Tiff, "tiff"),
            (ExportFormat::Webp, "webp"),
        ] {
            let named = name_for(Path::new("/photos/shot.nef"), &unaffixed(chosen(format)));

            assert_eq!((named.format, named.extension.as_str()), (format, extension), "{format:?}");
        }
    }

    #[test]
    fn the_affixes_go_around_the_name_and_the_file_goes_into_the_sources_directory() {
        let naming = naming("new-", "-opai", chosen(ExportFormat::Webp));

        assert_eq!(name_for(Path::new("/photos/beach.jpg"), &naming).name, "new-beach-opai.webp");
        assert_eq!(
            destination_for(Path::new("/photos/beach.jpg"), &naming, None).0,
            Path::new("/photos/new-beach-opai.webp")
        );
    }

    #[test]
    fn only_the_last_extension_is_cut_from_a_name_with_dots_in_it() {
        let (destination, _) =
            destination_for(Path::new("/photos/2026.09.24 beach.jpg"), &unaffixed(FormatChoice::Preserve), None);

        assert_eq!(destination, Path::new("/photos/2026.09.24 beach.jpg"));
    }

    #[test]
    fn a_browsed_folder_is_written_into_instead() {
        let (destination, _) = destination_for(
            Path::new("/photos/beach.jpg"),
            &unaffixed(chosen(ExportFormat::Png)),
            Some(Path::new("/exports")),
        );

        assert_eq!(destination, Path::new("/exports/beach.png"));
    }

    #[test]
    fn a_file_at_the_root_is_written_into_the_root() {
        let (destination, _) = destination_for(Path::new("/beach.jpg"), &unaffixed(chosen(ExportFormat::Png)), None);

        assert_eq!(destination, Path::new("/beach.png"));
    }

    #[test]
    fn a_bare_name_with_no_folder_gives_a_bare_name() {
        let (destination, _) = destination_for(Path::new("beach.jpg"), &unaffixed(chosen(ExportFormat::Png)), None);

        assert_eq!(destination, Path::new("beach.png"));
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_path_is_kept_in_backslashes() {
        let png = unaffixed(chosen(ExportFormat::Png));

        assert_eq!(
            destination_for(Path::new(r"C:\Users\someone\beach.JPG"), &png, None).0.as_os_str(),
            r"C:\Users\someone\beach.png"
        );
        assert_eq!(
            destination_for(Path::new(r"C:\Users\someone\beach.JPG"), &png, Some(Path::new(r"D:\Exports\")))
                .0
                .as_os_str(),
            r"D:\Exports\beach.png"
        );
        assert_eq!(
            destination_for(Path::new(r"C:\Users\someone\beach.JPG"), &png, Some(Path::new(r"D:\Exports")))
                .0
                .as_os_str(),
            r"D:\Exports\beach.png"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_utf8_is_kept_byte_for_byte() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let source = Path::new(OsStr::from_bytes(b"/photos/\xffbeach.jpg"));
        let (destination, named) = destination_for(source, &naming("new-", "", chosen(ExportFormat::Png)), None);

        assert_eq!(named.name.as_bytes(), b"new-\xffbeach.png");
        assert_eq!(destination.as_os_str().as_bytes(), b"/photos/new-\xffbeach.png");
    }
}
