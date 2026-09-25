//! How many bits per channel a run's result carries, and how the third choice is resolved.

use image::DynamicImage;

use imaging::ChannelDepth;

/// How many bits per channel a caller wants an enhancement's result to carry.
///
/// **A front end's own default should be [`Source`](Self::Source)**, rather than the library's [`Eight`](Self::Eight):
/// [`load`](crate::image::load) develops a RAW to 16 bits and [`save`](crate::image::save) keeps 16 through PNG, TIFF,
/// AVIF and HEIF, so a GUI defaulting to eight would throw away depth its own loader and its own exporters both
/// preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputDepth {
    /// Eight bits per channel, whatever the source carried.
    Eight,
    /// Sixteen bits per channel, whatever the source carried.
    Sixteen,
    // Resolved here rather than inside the tiled driver, which takes a region and a shape and never sees the file the
    // pixels came from.
    /// Whatever the source carried: 16 for a source carrying more than 8 bits per channel, 8 for anything else.
    Source,
}

impl Default for OutputDepth {
    /// [`Eight`](Self::Eight), so that carrying more than 8 bits is always something a caller asked for.
    fn default() -> Self {
        // The reference has no depth concept at all and decodes every model output to 8-bit, so a caller that states
        // nothing gets exactly that behaviour and anything richer is a deliberate request rather than a surprise. A
        // library's default is conservative because a library's should be; an application knows what its user is
        // looking at.
        Self::Eight
    }
}

impl OutputDepth {
    /// The channel depth a run over `source` produces.
    pub(crate) fn resolve(self, source: &DynamicImage) -> ChannelDepth {
        match self {
            Self::Eight => ChannelDepth::Eight,
            Self::Sixteen => ChannelDepth::Sixteen,
            Self::Source => source_depth(source),
        }
    }
}

/// The depth `source`'s own pixels carry, rounded to the two this application produces. The float variants resolve to
/// sixteen.
fn source_depth(source: &DynamicImage) -> ChannelDepth {
    // Every variant written out rather than an 8-bit `_ =>` catch-all: a variant added by an `image` bump reaches the
    // last arm below rather than a silently 8-bit answer for a format that carries more.
    match source {
        DynamicImage::ImageLuma8(_)
        | DynamicImage::ImageLumaA8(_)
        | DynamicImage::ImageRgb8(_)
        | DynamicImage::ImageRgba8(_) => ChannelDepth::Eight,

        // The float variants resolve to sixteen because they carry more than eight bits, not because 16 describes
        // them — nothing in this project produces a floating-point result.
        DynamicImage::ImageLuma16(_)
        | DynamicImage::ImageLumaA16(_)
        | DynamicImage::ImageRgb16(_)
        | DynamicImage::ImageRgba16(_)
        | DynamicImage::ImageRgb32F(_)
        | DynamicImage::ImageRgba32F(_) => ChannelDepth::Sixteen,

        // `DynamicImage` is `#[non_exhaustive]`, so the arms above — which name every variant this build knows —
        // still cannot be exhaustive, and an `image` bump cannot be made a compile error here. This is what stands in
        // for one. It fails loudly under test, where `every_variant` below enumerates the same set a second time, and
        // in a release build it answers from the pixels' own declared layout rather than narrowing an unknown variant
        // to eight bits by default: the one outcome this resolution exists to prevent.
        other => {
            debug_assert!(false, "an `image` bump added a `DynamicImage` variant this resolution does not classify");

            let color = other.color();
            let per_channel = color.bits_per_pixel() / u16::from(color.channel_count());

            if per_channel > 8 { ChannelDepth::Sixteen } else { ChannelDepth::Eight }
        }
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Luma, LumaA, Rgb, Rgba};

    use super::*;

    /// Every `DynamicImage` variant, each as a one-pixel image, paired with the depth its pixels carry.
    fn every_variant() -> Vec<(DynamicImage, ChannelDepth)> {
        // Written out as a list rather than derived, because the point of the test below is that the list and the
        // resolution are two hand-written enumerations of the same enum: one of them failing to grow is what a bump
        // would otherwise do silently.
        vec![
            (DynamicImage::ImageLuma8(ImageBuffer::<Luma<u8>, _>::new(1, 1)), ChannelDepth::Eight),
            (DynamicImage::ImageLumaA8(ImageBuffer::<LumaA<u8>, _>::new(1, 1)), ChannelDepth::Eight),
            (DynamicImage::ImageRgb8(ImageBuffer::<Rgb<u8>, _>::new(1, 1)), ChannelDepth::Eight),
            (DynamicImage::ImageRgba8(ImageBuffer::<Rgba<u8>, _>::new(1, 1)), ChannelDepth::Eight),
            (DynamicImage::ImageLuma16(ImageBuffer::<Luma<u16>, _>::new(1, 1)), ChannelDepth::Sixteen),
            (DynamicImage::ImageLumaA16(ImageBuffer::<LumaA<u16>, _>::new(1, 1)), ChannelDepth::Sixteen),
            (DynamicImage::ImageRgb16(ImageBuffer::<Rgb<u16>, _>::new(1, 1)), ChannelDepth::Sixteen),
            (DynamicImage::ImageRgba16(ImageBuffer::<Rgba<u16>, _>::new(1, 1)), ChannelDepth::Sixteen),
            (DynamicImage::ImageRgb32F(ImageBuffer::<Rgb<f32>, _>::new(1, 1)), ChannelDepth::Sixteen),
            (DynamicImage::ImageRgba32F(ImageBuffer::<Rgba<f32>, _>::new(1, 1)), ChannelDepth::Sixteen),
        ]
    }

    #[test]
    fn the_default_is_eight_so_more_than_eight_bits_is_always_a_request() {
        // Pinned on its own, because a default that drifted would change what every caller passing `None` receives
        // without any of them changing.
        assert_eq!(OutputDepth::default(), OutputDepth::Eight);
    }

    #[test]
    fn the_source_depth_of_every_dynamic_image_variant_is_classified() {
        for (source, expected) in every_variant() {
            assert_eq!(OutputDepth::Source.resolve(&source), expected, "{source:?} resolved to the wrong depth",);
        }
    }

    #[test]
    fn the_two_stated_depths_ignore_what_the_source_carried() {
        for (source, _) in every_variant() {
            assert_eq!(OutputDepth::Eight.resolve(&source), ChannelDepth::Eight);
            assert_eq!(OutputDepth::Sixteen.resolve(&source), ChannelDepth::Sixteen);
        }
    }
}
