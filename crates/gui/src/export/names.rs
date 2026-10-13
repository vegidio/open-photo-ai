//! The export settings' naming as the window sends it, and the names it is shown before anything is written.
//!
//! The rule itself is `opai::export`'s: both the rows [`names`] feeds and the destination
//! [`export_with`](super::run::export_with) claims come from [`ExportNaming::destination_for`], so a row's name is the
//! name an export asks for.

use std::path::{Path, PathBuf};

use opai::export::{ExportFormat, FormatChoice, Named, Naming, destination_for};
use serde::{Deserialize, Serialize};

/// What decides the name and folder of each file written, as the window's export settings store it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportNaming {
    /// Written before the source's name.
    pub(crate) prefix: String,
    /// Written after the source's name, before the extension.
    pub(crate) suffix: String,
    /// The folder to write into, or absent for the directory each source is in.
    #[serde(default)]
    pub(crate) location: Option<PathBuf>,
    /// The format to write, or Preserve.
    pub(crate) format: FormatChoice,
}

impl ExportNaming {
    /// The destination `source` asks to be written to, and what it is written as.
    pub(crate) fn destination_for(&self, source: &Path) -> (PathBuf, Named) {
        let naming = Naming { prefix: self.prefix.clone(), suffix: self.suffix.clone(), format: self.format };

        destination_for(source, &naming, self.location.as_deref())
    }
}

/// What one queued file is exported as, as a row shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ExportName {
    /// The source's path, as it was asked for.
    pub(crate) path: String,
    /// The file name written, lossily converted where it is not UTF-8.
    pub(crate) name: String,
    /// The path asked for: what is written unless it is taken and the export is not overwriting.
    pub(crate) destination: String,
    /// The format written.
    pub(crate) format: ExportFormat,
    /// The extension written, without its dot.
    pub(crate) extension: String,
}

/// What each of `paths` is exported as under `naming`, in the order asked.
pub(crate) fn names(paths: Vec<String>, naming: &ExportNaming) -> Vec<ExportName> {
    paths
        .into_iter()
        .map(|path| {
            let (destination, Named { name, format, extension }) = naming.destination_for(Path::new(&path));

            ExportName {
                name: name.to_string_lossy().into_owned(),
                destination: destination.to_string_lossy().into_owned(),
                path,
                format,
                extension,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naming(format: FormatChoice) -> ExportNaming {
        ExportNaming { prefix: "new-".into(), suffix: "-opai".into(), location: None, format }
    }

    #[test]
    fn the_names_come_back_in_the_order_asked() {
        let paths = ["/photos/c.png", "/photos/a.jpg", "/photos/b.webp"].map(String::from).to_vec();

        let answered = names(paths.clone(), &naming(FormatChoice::Preserve));

        assert_eq!(answered.iter().map(|name| name.path.clone()).collect::<Vec<_>>(), paths);
        assert_eq!(
            answered.iter().map(|name| name.name.as_str()).collect::<Vec<_>>(),
            ["new-c-opai.png", "new-a-opai.jpg", "new-b-opai.webp"]
        );
    }

    #[test]
    fn preserve_over_a_raw_path_is_named_tiff() {
        let [answered] =
            <[ExportName; 1]>::try_from(names(vec!["/photos/shot.nef".into()], &naming(FormatChoice::Preserve)))
                .expect("one path asked, one answered");

        assert_eq!((answered.format, answered.extension.as_str()), (ExportFormat::Tiff, "tiff"));
        assert_eq!(answered.destination, Path::new("/photos").join("new-shot-opai.tiff").to_string_lossy());
    }

    #[test]
    fn the_naming_is_read_as_the_window_stores_it() {
        let read: ExportNaming = serde_json::from_value(serde_json::json!({
            "prefix": "",
            "suffix": "-opai",
            "location": "/exports",
            "format": "preserve",
        }))
        .expect("the window's export settings");
        assert_eq!(read.location.as_deref(), Some(Path::new("/exports")));
        assert_eq!(read.format, FormatChoice::Preserve);

        let unlocated: ExportNaming =
            serde_json::from_value(serde_json::json!({ "prefix": "", "suffix": "", "format": "jpeg" }))
                .expect("a naming with no folder chosen");
        assert_eq!(unlocated.location, None);
        assert_eq!(unlocated.format, FormatChoice::Format(ExportFormat::Jpeg));
    }

    /// The whole wire shape, pinned: the rows read exactly this.
    #[test]
    fn a_name_crosses_the_boundary_as_the_window_reads_it() {
        let naming =
            ExportNaming { location: Some("/exports".into()), ..naming(FormatChoice::Format(ExportFormat::Jpeg)) };

        let json = serde_json::to_value(names(vec!["/photos/beach.png".into()], &naming)).expect("serializes");
        let destination = Path::new("/exports").join("new-beach-opai.jpg");

        assert_eq!(
            json,
            serde_json::json!([{
                "path": "/photos/beach.png",
                "name": "new-beach-opai.jpg",
                "destination": destination.to_string_lossy(),
                "format": "jpeg",
                "extension": "jpg",
            }])
        );
    }
}
