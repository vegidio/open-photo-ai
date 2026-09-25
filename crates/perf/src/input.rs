//! The image every model is measured against.

// The sample is the workspace root's `fixtures/test.dat` — the one the live inference checks enhance. It is
// **embedded** at compile time rather than read from `CARGO_MANIFEST_DIR` at run time, which is what
// `inference/live.rs` does and which is right for a test and wrong for a binary somebody copies out of
// `target/release`. It is the same 640x640 JPEG the Go application's `cmd/perf` carries, so a number taken here can be
// compared with one taken there.
//
// It is materialized through a temp file because `opai::image::load` takes a path — and loading through it, rather
// than decoding the bytes some other way, is also what checks that a file this produced is a file the pipeline
// accepts.

use std::io::Write;
use std::path::{Path, PathBuf};

use opai::{ImageIoError, Picture, image};

// The workspace root's copy, not one of this crate's own: `fixtures/` is where a file more than one crate's tests
// need lives, and a second copy here would be a second thing to keep in step with the reference. `.dat` is what the
// Go application named it, kept so the two projects measure the same bytes.
/// The sample photograph, carried in the binary.
///
/// A JPEG despite the extension. Nothing routes on the name — [`opai::image::load`] reads the format from the
/// content's magic bytes.
const SAMPLE: &[u8] = include_bytes!("../../../fixtures/test.dat");

/// The image a sweep runs against, and which one it is.
#[derive(Debug, Clone)]
pub struct Input {
    /// The decoded image, shared by every run of every model.
    pub picture: Picture,

    /// Where the pixels came from, for the header.
    pub source: Source,
}

// Reported in the header so two pasted tables cannot be confused: the sample is 0.41 megapixels and the photographs
// this application exists for are twenty times that, which is a larger difference than any the table shows.
/// Which image was measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The sample carried in the binary.
    Embedded,
    /// A photograph the operator named.
    File(PathBuf),
}

impl Input {
    /// The image's width and height.
    pub fn dimensions(&self) -> (u32, u32) {
        self.picture.dimensions()
    }

    /// The input's megapixels, which is what the throughput figure is computed over.
    pub fn megapixels(&self) -> f64 {
        let (width, height) = self.dimensions();

        f64::from(width) * f64::from(height) / 1_000_000.0
    }

    /// How the header names the image.
    pub fn label(&self) -> String {
        match &self.source {
            Source::Embedded => "embedded sample".to_string(),
            Source::File(path) => path.display().to_string(),
        }
    }
}

/// Why the input could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    /// No temporary file could be created for the embedded sample.
    #[error("could not materialize the embedded sample: {0}")]
    Materialize(#[from] rust_sak::fs::FsError),

    /// The embedded sample could not be written to a temporary file.
    #[error("could not write the embedded sample to {path}: {source}")]
    Write {
        /// The temporary file being written.
        path: PathBuf,
        /// What the write reported.
        #[source]
        source: std::io::Error,
    },

    /// The file could not be read or decoded.
    #[error("could not load the image {path}: {source}")]
    Load {
        // The embedded sample's case is one nobody should ever see, and it would say nothing at all if the path were
        // omitted.
        /// The file that was named. The embedded sample's temporary path where it is the one that failed.
        path: PathBuf,
        // Boxed because it is large beside the two other variants, and this error sits in the `Err` of every load —
        // including the one that always succeeds.
        /// What the loader reported.
        #[source]
        source: Box<ImageIoError>,
    },
}

/// Loads the image a sweep measures: the photograph `path` names, or the embedded sample where it names none.
///
/// # Errors
///
/// [`InputError`], naming the file in every case.
pub async fn load(path: Option<&Path>) -> Result<Input, InputError> {
    match path {
        Some(path) => {
            let picture = decode(path).await?;

            Ok(Input { picture, source: Source::File(path.to_path_buf()) })
        }
        None => {
            let picture = embedded().await?;

            Ok(Input { picture, source: Source::Embedded })
        }
    }
}

/// The embedded sample, through a temporary file that is deleted before this returns.
async fn embedded() -> Result<Picture, InputError> {
    // Dropping `rust-sak`'s handle deletes the file, including on an early `?`. It is dropped as soon as the decode is
    // done rather than held for the sweep: the pixels are in memory from that point on, nothing reads the file again,
    // and a temp file living for the length of a multi-minute sweep is a file left behind by anything that kills the
    // process.
    let mut file = rust_sak::fs::mk_temp_file("open-photo-ai-perf-")?;
    let path = file.path().to_path_buf();

    file.write_all(SAMPLE).map_err(|source| InputError::Write { path: path.clone(), source })?;
    file.flush().map_err(|source| InputError::Write { path: path.clone(), source })?;

    decode(&path).await
}

/// Reads and decodes one file, naming it in whatever it reports.
async fn decode(path: &Path) -> Result<Picture, InputError> {
    image::load(path)
        .await
        .map_err(|source| InputError::Load { path: path.to_path_buf(), source: Box::new(source) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_embedded_sample_decodes_to_the_640x640_the_reference_measures() {
        let input = load(None).await.expect("the embedded sample decodes");

        assert_eq!(input.dimensions(), (640, 640));
        assert_eq!(input.source, Source::Embedded);
        assert_eq!(input.label(), "embedded sample");
    }

    #[tokio::test]
    async fn the_sample_is_reported_at_the_megapixels_the_throughput_figure_is_over() {
        let input = load(None).await.expect("the embedded sample decodes");

        // 640 x 640 = 409,600 pixels. Small enough that a header stating it is what stops somebody reading this
        // binary's MPix/s as a figure for the photographs the application is actually for.
        assert!((input.megapixels() - 0.4096).abs() < f64::EPSILON, "{}", input.megapixels());
    }

    #[tokio::test]
    async fn the_temporary_file_the_sample_was_materialized_through_is_gone_afterwards() {
        let input = load(None).await.expect("the embedded sample decodes");

        // The pixels are in memory; the file the loader read them from is not on disk any more.
        assert!(!input.picture.path().exists(), "{}", input.picture.path().display());
    }

    #[tokio::test]
    async fn a_file_that_does_not_decode_fails_naming_it() {
        let directory = rust_sak::fs::mk_temp_dir("open-photo-ai-perf-test-").expect("a temporary directory");
        let path = directory.path().join("not-an-image.jpg");
        std::fs::write(&path, b"this is not a photograph").expect("the file is written");

        let error = load(Some(&path)).await.expect_err("text is not an image");

        assert!(error.to_string().contains(&path.display().to_string()), "{error}");
    }

    #[tokio::test]
    async fn a_file_that_is_not_there_fails_naming_it() {
        let path = Path::new("/no/such/photograph.jpg");

        let error = load(Some(path)).await.expect_err("a missing file is not an image");

        assert!(error.to_string().contains("/no/such/photograph.jpg"), "{error}");
    }

    #[tokio::test]
    async fn a_photograph_of_your_own_is_reported_as_itself_rather_than_as_the_sample() {
        // Written as the sample's own bytes so the test needs no second fixture: what is under test is which source
        // the header will name, not what the decoder makes of it.
        let directory = rust_sak::fs::mk_temp_dir("open-photo-ai-perf-test-").expect("a temporary directory");
        let path = directory.path().join("holiday.jpg");
        std::fs::write(&path, SAMPLE).expect("the file is written");

        let input = load(Some(&path)).await.expect("the file decodes");

        assert_eq!(input.source, Source::File(path.clone()));
        assert_eq!(input.label(), path.display().to_string());
        assert_eq!(input.dimensions(), (640, 640));
    }
}
