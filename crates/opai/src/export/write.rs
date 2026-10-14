//! Writing an exported photograph: the destination claimed, the picture saved into it, and the claim kept — the three
//! steps every front end takes, in the one order that leaves nothing behind when a step fails.

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};

use super::claim::claim;
use crate::image::{self, ImageFormat, ImageIoError, Picture, recommended_encode_options};
use crate::task::{Cancelled, spawn_blocking};

/// How [`write`] writes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WriteOptions {
    /// The encoder quality, or `None` for the format's default — see
    /// [`recommended_encode_options`](crate::image::recommended_encode_options).
    pub quality: Option<f64>,
    /// Replace a file already at the destination once the new one is complete, rather than numbering the result
    /// `_1`, `_2`, … beside it.
    pub overwrite: bool,
    /// Create the destination's directory first where it is missing, rather than failing.
    pub create_parent: bool,
}

/// What [`write`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// Where it went: the destination, or the numbered name beside it.
    pub path: PathBuf,
    /// How many bytes it wrote.
    pub bytes: u64,
}

/// Why [`write`] wrote nothing.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// The destination's directory could not be created, or no name could be held or kept. The filesystem's error
    /// names no path, so the destination asked for is named here.
    #[error("cannot write {}: {source}", destination.display())]
    Unwritable {
        /// The destination asked for.
        destination: PathBuf,
        /// What the filesystem refused.
        #[source]
        source: io::Error,
    },
    /// The picture could not be encoded or saved. The library's message names the file.
    #[error(transparent)]
    Save(#[from] ImageIoError),
}

impl From<Cancelled> for WriteError {
    fn from(cancelled: Cancelled) -> Self {
        Self::Save(cancelled.into())
    }
}

/// Writes `picture` to `destination` as `format`, at the library's recommended settings for it and `options`' quality.
///
/// Without [`overwrite`](WriteOptions::overwrite) a file already there is kept and the result takes the first free
/// `_1` … `_999` name beside it; with it, the file is replaced once the new one is complete. **A write that does not
/// finish leaves nothing behind**: the name claimed is given back, and a file being replaced is untouched.
///
/// The filesystem work is done off the async runtime. Every failure is recorded once: a failed save where it happened,
/// and a destination that could not be claimed or kept here.
///
/// # Errors
///
/// [`WriteError`].
pub async fn write(
    picture: &Picture,
    destination: &Path,
    format: ImageFormat,
    options: WriteOptions,
) -> Result<Written, WriteError> {
    let pixels = picture.shared_pixels();
    let encode = recommended_encode_options(format, options.quality);

    write_with(destination, options, |path| image::save(pixels, path, format, Some(encode))).await
}

/// [`write`] with the encoding left to `save`, which is handed the path to write into and answers the bytes written.
async fn write_with<F>(
    destination: &Path,
    options: WriteOptions,
    save: impl FnOnce(PathBuf) -> F,
) -> Result<Written, WriteError>
where
    F: Future<Output = Result<u64, ImageIoError>>,
{
    let unwritable = |source: io::Error| {
        tracing::warn!(op = "export", path = %destination.display(), %source, "an export could not be written");

        WriteError::Unwritable { destination: destination.to_path_buf(), source }
    };

    let requested = destination.to_path_buf();
    let (create_parent, overwrite) = (options.create_parent, options.overwrite);
    // Up to a thousand exclusive creates, on whatever drive the user chose.
    let claim = spawn_blocking::<_, WriteError, _>(move || {
        if create_parent && let Some(parent) = requested.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }

        claim(&requested, overwrite)
    })
    .await?
    .map_err(unwritable)?;

    // A failure drops the claim, which gives its name back.
    let bytes = save(claim.path().to_path_buf()).await?;

    // Overwriting, this is where the destination is replaced, and not before: a failed save above left it untouched.
    let path = spawn_blocking::<_, WriteError, _>(move || claim.commit()).await?.map_err(unwritable)?;

    Ok(Written { path, bytes })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn picture() -> Picture {
        Picture::new("source.png", Arc::new(::image::DynamicImage::new_rgb8(4, 3)), "0123456789abcdef")
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("listed")
            .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn the_picture_is_written_and_its_bytes_answered() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("out.png");

        let written = write(&picture(), &out, ImageFormat::Png, WriteOptions::default()).await.expect("written");

        assert_eq!(written.path, out);
        assert_eq!(written.bytes, std::fs::metadata(&out).expect("on disk").len());
        assert!(image::probe_blocking(&out).is_ok(), "the file is an image");
    }

    #[tokio::test]
    async fn an_existing_file_is_kept_and_the_result_numbered() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("out.png");
        std::fs::write(&out, "earlier").expect("written");

        let written = write(&picture(), &out, ImageFormat::Png, WriteOptions::default()).await.expect("written");

        assert_eq!(written.path, dir.path().join("out_1.png"));
        assert_eq!(std::fs::read(&out).expect("kept"), b"earlier");
    }

    #[tokio::test]
    async fn overwrite_replaces_the_file_and_leaves_nothing_beside_it() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("out.png");
        std::fs::write(&out, "earlier").expect("written");
        let options = WriteOptions { overwrite: true, ..Default::default() };

        let written = write(&picture(), &out, ImageFormat::Png, options).await.expect("written");

        assert_eq!(written.path, out);
        assert!(image::probe_blocking(&out).is_ok(), "the file was replaced by an image");
        assert_eq!(names(dir.path()), ["out.png"]);
    }

    #[tokio::test]
    async fn a_missing_directory_is_unwritable_naming_the_destination() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("missing/out.png");

        let error = write(&picture(), &out, ImageFormat::Png, WriteOptions::default())
            .await
            .expect_err("no directory");

        assert!(matches!(&error, WriteError::Unwritable { destination, .. } if *destination == out), "{error:?}");
        assert!(error.to_string().starts_with(&format!("cannot write {}: ", out.display())), "{error}");
        assert!(!dir.path().join("missing").exists());
    }

    #[tokio::test]
    async fn create_parent_creates_the_missing_directory() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("2024/06/out.png");
        let options = WriteOptions { create_parent: true, ..Default::default() };

        let written = write(&picture(), &out, ImageFormat::Png, options).await.expect("written");

        assert_eq!(written.path, out);
    }

    #[tokio::test]
    async fn a_failed_save_leaves_nothing_behind() {
        for overwrite in [false, true] {
            let dir = tempfile::tempdir().expect("a temporary directory");
            let out = dir.path().join("out.png");
            std::fs::write(&out, "earlier").expect("written");
            let options = WriteOptions { overwrite, ..Default::default() };

            let error = write_with(&out, options, async |path: PathBuf| {
                std::fs::write(&path, "half").expect("the claim is writable");
                Err(ImageIoError::Write { path, source: io::Error::other("the encoder failed") })
            })
            .await
            .expect_err("the save failed");

            assert!(matches!(error, WriteError::Save(_)), "{error:?}");
            assert_eq!(std::fs::read(&out).expect("kept"), b"earlier", "overwrite: {overwrite}");
            assert_eq!(names(dir.path()), ["out.png"], "overwrite: {overwrite}");
        }
    }
}
