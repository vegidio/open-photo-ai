//! Recognising camera RAW files by their extension, and asking the backend which of them it can decode.
//!
//! *Is this a RAW file?* is answered locally, from [`RAW_EXTENSIONS`]; *can it be decoded?* is answered by
//! [`decodable`], which asks `rust-sak`. [`route`] is how the rest of the module asks both at once, and is the only
//! way in. [`is_raw`] is the first question asked on its own, for a caller choosing between [`fn@super::probe`] and
//! [`super::probe_raw`].

// The extension is checked before any content-based routing, and that ordering is the point. Most RAW formats —
// every one here except CR3, RAF, RW2 and ORF, which carry container signatures of their own and which `decode_bytes`
// would develop unaided — are TIFF containers and carry plain TIFF magic (`II*\0` or `MM\0*`), so routing on content
// would hand a NEF, a CR2 or an ARW to the TIFF decoder, which would succeed and return an embedded JPEG preview or
// corrupt pixels. A wrong image reported as a success is worse than a refusal. A DNG names itself in its `DNGVersion`
// tag, but its header is a TIFF header like the rest, so that is what a content sniffer sees first.
//
// Recognition deliberately covers more formats than can be decoded: a RAW format with no decoder is still a RAW
// format and must never reach the TIFF decoder.

use std::path::Path;

use rust_sak::image::RawFormat;

use super::error::ImageIoError;

// The Go application's `SupportedRawExtensions` list, kept in its order and grouping so the two can be compared by
// eye, plus `ari`, `crm` and `ori` — three formats the backend decodes that the Go list does not name, and which
// without this entry would fall through to content routing and be misdecoded as TIFF.
/// The camera RAW extensions the application recognises, lowercase and without the leading dot.
///
/// Recognition is a superset of decoding: 47 extensions are recognised here and [`decodable`] answers `true` for 29
/// of them. The remaining 18 are refused by name.
pub(crate) const RAW_EXTENSIONS: &[&str] = &[
    "crw", "cr2", "cr3", "crm", // Canon (+ Cinema RAW Light)
    "nef", "nrw", // Nikon
    "arw", "srf", "sr2", // Sony
    "raf", // Fujifilm
    "orf", "ori", // Olympus
    "rw2", "raw", "rwl", // Panasonic/Leica
    "pef", "ptx", "dng", // Pentax/Ricoh (+ Adobe/generic DNG)
    "srw", // Samsung
    "x3f", // Sigma
    "3fr", "fff", // Hasselblad
    "iiq", "cap", "eip", // Phase One
    "dcr", "kdc", "k25", "dcs", "dc2", // Kodak
    "mos", // Leaf
    "mef", // Mamiya
    "mrw", "mdc", // Minolta
    "erf", // Epson
    "bay", // Casio
    "pxn", // Logitech
    "gpr", // GoPro
    "ari", // ARRI
    "bmq", "cs1", "cine", "ia", "kc2", "qtk", "rdc", "sti", // misc
];

/// The extension of `path`, lowercased and without the leading dot, if it has one.
fn extension_of(path: &Path) -> Option<String> {
    // Lowercased because a camera writes `.NEF` as readily as `.nef`, and on a case-sensitive filesystem the two are
    // different strings for the same format.
    path.extension().map(|extension| extension.to_string_lossy().to_lowercase())
}

/// Whether `path` names a camera RAW file, decodable or not.
///
/// A caller asks it to choose between [`probe`](fn@super::probe) and [`probe_raw`](super::probe_raw), which describe
/// the two families and cost wildly different amounts. Loading needs no such choice: [`load`](fn@super::load) asks
/// this itself.
///
/// The answer ignores the case the camera wrote, and does not depend on whether the format can be decoded.
///
/// ```
/// assert!(opai::image::is_raw("/pictures/DSC_0001.NEF"));
/// assert!(!opai::image::is_raw("/pictures/scan.tiff"));
/// ```
pub fn is_raw(path: impl AsRef<Path>) -> bool {
    // A linear scan rather than a set: the list is short, the comparison is on a handful of bytes, and building a set
    // per process to answer a question asked once per file would cost more than it saves.
    extension_of(path.as_ref()).is_some_and(|extension| RAW_EXTENSIONS.contains(&extension.as_str()))
}

