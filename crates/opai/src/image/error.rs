//! The error type image IO reports.

use std::path::{Path, PathBuf};

use rust_sak::image::{ImageError, ImageFormat};
use thiserror::Error;

use crate::task::Cancelled;
use crate::telemetry::unit::{self, Outcome, Unit};
use std::time::Instant;
use tracing::Span;

// Separate from `InitError` rather than a set of variants on it: nothing here is an installation failure, and a front
// end that reports "the application could not start" over a file the user picked would be telling them the wrong
// thing. The two share only the crate's internal cancellation marker, which each turns into a case of its own.
//
// Every variant about a file carries its path because `std::io::Error` says `permission denied (os error 13)` and
// nothing about which file, and `ImageError` is the same — it names a codec and not a photograph. A front end listing
// a directory reports several of these at once, so each has to say which file it is about.
/// Everything that can stop an image being read, written or described.
///
/// Every variant that is about a file carries the path it was working on.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ImageIoError {
    /// The file could not be read: it does not exist, it is a directory, or the operating system refused it.
    #[error("failed to read {}: {source}", path.display())]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: std::io::Error,
    },

    /// The destination could not be opened for writing: its directory does not exist, or the path is not writable.
    #[error("failed to write {}: {source}", path.display())]
    Write {
        /// The destination that could not be written.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: std::io::Error,
    },

    // Told apart from `Read` because the two mean different things to whoever sees them: one is a file the
    // application could not get at, the other a file it got at and found was not a photograph.
    /// The file was read in full, and its content is not an image in any supported format.
    #[error("failed to decode {}: {source}", path.display())]
    Decode {
        /// The file whose content could not be decoded.
        path: PathBuf,
        /// What `rust-sak` reported — which codec refused it, or that no magic signature matched.
        #[source]
        source: ImageError,
    },

    /// A codec refused work that is not decoding a file's content: encoding, or parsing a header while probing.
    ///
    /// It also carries the refusal of encoder settings that target a format other than the one being written, which
    /// `rust-sak` reports as [`ImageError::FormatMismatch`] rather than attempting the encode.
    #[error("the codec refused {}: {source}", path.display())]
    Codec {
        /// The file the codec was working on — the destination when encoding, the source when probing.
        path: PathBuf,
        /// What `rust-sak` reported.
        #[source]
        source: ImageError,
    },

    // Told apart from `Codec`, which is the same refusal with a destination behind it: a caller that failed to export
    // can name the file it failed to write, and a caller that failed to build a preview cannot.
    /// A codec refused to encode pixels that were never on their way to a file.
    ///
    /// [`encode`](fn@super::encode)'s failure, and the only one here with no path: the picture is in memory and the
    /// bytes were going into a buffer, so there is no file this could be about.
    ///
    /// It carries the refusal of encoder settings that target a format other than the one being encoded, which
    /// `rust-sak` reports as [`ImageError::FormatMismatch`].
    #[error("the codec refused to encode the image: {source}")]
    Encode {
        /// What `rust-sak` reported.
        #[source]
        source: ImageError,
    },

    /// A camera RAW file in one of the recognised formats that **has no decoder**, refused rather than decoded as
    /// the TIFF it resembles: one of the 18 the [module documentation](super) lists.
    ///
    /// It is the format that is refused and never the file, so this says nothing about the content: a decodable RAW
    /// that turns out to be corrupt is an [`ImageIoError::Decode`], the same as a corrupt JPEG. That is also why
    /// the refusal happens before the file is opened — with nothing read, there is nothing to be wrong about.
    #[error("{} is a camera RAW file in a format with no decoder: {extension} images cannot be opened", path.display())]
    UnsupportedRaw {
        /// The file that was refused.
        path: PathBuf,
        /// Its extension, lowercased and without the leading dot, so a caller can say which format it was.
        extension: String,
    },

    /// The asynchronous runtime shut down before the read or write handed to a blocking thread could run.
    ///
    /// Distinct from every failure above because nothing failed: no file is missing, unreadable or corrupt, and the
    /// right response is to stop quietly rather than to tell a user their photograph could not be opened.
    #[error("cancelled: the runtime shut down before the image could be read or written")]
    Cancelled,
}

