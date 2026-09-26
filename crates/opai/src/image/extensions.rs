//! Which file extensions this crate accepts as input.

use rust_sak::image::ImageFormat;

use super::raw;

/// Every file extension [`load`](fn@super::load) can open, lowercase and without a leading dot: what a file picker
/// filters on and what a drag-and-drop check consults.
///
/// The eight standard formats' extensions — **aliases included**, so a `.jpeg` and a `.heic` are as openable as a
/// `.jpg` and a `.heif` — followed by the camera RAW extensions the backend decodes; the 18 recognised RAW formats
/// with no decoder are absent. The order is stable: the standard formats in [`ImageFormat`]'s own order, then RAW in
/// the order the routing table lists it, which is the cameras grouped by manufacturer.
///
/// ```
/// let extensions = opai::image::input_extensions();
///
/// assert!(extensions.contains(&"jpg") && extensions.contains(&"jpeg"));
/// assert!(extensions.contains(&"nef"), "camera RAW is an input format");
/// assert!(!extensions.contains(&"gpr"), "a RAW format with no decoder is not offered");
/// ```
pub fn input_extensions() -> Vec<&'static str> {
    // Derived rather than written down, so no front end keeps a list either: a copy there would go stale for the
    // reason `raw::decodable` gives, and refuse a format this crate opens perfectly well. The standard half is an
    // exhaustive `match` over `ImageFormat`, so a ninth format is a compile error here rather than a silent omission
    // from every picker in the application. The RAW half is `raw::RAW_EXTENSIONS` filtered by the same decodability
    // check `load` routes on.
    //
    // Recomputed per call rather than memoized: it is asked once when a picker opens, and building a 40-element vector
    // of `&'static str` is cheaper than the `LazyLock` that would avoid it.
    const EVERY_FORMAT: [ImageFormat; 8] = [
        ImageFormat::Bmp,
        ImageFormat::Gif,
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Tiff,
        ImageFormat::Avif,
        ImageFormat::Heif,
        ImageFormat::WebP,
    ];

    EVERY_FORMAT
        .into_iter()
        .flat_map(extensions_of)
        .copied()
        .chain(raw::decodable_extensions())
        .collect()
}

/// Every extension `format` is written under, canonical first.
///
/// ```
/// use opai::ImageFormat;
///
/// assert_eq!(opai::image::extensions_of(ImageFormat::Jpeg), ["jpg", "jpeg"]);
/// ```
pub fn extensions_of(format: ImageFormat) -> &'static [&'static str] {
    // An exhaustive match, which is the whole point of the function: `rust_sak::image::ImageFormat::extension` answers
    // with one canonical name per format, and a picker built from that alone would refuse the `.jpeg`, `.heic` and
    // `.tif` a camera or another application wrote. The second names come from `ImageFormat::from_extension`, which is
    // what actually decides whether a name is accepted, and this is the one place the two lists have to agree — which
    // `every_extension_maps_back_to_the_format_it_came_from` asserts they do.
    match format {
        ImageFormat::Bmp => &["bmp"],
        ImageFormat::Gif => &["gif"],
        ImageFormat::Jpeg => &["jpg", "jpeg"],
        ImageFormat::Png => &["png"],
        ImageFormat::Tiff => &["tiff", "tif"],
        ImageFormat::Avif => &["avif"],
        ImageFormat::Heif => &["heif", "heic"],
        ImageFormat::WebP => &["webp"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_canonical_extension_of_the_eight_formats_is_offered() {
        // A format this crate opens and a picker never learns about is the failure, and it is silent everywhere else.
        let offered = input_extensions();

        for extension in ["bmp", "gif", "jpg", "png", "tiff", "avif", "heif", "webp"] {
            assert!(offered.contains(&extension), "{extension} names a format this crate opens and is not offered");
        }
    }

    #[test]
    fn the_alias_a_camera_or_another_application_writes_is_offered_too() {
        // What `ImageFormat::extension` alone would cost: a photographer whose files are named `.jpeg` would find
        // every one of them greyed out in the picker, for a format the application decodes.
        let offered = input_extensions();

        for extension in ["jpeg", "tif", "heic"] {
            assert!(offered.contains(&extension), "{extension} is a name these formats are written under");
        }
    }

    #[test]
    fn every_extension_maps_back_to_the_format_it_came_from() {
        // The agreement the match above depends on, checked against `from_extension`, the codec's own reading of each
        // name.
        for format in [
            ImageFormat::Bmp,
            ImageFormat::Gif,
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::Tiff,
            ImageFormat::Avif,
            ImageFormat::Heif,
            ImageFormat::WebP,
        ] {
            for extension in extensions_of(format) {
                assert_eq!(
                    ImageFormat::from_extension(extension),
                    Some(format),
                    "{extension} is offered for {format:?} and the codec reads it as something else"
                );
            }
            assert!(
                extensions_of(format).contains(&format.extension()),
                "{format:?}'s own canonical extension is missing from its list"
            );
        }
    }

    #[test]
    fn all_twenty_nine_decodable_raw_extensions_are_offered() {
        // The whole reason the application exists for a photographer: the picker has to show them their NEFs.
        let offered = input_extensions();

        let raw: Vec<_> = offered.iter().filter(|extension| super::super::is_raw(format!("x.{extension}"))).collect();
        assert_eq!(raw.len(), 29, "the offered RAW formats are not the ones the backend decodes");
    }

    #[test]
    fn a_raw_format_with_no_decoder_is_not_offered() {
        // The 18 recognised-but-undecodable formats. Offering one would let a user choose a file the application
        // then refuses by name — a picker that lies about what it can open.
        let offered = input_extensions();

        for extension in ["gpr", "sr2", "srf", "cap", "eip", "ptx", "bay", "cine", "mdc", "k25"] {
            assert!(!offered.contains(&extension), "{extension} has no decoder and was offered anyway");
        }
    }

    #[test]
    fn nothing_is_offered_twice() {
        // A duplicate would reach a filter string as a repeated `*.jpg`, which is visible to the user.
        let mut sorted = input_extensions();
        let count = sorted.len();
        sorted.sort_unstable();
        sorted.dedup();

        assert_eq!(sorted.len(), count, "an extension is offered more than once");
    }

    #[test]
    fn everything_offered_is_lowercase_and_carries_no_dot() {
        // How a filter is built from it, and how `is_raw` and `from_extension` both expect to be asked.
        for extension in input_extensions() {
            assert!(!extension.is_empty(), "an empty extension is not a filter");
            assert!(!extension.starts_with('.'), "{extension} carries a leading dot");
            assert_eq!(extension, extension.to_lowercase(), "{extension} is not lowercase");
        }
    }

    #[test]
    fn the_order_is_the_same_on_every_call() {
        // It reaches a user as the text of a filter. A set-backed answer would reorder it per process and the dialog
        // would read differently between launches.
        assert_eq!(input_extensions(), input_extensions());
    }
}