/// Whether `extension` names a RAW format the backend can decode.
///
/// `extension` is expected lowercase and without its dot, as [`extension_of`] returns it, though
/// `RawFormat::from_extension` is itself case-insensitive.
fn decodable(extension: &str) -> bool {
    // Asked of `rust-sak` at the point of use rather than answered from a constant here. A second list would be a copy
    // of someone else's truth: a backend that gained a format would go on being refused by a stale list, which is the
    // failure mode a hardcoded copy always has.
    RawFormat::from_extension(extension).is_some()
}

/// The recognised RAW extensions this backend can actually decode, in [`RAW_EXTENSIONS`]' own order:
/// [`super::input_extensions`]' RAW half.
pub(super) fn decodable_extensions() -> impl Iterator<Item = &'static str> {
    // Filtered through `decodable` rather than kept as a second constant, for the reason that function gives.
    RAW_EXTENSIONS.iter().copied().filter(|extension| decodable(extension))
}

/// Which decoder `path` routes to, decided from its name alone and before the file is opened.
///
/// `Ok(Some(extension))` is a RAW file this backend develops, and the extension is the one
/// [`fn@super::load`] routes on. `Ok(None)` is everything else, which goes on to content routing.
/// The error is the one case that is settled here: a recognised RAW format with no decoder is refused by
/// name, so a `.gpr` on a drive that is no longer mounted names the format that is missing rather than an
/// I/O failure.
pub(crate) fn route(path: &Path) -> Result<Option<String>, ImageIoError> {
    // One function rather than the three steps written out at each entry point, because the ordering is the contract
    // `load` and `probe_raw` both promise — that the two agree about a path on an unreachable drive — and two copies of
    // it would be kept in step only by a comment in each saying so.
    let Some(extension) = extension_of(path).filter(|extension| RAW_EXTENSIONS.contains(&extension.as_str())) else {
        return Ok(None);
    };

    if !decodable(&extension) {
        return Err(ImageIoError::UnsupportedRaw { path: path.to_path_buf(), extension });
    }

    Ok(Some(extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The 29 extensions the backend decodes, spelled out rather than derived, so that a backend which quietly
    /// stopped covering one of them fails here instead of turning into a refusal a user sees.
    const DECODABLE: &[&str] = &[
        "ari", "arw", "crw", "cr2", "cr3", "crm", "dcr", "dcs", "dng", "erf", "fff", "iiq", "kdc", "mef", "mos", "mrw",
        "nef", "nrw", "orf", "ori", "pef", "qtk", "raf", "raw", "rw2", "rwl", "srw", "3fr", "x3f",
    ];

    /// The 18 recognised formats no pure-Rust backend decodes, and the list a refusal is allowed to name.
    const REFUSED: &[&str] = &[
        "bay", "bmq", "cap", "cine", "cs1", "dc2", "eip", "gpr", "ia", "k25", "kc2", "mdc", "ptx", "pxn", "rdc", "sr2",
        "srf", "sti",
    ];

    #[test]
    fn every_published_raw_extension_is_recognised() {
        // The count is asserted because the list is the routing table: an extension dropped from it is not a
        // refusal, it is a RAW file handed to the TIFF decoder, which is the one outcome this module exists to stop.
        assert_eq!(RAW_EXTENSIONS.len(), 47, "the recognised list changed size");

        for extension in RAW_EXTENSIONS {
            let path = PathBuf::from(format!("/pictures/DSC_0001.{extension}"));
            assert!(is_raw(&path), "{extension} is on the list and was not recognised");
        }
    }

    #[test]
    fn the_three_formats_the_go_list_omits_are_recognised() {
        // ARRI, Canon Cinema RAW Light and Olympus's second extension; see `RAW_EXTENSIONS` for why they are listed.
        for extension in ["ari", "crm", "ori"] {
            let path = PathBuf::from(format!("/pictures/DSC_0001.{extension}"));
            assert!(is_raw(&path), "{extension} was not recognised");
            assert!(decodable(extension), "{extension} was added because the backend decodes it");
        }
    }

    #[test]
    fn the_backend_decodes_the_twenty_nine_formats_and_refuses_the_eighteen() {
        assert_eq!(DECODABLE.len(), 29);
        assert_eq!(REFUSED.len(), 18);

        for extension in DECODABLE {
            assert!(decodable(extension), "{extension} is decodable and was reported as unsupported");
        }
        for extension in REFUSED {
            assert!(!decodable(extension), "{extension} has no decoder and was reported as decodable");
        }
    }

    #[test]
    fn decodable_and_refused_partition_the_recognised_list_exactly() {
        // Neither an extension recognised but in neither set — which would be a format whose fate depends on where
        // the code happens to ask — nor one in a set but unrecognised, which would be a decoder that can never be
        // reached because `is_raw` never routes to it.
        assert_eq!(DECODABLE.len() + REFUSED.len(), RAW_EXTENSIONS.len());

        for extension in RAW_EXTENSIONS {
            assert_ne!(
                DECODABLE.contains(extension),
                REFUSED.contains(extension),
                "{extension} is in both lists or in neither"
            );
            assert_eq!(
                decodable(extension),
                DECODABLE.contains(extension),
                "{extension} and the backend disagree about whether it can be decoded"
            );
        }

        for extension in DECODABLE.iter().chain(REFUSED) {
            assert!(RAW_EXTENSIONS.contains(extension), "{extension} is claimed by a set and recognised by nothing");
        }
    }

    #[test]
    fn a_tiff_is_neither_decodable_as_raw_nor_recognised_as_it() {
        // `tif` and `tiff` must be in neither set. Almost every RAW format *is* a TIFF container, so claiming the
        // extension for RAW would route a scanned TIFF to the RAW decoder and break a format that already works.
        for extension in ["tif", "tiff"] {
            assert!(!decodable(extension), "{extension} was claimed for RAW");
            assert!(!RAW_EXTENSIONS.contains(&extension), "{extension} was claimed for RAW");
        }
    }

    #[test]
    fn recognition_ignores_the_case_the_camera_wrote() {
        for path in ["/pictures/DSC_0001.NEF", "/pictures/DSC_0001.Nef", "/pictures/DSC_0001.nef"] {
            assert!(is_raw(Path::new(path)), "{path} was not recognised as RAW");
        }
    }

    #[test]
    fn a_tiff_is_not_raw_even_though_the_raw_formats_are_tiffs() {
        // The whole point of the check: RAW files carry TIFF magic, so the extension is the only thing that tells
        // them apart — which means a genuine TIFF must survive it and go on to be decoded normally.
        assert!(!is_raw(Path::new("/pictures/scan.tif")));
        assert!(!is_raw(Path::new("/pictures/scan.tiff")));
        assert!(!is_raw(Path::new("/pictures/scan.TIFF")));
    }

    #[test]
    fn the_other_supported_formats_are_not_raw() {
        for extension in ["avif", "bmp", "gif", "heic", "heif", "jpeg", "jpg", "png", "webp"] {
            let path = PathBuf::from(format!("/pictures/holiday.{extension}"));
            assert!(!is_raw(&path), "{extension} is a standard format and was taken for RAW");
        }
    }

    #[test]
    fn a_file_with_no_extension_is_not_raw() {
        // It is not refused here: with no extension there is nothing to route on, so it falls through to content
        // detection, which is where a correctly formed image with no name to speak of is decoded anyway.
        assert!(!is_raw(Path::new("/pictures/DSC_0001")));
        assert_eq!(extension_of(Path::new("/pictures/DSC_0001")), None);
    }

    #[test]
    fn the_extension_is_reported_lowercased_and_without_its_dot() {
        // This is the string a refusal names, so a user sees `nef` rather than `.NEF`.
        assert_eq!(extension_of(Path::new("/pictures/DSC_0001.NEF")).as_deref(), Some("nef"));
    }
}