impl From<Cancelled> for ImageIoError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

impl ImageIoError {
    /// The error's stable name, which a failure is counted by: one snake-case name per variant, never its text.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Read { .. } => "read",
            Self::Write { .. } => "write",
            Self::Decode { .. } => "decode",
            Self::Codec { .. } => "codec",
            Self::Encode { .. } => "encode",
            Self::UnsupportedRaw { .. } => "unsupported_raw",
            Self::Cancelled => "cancelled",
        }
    }

    /// Builds the [`ImageIoError::Read`] for a failure against `path`, curried so a site reads
    /// `.map_err(ImageIoError::read(&path))`.
    pub(crate) fn read(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        // Curried for the same reason `InitError::io` is: there is no `#[from]` to reach for, because a bare `?` is
        // exactly how the path gets lost.
        |source| Self::Read { path: path.into(), source }
    }

    /// Builds the [`ImageIoError::Write`] for a failure against `path`.
    pub(crate) fn write(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        |source| Self::Write { path: path.into(), source }
    }

    /// Builds the [`ImageIoError::Decode`] for a failure against `path`.
    pub(crate) fn decode(path: impl Into<PathBuf>) -> impl FnOnce(ImageError) -> Self {
        |source| Self::Decode { path: path.into(), source }
    }

    /// Builds the [`ImageIoError::Codec`] for a failure against `path`.
    pub(crate) fn codec(path: impl Into<PathBuf>) -> impl FnOnce(ImageError) -> Self {
        |source| Self::Codec { path: path.into(), source }
    }
}

// The three records every public blocking operation writes on failure, once, at its own boundary: a constructor above
// does not know which operation it is serving, and fires for errors that are then handled internally. The async twins
// call the blocking forms, so they are covered by construction; the one error only they add, `Cancelled`, is a
// shutdown and is correctly not recorded. Success writes nothing — describing a folder is one call per file.

/// Runs one public operation's `work` inside `span`, as `unit`, and ends the span the way it went.
///
/// The failure record `work` writes at its boundary is written inside the span, so it is in the operation's trace.
pub(super) fn traced<T>(
    unit: Unit,
    span: Span,
    work: impl FnOnce() -> Result<T, ImageIoError>,
) -> Result<T, ImageIoError> {
    let started = Instant::now();
    let outcome = span.in_scope(work);
    let duration = started.elapsed();

    let ending = match &outcome {
        Ok(_) => Outcome::Finished,
        Err(ImageIoError::Cancelled) => Outcome::Stopped,
        Err(error) => Outcome::Failed { kind: error.kind(), error },
    };
    unit::ended(unit, &span, duration, ending);

    outcome
}

/// Records a failure to read `path` for `op` — a load, a probe or an identity.
pub(super) fn unreadable<'a>(op: &'static str, path: &'a Path) -> impl FnOnce(&ImageIoError) + 'a {
    move |error| tracing::warn!(op, path = %path.display(), %error, "an image could not be read")
}

/// Records a failure to write `path` as `format`.
pub(super) fn unwritable(path: &Path, format: ImageFormat) -> impl FnOnce(&ImageIoError) + '_ {
    move |error| tracing::warn!(op = "save", path = %path.display(), ?format, %error, "an image could not be written")
}

/// Records a failure to encode as `format`, which has no file to name.
pub(super) fn unencodable(format: ImageFormat) -> impl FnOnce(&ImageIoError) {
    move |error| tracing::warn!(op = "encode", ?format, %error, "an image could not be encoded")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    /// The file every error below is built against. A NEF: a RAW format the backend decodes, so a failure against
    /// it is a failure about the file rather than about the format.
    const PHOTO: &str = "/pictures/DSC_0001.nef";

    /// A RAW format with no decoder, which is the only thing [`ImageIoError::UnsupportedRaw`] describes. A NEF
    /// would be wrong here in a way the compiler cannot catch: it is decoded, so it never reaches this variant.
    const UNDECODABLE: &str = "/pictures/clip.gpr";

    #[test]
    fn a_raw_file_a_corrupt_file_and_an_unreadable_one_are_told_apart() {
        // Three different things to tell a user: this RAW format has no decoder, this file is not an image, and
        // this file could not be opened. Flattened into one variant a front end could only say "it did not work".
        let raw = ImageIoError::UnsupportedRaw { path: PathBuf::from(UNDECODABLE), extension: "gpr".to_string() };
        let decode = ImageIoError::Decode { path: PathBuf::from(PHOTO), source: ImageError::UnrecognizedFormat };
        let read = ImageIoError::Read {
            path: PathBuf::from(PHOTO),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        };

        assert!(matches!(raw, ImageIoError::UnsupportedRaw { .. }));
        assert!(matches!(decode, ImageIoError::Decode { .. }));
        assert!(matches!(read, ImageIoError::Read { .. }));

        // And they do not merely differ by variant: what a user is shown differs too.
        assert_ne!(raw.to_string(), decode.to_string());
        assert_ne!(decode.to_string(), read.to_string());
        assert_ne!(raw.to_string(), read.to_string());
    }

    #[test]
    fn a_refused_raw_file_names_the_extension_it_was_refused_for() {
        // The extension is what says *which* format is missing, and a user with a folder of `.gpr` files and a
        // folder of `.cine` ones needs to know which of them this application cannot open.
        let error = ImageIoError::UnsupportedRaw { path: PathBuf::from(UNDECODABLE), extension: "gpr".to_string() };

        let message = error.to_string();
        assert!(message.contains("gpr"), "the message did not name the extension: {message}");
        assert!(message.contains(UNDECODABLE), "the message did not name the file: {message}");
        // And it says the *format* has no decoder rather than that RAW cannot be opened: 29 of the 47 recognised
        // extensions decode.
        assert!(message.contains("no decoder"), "the message does not say what is missing: {message}");
    }

    #[test]
    fn every_failure_against_a_file_names_that_file() {
        let errors = [
            ImageIoError::read(PHOTO)(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            ImageIoError::write(PHOTO)(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            ImageIoError::decode(PHOTO)(ImageError::UnrecognizedFormat),
            ImageIoError::codec(PHOTO)(ImageError::UnknownExtension),
        ];

        for error in &errors {
            let message = error.to_string();
            assert!(message.contains(PHOTO), "the message did not name the file: {message}");
            // And the chain still reaches what actually failed, so a front end that walks it sees the codec's or the
            // operating system's own words.
            assert!(error.source().is_some(), "the underlying error is not reachable from {error:?}");
        }
    }

    #[test]
    fn every_failure_is_counted_by_a_pinned_name() {
        let denied = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let named = [
            (ImageIoError::read(PHOTO)(denied()), "read"),
            (ImageIoError::write(PHOTO)(denied()), "write"),
            (ImageIoError::decode(PHOTO)(ImageError::UnrecognizedFormat), "decode"),
            (ImageIoError::codec(PHOTO)(ImageError::UnknownExtension), "codec"),
            (ImageIoError::Encode { source: ImageError::UnknownExtension }, "encode"),
            (
                ImageIoError::UnsupportedRaw { path: PathBuf::from(UNDECODABLE), extension: "gpr".to_string() },
                "unsupported_raw",
            ),
            (ImageIoError::Cancelled, "cancelled"),
        ];
        for (error, kind) in named {
            assert_eq!(error.kind(), kind, "{error:?}");
        }
    }

    #[test]
    fn a_cancelled_operation_is_told_apart_from_a_file_that_could_not_be_read() {
        let cancelled = ImageIoError::from(Cancelled);
        let read = ImageIoError::read(PHOTO)(std::io::Error::from(std::io::ErrorKind::NotFound));

        assert!(matches!(cancelled, ImageIoError::Cancelled));
        assert!(matches!(read, ImageIoError::Read { .. }));
        assert_ne!(cancelled.to_string(), read.to_string());
        // Nothing failed underneath it, so there is nothing in the chain to walk to.
        assert!(cancelled.source().is_none(), "cancellation invented an underlying failure");
    }
}
