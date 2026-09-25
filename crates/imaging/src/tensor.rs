//! Pixels to the numbers a model reads, and back again.

use image::{DynamicImage, ImageBuffer, Rgb, Rgba};

use crate::grid::Tile;
use crate::pad;

// Both are shipped: the three convolutional upscalers were trained against `[0, 1]`, and the diffusion upscaler and
// several of the other families against `[-1, 1]` — so the two live side by side even within one family.
/// The range a model's input and output are normalised over: the model's property, which it was trained against.
///
/// Feeding a model the wrong one is not an error a runtime can report: it is a worse image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Normalisation {
    /// `[0, 1]`.
    Unit,
    /// `[-1, 1]`.
    Signed,
}

impl Normalisation {
    /// A 16-bit channel value as the float a model is fed.
    #[inline]
    fn encode(self, value: u16) -> f32 {
        self.encode_unit(f32::from(value) / f32::from(u16::MAX))
    }

    /// A `[0, 1]` channel fraction as the float a model is fed.
    ///
    /// For a caller whose channel value is **fractional** rather than a `u16`: a warp that interpolates four samples
    /// before normalising them has a float in channel space and nothing to round it to.
    #[inline]
    pub fn encode_unit(self, unit: f32) -> f32 {
        // `encode`'s own body, split out because rounding a fraction to a `u16` purely to pass it to `encode` would
        // quantise a 16-bit source in the one step that exists to avoid quantising it. One definition of the range
        // mapping rather than two: a second copy of `[-1, 1]`'s `2x - 1` somewhere else is a copy that can disagree
        // with this one.
        match self {
            Self::Unit => unit,
            Self::Signed => unit.mul_add(2.0, -1.0),
        }
    }

    /// A float a model produced back to a `[0, 1]` fraction, before it is quantised to a channel.
    #[inline]
    pub fn decode(self, value: f32) -> f32 {
        // Reachable on its own for the same reason `encode_unit` is: a composite that samples a model's output tensor
        // bilinearly and blends it against the picture has a fraction to produce and no image to produce it from, and
        // a second copy of `[-1, 1]`'s inverse elsewhere is a copy that can disagree with this one.
        match self {
            Self::Unit => value,
            Self::Signed => value.mul_add(0.5, 0.5),
        }
    }
}

// **`DynamicImage::get_pixel` is not used here and must not be.** `DynamicImage`'s `GenericImageView` implementation
// is typed `Rgba<u8>`, so that accessor maps every variant through an 8-bit conversion — reading a 16-bit image
// through it discards the low byte of every channel, silently and with no error. That is the whole reason this type
// exists: the four variants the loader actually produces are borrowed and indexed directly, and everything else is
// converted **up** to 16 bits once rather than narrowed per pixel.
/// One image's pixels, presented as 16-bit RGB triples whatever the image actually holds.
pub enum Sampler<'a> {
    /// The two 8-bit variants every one of the eight file formats decodes into, borrowed.
    Rgb8(&'a ImageBuffer<Rgb<u8>, Vec<u8>>),
    Rgba8(&'a ImageBuffer<Rgba<u8>, Vec<u8>>),
    /// The two 16-bit variants a developed RAW produces, borrowed.
    Rgb16(&'a ImageBuffer<Rgb<u16>, Vec<u16>>),
    Rgba16(&'a ImageBuffer<Rgba<u16>, Vec<u16>>),
    /// Everything else — the greyscale variants and the two float ones — converted once and owned. It costs one
    /// full-image conversion for a 16-bit greyscale scan, which is rare, and it is correct rather than lossy.
    Owned(ImageBuffer<Rgba<u16>, Vec<u16>>),
}

impl Sampler<'_> {
    /// Borrows `source` where its variant can be indexed directly, and converts it once where it cannot.
    pub fn new(source: &DynamicImage) -> Sampler<'_> {
        match source {
            DynamicImage::ImageRgb8(buffer) => Sampler::Rgb8(buffer),
            DynamicImage::ImageRgba8(buffer) => Sampler::Rgba8(buffer),
            DynamicImage::ImageRgb16(buffer) => Sampler::Rgb16(buffer),
            DynamicImage::ImageRgba16(buffer) => Sampler::Rgba16(buffer),
            other => Sampler::Owned(other.to_rgba16()),
        }
    }

    /// The pixel at `(x, y)` as a 16-bit RGB triple, with straight alpha premultiplied against black.
    #[inline]
    pub fn rgb(&self, x: u32, y: u32) -> [u16; 3] {
        self.rgb_and_alpha(x, y).0
    }

    // One match behind both accessors rather than a second copy of it, so the colour a caller that also reads alpha
    // sees cannot drift from the colour every model is fed. `rgb` inlines this, and discarding the alpha costs nothing.
    /// The pixel at `(x, y)` exactly as [`rgb`](Self::rgb) returns it, and beside it the pixel's own alpha at 16 bits:
    /// `u16::MAX` for a source that has no alpha channel.
    ///
    /// The colour is **still premultiplied**. The alpha is for a caller that has to know how much of that colour is the
    /// black it was composited against, such as a statistic that must not read a cut-out's transparent surround as a
    /// dark photograph.
    #[inline]
    pub fn rgb_and_alpha(&self, x: u32, y: u32) -> ([u16; 3], u16) {
        // The premultiply is not a choice made here: the reference feeds a model premultiplied values because Go's
        // colour accessor returns them, and the `image` crate's `Rgba` is straight alpha, so producing the same numbers
        // takes an explicit multiply. Skipping it would be a silent behaviour divergence on every transparent PNG.
        match self {
            Sampler::Rgb8(buffer) => {
                let Rgb([r, g, b]) = *buffer.get_pixel(x, y);
                ([widen(r), widen(g), widen(b)], u16::MAX)
            }
            Sampler::Rgba8(buffer) => {
                let Rgba([r, g, b, a]) = *buffer.get_pixel(x, y);
                let alpha = widen(a);
                (premultiply([widen(r), widen(g), widen(b)], alpha), alpha)
            }
            Sampler::Rgb16(buffer) => (buffer.get_pixel(x, y).0, u16::MAX),
            Sampler::Rgba16(buffer) => {
                let Rgba([r, g, b, a]) = *buffer.get_pixel(x, y);
                (premultiply([r, g, b], a), a)
            }
            Sampler::Owned(buffer) => {
                let Rgba([r, g, b, a]) = *buffer.get_pixel(x, y);
                (premultiply([r, g, b], a), a)
            }
        }
    }

    /// Row `y`'s first `out.len()` pixels, exactly as [`rgb`](Self::rgb) returns each of them.
    ///
    /// # Panics
    ///
    /// Panics when `y` or `out.len()` is past the image, as `rgb` would.
    pub fn row(&self, y: u32, out: &mut [[u16; 3]]) {
        self.read_row(y, None, out);
    }

    /// Row `y`'s pixels at `columns`, in that order, exactly as [`rgb`](Self::rgb) returns each of them.
    ///
    /// # Panics
    ///
    /// Panics when `y` or any column is past the image, as `rgb` would, or when `columns` and `out` differ in length.
    pub fn row_at(&self, y: u32, columns: &[u32], out: &mut [[u16; 3]]) {
        assert_eq!(columns.len(), out.len(), "one output slot per column");
        self.read_row(y, Some(columns), out);
    }

    // The full-resolution loops read through these rather than through `rgb`, which matches on the variant and
    // bounds-checks `(x, y)` once per pixel — twenty-four million times over a 24-megapixel photograph. Here the match
    // happens once per row and each arm walks the row's own storage, so the per-pixel work is the conversion alone.
    // The conversions are `rgb_and_alpha`'s, and a test pins the two paths to the same bits for every variant.
    fn read_row(&self, y: u32, columns: Option<&[u32]>, out: &mut [[u16; 3]]) {
        match self {
            Sampler::Rgb8(buffer) => {
                decode_row(row_of(buffer, y), columns, out, |[r, g, b]: [u8; 3]| [widen(r), widen(g), widen(b)]);
            }
            Sampler::Rgba8(buffer) => decode_row(row_of(buffer, y), columns, out, |[r, g, b, a]: [u8; 4]| {
                premultiply([widen(r), widen(g), widen(b)], widen(a))
            }),
            Sampler::Rgb16(buffer) => decode_row(row_of(buffer, y), columns, out, |rgb: [u16; 3]| rgb),
            Sampler::Rgba16(buffer) => {
                decode_row(row_of(buffer, y), columns, out, |[r, g, b, a]: [u16; 4]| premultiply([r, g, b], a));
            }
            Sampler::Owned(buffer) => {
                decode_row(row_of(buffer, y), columns, out, |[r, g, b, a]: [u16; 4]| premultiply([r, g, b], a));
            }
        }
    }
}

/// Row `y` of `buffer`'s storage, every channel of every pixel in it.
fn row_of<P: image::Pixel>(buffer: &ImageBuffer<P, Vec<P::Subpixel>>, y: u32) -> &[P::Subpixel] {
    assert!(y < buffer.height(), "row {y} is outside an image {} rows high", buffer.height());

    let stride = buffer.width() as usize * usize::from(P::CHANNEL_COUNT);
    &buffer.as_raw()[y as usize * stride..][..stride]
}

/// `row`'s pixels — the first `out.len()` of them, or those at `columns` — through `decode` into `out`.
#[inline]
fn decode_row<S: Copy, const C: usize>(
    row: &[S],
    columns: Option<&[u32]>,
    out: &mut [[u16; 3]],
    decode: impl Fn([S; C]) -> [u16; 3],
) {
    let pixels = row.as_chunks::<C>().0;

    match columns {
        None => {
            let pixels = &pixels[..out.len()];

            for (slot, pixel) in out.iter_mut().zip(pixels) {
                *slot = decode(*pixel);
            }
        }
        Some(columns) => {
            for (slot, &x) in out.iter_mut().zip(columns) {
                *slot = decode(pixels[x as usize]);
            }
        }
    }
}

// Its own error rather than a partial write or a panic: a short buffer means a caller's scratch and the shape it
// believes the model accepts have drifted apart, and writing what fits would feed the model the previous tile's pixels
// in the region left over.
/// A buffer handed to a conversion was not the length the shape it was asked for requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the tensor buffer holds {actual} floats, but a {width}x{height} extent needs {expected}")]
pub struct TensorShape {
    /// How many floats the shape requires: three planes of `width * height`.
    pub expected: usize,
    /// How many the caller supplied.
    pub actual: usize,
    /// The width the conversion was asked for — a tile's shape, or a whole padded extent.
    pub width: u32,
    /// The height the conversion was asked for.
    pub height: u32,
}

/// Writes `region` of `source` into `dest` as the planar CHW `f32` a fixed-shape model reads: the whole red plane,
/// then the whole green one, then the whole blue one, each `shape` x `shape` and row-major.
///
/// Where `region` is smaller than `shape` — which happens only when the whole image is smaller than one tile — the
/// region's own edge pixels are mirrored outwards to fill it. See [`pad`].
///
/// # Errors
///
/// Returns [`TensorShape`] when `dest` is not exactly `3 * shape * shape` floats long, having written nothing.
pub fn image_to_chw(
    dest: &mut [f32],
    sampler: &Sampler<'_>,
    region: Tile,
    shape: u32,
    norm: Normalisation,
) -> Result<(), TensorShape> {
    extend_to_chw(dest, sampler, (region.x, region.y), (region.width, region.height), (shape, shape), norm)
}

/// Writes the whole of an image, extended to `extent`, into `dest` as planar CHW `f32`.
///
/// The non-square, whole-image sibling of [`image_to_chw`]. `source` is the image's own dimensions and `extent` is
/// the padded extent to produce; past the source's dimensions on either axis the source's own edge pixels are
/// mirrored outwards through the same [`pad::source_offset`] the tile conversion uses. Only the right and bottom edges
/// are ever extended, which is what a padded extent means here.
///
/// # Errors
///
/// Returns [`TensorShape`] when `dest` is not exactly three planes of `extent`, having written nothing.
///
/// # Panics
///
/// Panics when either of `source`'s dimensions is zero.
pub fn padded_to_chw(
    dest: &mut [f32],
    sampler: &Sampler<'_>,
    source: (u32, u32),
    extent: (u32, u32),
    norm: Normalisation,
) -> Result<(), TensorShape> {
    // A sibling rather than two more parameters on `image_to_chw`, because the two answer different questions.
    // `image_to_chw` takes one square `shape` and a `Tile`, and its contract is *"one region at the shape the model
    // accepts"* — the shape is the model's and the tile is the grid's, and the compiler keeps a scratch allocated at
    // that shape agreeing with it. This one takes a width and a height and reads a whole image at neither a tile's
    // origin nor a model's shape. Collapsing them would give the tile path a width and a height it has no use for and
    // one more way for a caller to disagree with the grid — the same reasoning that keeps `run_tile` and `run_graph`
    // apart.
    extend_to_chw(dest, sampler, (0, 0), source, extent, norm)
}

/// The one loop behind [`image_to_chw`] and [`padded_to_chw`]: `extent` pixels of planar CHW read from `origin`, with
/// anything past `source` mirrored back in through [`pad::source_offset`].
///
/// # Errors
///
/// Returns [`TensorShape`] when `dest` is not exactly three planes of `extent`, having written nothing.
fn extend_to_chw(
    dest: &mut [f32],
    sampler: &Sampler<'_>,
    origin: (u32, u32),
    source: (u32, u32),
    extent: (u32, u32),
    norm: Normalisation,
) -> Result<(), TensorShape> {
    // Private, and deliberately so. The two public contracts above stay apart for the reasons `padded_to_chw` gives,
    // but that is an argument about what a *caller* may say, not about writing the traversal twice. Written once, the
    // mirror rule, the plane offsets and the column tabulation cannot drift between the region path and the
    // whole-image one.
    let (origin_x, origin_y) = origin;
    let (source_width, source_height) = source;
    let (width, height) = extent;

    let plane = (width as usize) * (height as usize);
    let expected = 3 * plane;

    if dest.len() != expected {
        return Err(TensorShape { expected, actual: dest.len(), width, height });
    }

    // Nothing to write, and `chunks_exact_mut` below refuses a zero-width row.
    if plane == 0 {
        return Ok(());
    }

    // Tabulated once rather than evaluated per pixel, as `blend::blend_tile` does with its ramp weights and for the
    // same reason: the horizontal source offset depends only on the column, so evaluating it inside the inner loop
    // would repeat every mirror — and its bounds assertion — on each of `height` rows, over an extent that at 4x is
    // nine figures of them.
    let columns: Vec<u32> = (0..width).map(|column| origin_x + pad::source_offset(column, source_width)).collect();
    let mut pixels = vec![[0_u16; 3]; width as usize];

    let (red, rest) = dest.split_at_mut(plane);
    let (green, blue) = rest.split_at_mut(plane);
    let rows = red.chunks_exact_mut(width as usize).zip(green.chunks_exact_mut(width as usize));

    for (row, ((red, green), blue)) in rows.zip(blue.chunks_exact_mut(width as usize)).enumerate() {
        let y = origin_y + pad::source_offset(row as u32, source_height);
        sampler.row_at(y, &columns, &mut pixels);

        for (((red, green), blue), &[r, g, b]) in red.iter_mut().zip(green.iter_mut()).zip(blue.iter_mut()).zip(&pixels)
        {
            *red = norm.encode(r);
            *green = norm.encode(g);
            *blue = norm.encode(b);
        }
    }

    Ok(())
}

// The decode and the seam blend are written once against this and instantiated twice, rather than in two concrete
// copies. The ramp arithmetic is the one place in this module where a divergence between two implementations is a
// visible artefact rather than a wrong number, so there is one of it.
//
// A model's output is floating point and carries more precision than either depth, so neither is the natural one.
// Fixing it at eight — which is what the reference implementation does — would mean a 16-bit RAW losing its depth in
// the middle of a pipeline whose loader and whose four 16-bit export formats both preserve it, and no later change
// could recover it. `depth::OutputDepth` resolves "8", "16" and "whatever the source was" onto these two.
/// A channel a result can be written at: eight bits per channel or sixteen.
pub trait Channel: image::Primitive + 'static
where
    Rgb<Self>: image::Pixel<Subpixel = Self>,
{
    /// The largest value the channel holds, as the float the arithmetic is done in.
    const MAX_VALUE: f32;

    // The reference truncates, only because Go's conversion from a float does. Truncating biases every channel down by
    // half a level, and — because the seam blend reads a channel back out and writes it again — it turns a pixel
    // blended against an identical neighbour into one a level darker, which is a visible stripe down every seam rather
    // than a rounding detail.
    /// The channel value for a `[0, 1]` fraction, clamping anything outside that range to the nearest end of it.
    ///
    /// Rounded to the nearest value rather than truncated towards zero.
    fn from_unit(value: f32) -> Self;

    /// The channel value as a `[0, 1]` fraction. Exactly inverse to [`from_unit`](Self::from_unit) for every value
    /// either depth can hold.
    fn to_unit(self) -> f32;

    // The hook that lets a whole-photograph conversion be skipped rather than performed and undone. Carrying an `Rgb8`
    // source through to an eight-bit result runs every channel through `widen`, a divide by 65535 and a multiply back
    // by 255 — ten operations a pixel, twenty-four million of them for a 24-megapixel photograph, every one of which
    // is the identity. `Rgba8` and the rest have straight alpha to premultiply against black, which is a real
    // conversion and not one this can skip.
    /// `sampler`'s pixels where it already carries them at this depth with no alpha to premultiply, or `None`.
    fn matching<'a>(sampler: &'a Sampler<'_>) -> Option<&'a ImageBuffer<Rgb<Self>, Vec<Self>>>;

    /// The finished result as the variant of [`DynamicImage`] that carries this depth.
    fn into_dynamic(buffer: ImageBuffer<Rgb<Self>, Vec<Self>>) -> DynamicImage;
}

impl Channel for u8 {
    const MAX_VALUE: f32 = u8::MAX as f32;

    #[inline]
    fn from_unit(value: f32) -> Self {
        // `as` saturates at the type's bounds for a float, so the clamp is what keeps an out-of-range model output
        // from landing on the wrong extreme; it is stated rather than relied on because the multiply happens first.
        (value.clamp(0.0, 1.0) * Self::MAX_VALUE).round() as Self
    }

    #[inline]
    fn to_unit(self) -> f32 {
        f32::from(self) / Self::MAX_VALUE
    }

    #[inline]
    fn matching<'a>(sampler: &'a Sampler<'_>) -> Option<&'a ImageBuffer<Rgb<Self>, Vec<Self>>> {
        match sampler {
            Sampler::Rgb8(buffer) => Some(buffer),
            _ => None,
        }
    }

    fn into_dynamic(buffer: ImageBuffer<Rgb<Self>, Vec<Self>>) -> DynamicImage {
        DynamicImage::ImageRgb8(buffer)
    }
}

impl Channel for u16 {
    const MAX_VALUE: f32 = u16::MAX as f32;

    #[inline]
    fn from_unit(value: f32) -> Self {
        (value.clamp(0.0, 1.0) * Self::MAX_VALUE).round() as Self
    }

    #[inline]
    fn to_unit(self) -> f32 {
        f32::from(self) / Self::MAX_VALUE
    }

    #[inline]
    fn matching<'a>(sampler: &'a Sampler<'_>) -> Option<&'a ImageBuffer<Rgb<Self>, Vec<Self>>> {
        match sampler {
            Sampler::Rgb16(buffer) => Some(buffer),
            _ => None,
        }
    }

    fn into_dynamic(buffer: ImageBuffer<Rgb<Self>, Vec<Self>>) -> DynamicImage {
        DynamicImage::ImageRgb16(buffer)
    }
}

// One definition for every pipeline that needs the photograph itself as an RGB buffer — face recovery's canvas, and
// colorization's pre-stretch flatten — rather than a copy each whose rounding could drift from the other's.
/// `sampler`'s pixels as an RGB buffer of `width` by `height` at `T`'s depth, with any alpha composited against black
/// on the way in.
///
/// Read through [`Sampler`] rather than through `DynamicImage`'s own accessor, which is typed `Rgba<u8>` and would
/// discard the low byte of every channel of a 16-bit source.
pub fn flattened<T: Channel>(sampler: &Sampler<'_>, width: u32, height: u32) -> ImageBuffer<Rgb<T>, Vec<T>>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // Where the source already carries this depth with no alpha to premultiply, the conversion below is the identity
    // performed ten times a pixel — so the buffer is copied wholesale instead. For the ordinary case of an eight-bit
    // photograph carried to an eight-bit result that is one memcpy in place of twenty-four million closure calls on a
    // 24-megapixel source. The equality of the two paths is pinned by the test below.
    if let Some(buffer) = T::matching(sampler) {
        return buffer.clone();
    }

    // Through the unit float and this crate's rounding, which at 8 bits is the nearest byte to `v / 257`: an opaque
    // 8-bit channel arrives as `v * 257` and leaves as `v`. `to_unit` rather than the division written out: it is
    // `Channel`'s own `[0, 1]` mapping, and it is exactly inverse to the `from_unit` on the other side of it.
    let mut raw = Vec::with_capacity(3 * width as usize * height as usize);
    let mut pixels = vec![[0_u16; 3]; width as usize];

    for y in 0..height {
        sampler.row(y, &mut pixels);
        raw.extend(pixels.iter().flatten().map(|value| T::from_unit(value.to_unit())));
    }

    ImageBuffer::from_raw(width, height, raw).expect("three channels for every pixel of every row")
}

/// Writes a model's planar CHW output into `dest`, reversing the normalisation it was produced under.
///
/// The result carries **colour only**: alpha is not preserved through a model run.
///
/// # Errors
///
/// Returns [`TensorShape`] when `output` is not exactly three planes of `dest`'s dimensions, having written nothing.
pub fn chw_to_image<T: Channel>(
    dest: &mut ImageBuffer<Rgb<T>, Vec<T>>,
    output: &[f32],
    norm: Normalisation,
) -> Result<(), TensorShape>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // No alpha because the composite against black that `Sampler::rgb` performs has already happened by the time a
    // value reaches here, and a fourth channel would be claiming the model returned an opacity it was never given. The
    // reference writes RGBA with every alpha byte forced opaque; the pixels are identical and this is a quarter off
    // the largest allocation the application makes.
    decode_chw(dest, output, dest.dimensions(), norm)
}

/// Writes the **top-left** of a model's planar CHW output into `dest`, reversing the normalisation it was produced
/// under, where the tensor is a padded `extent` and `dest` is the region inside it that is real picture.
///
/// [`chw_to_image`]'s cropped sibling, and the decode side of [`padded_to_chw`]: that one writes an image into the
/// padded square a fixed-shape graph accepts, and this one reads the image back out of the square the graph
/// returned.
///
/// Only the right and bottom are ever extended, which is what a padded extent means here and why the region wanted
/// is contiguous from the origin.
///
/// # Errors
///
/// Returns [`TensorShape`] when `output` is not exactly three planes of `extent`, or when `dest` is larger than
/// `extent` on either axis, having written nothing.
pub fn padded_chw_to_image<T: Channel>(
    dest: &mut ImageBuffer<Rgb<T>, Vec<T>>,
    output: &[f32],
    extent: (u32, u32),
    norm: Normalisation,
) -> Result<(), TensorShape>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // The extension is dropped **here**, before anything downstream is derived from it — a family that upsampled the
    // whole square and cropped afterwards would have resampled the mirror of its own edge into the pixels it keeps,
    // and would carry an aspect ratio that is the canvas's rather than the photograph's.
    //
    // A sibling rather than an optional extent on `chw_to_image`, for the argument `padded_to_chw` makes on the other
    // side of the conversion. `chw_to_image`'s contract is *"the whole of what the model returned"*, and the compiler
    // keeps a buffer allocated at the graph's shape agreeing with it; this one takes a tensor extent that is
    // deliberately **not** the destination's, which is one more thing for a caller to get wrong wherever it is not
    // wanted.
    decode_chw(dest, output, extent, norm)
}

/// The one loop behind [`chw_to_image`] and [`padded_chw_to_image`]: `dest`'s own extent read out of the top-left of
/// a tensor that is three planes of `extent`.
///
/// # Errors
///
/// Returns [`TensorShape`] when `output` is not three planes of `extent`, or when `dest` does not fit inside
/// `extent`, having written nothing.
fn decode_chw<T: Channel>(
    dest: &mut ImageBuffer<Rgb<T>, Vec<T>>,
    output: &[f32],
    extent: (u32, u32),
    norm: Normalisation,
) -> Result<(), TensorShape>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // Private, for the reason `extend_to_chw` is: the two public contracts above stay apart so that a caller cannot
    // name a sub-extent where none is wanted, which is an argument about what a caller may *say* rather than about
    // writing the traversal twice. Written once, the plane offsets, the stride and the decode cannot drift between the
    // whole-tensor path and the cropped one.
    let (width, height) = extent;
    let (region_width, region_height) = dest.dimensions();

    let plane = (width as usize) * (height as usize);
    let expected = 3 * plane;

    if output.len() != expected {
        return Err(TensorShape { expected, actual: output.len(), width, height });
    }

    // A region reaching past the tensor, reported as what it would have needed rather than as what the tensor holds:
    // the two numbers together are the whole of the mistake, and a shape error that named the tensor's own extent
    // would describe a buffer that is exactly the length it should be.
    if region_width > width || region_height > height {
        return Err(TensorShape {
            expected: 3 * (region_width as usize) * (region_height as usize),
            actual: output.len(),
            width: region_width,
            height: region_height,
        });
    }

    let (green, blue) = (plane, 2 * plane);

    // Walked a row at a time rather than as an `x`/`y` pair, so the offset into each plane is one addition per pixel
    // and one multiply per row — where a nested loop would recompute `y * width + x` and then have `put_pixel`
    // recompute and bounds-check it again, `extent` squared times per run.
    //
    // The stride is the **tensor's** width rather than the destination's, which is the whole of what the cropped
    // path is: where the two are equal this is exactly the flat walk over the planes that `chw_to_image` needs.
    for (row, pixels) in dest.rows_mut().enumerate() {
        let base = row * (width as usize);

        for (column, pixel) in pixels.enumerate() {
            let index = base + column;

            *pixel = Rgb([
                T::from_unit(norm.decode(output[index])),
                T::from_unit(norm.decode(output[green + index])),
                T::from_unit(norm.decode(output[blue + index])),
            ]);
        }
    }

    Ok(())
}

/// An 8-bit channel as the 16-bit one it is exactly: `* 257` maps 0 to 0 and 255 to 65535 with nothing in between
/// rounded.
fn widen(value: u8) -> u16 {
    u16::from(value) * 257
}

/// Composites a straight-alpha triple against black, which is what a premultiplied value is.
fn premultiply(channels: [u16; 3], alpha: u16) -> [u16; 3] {
    if alpha == u16::MAX {
        return channels;
    }

    channels.map(|value| {
        // Widened to `u32` because the product of two 16-bit channels overflows a `u16` at almost every value.
        ((u32::from(value) * u32::from(alpha)) / u32::from(u16::MAX)) as u16
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::gradient;

    /// [`flattened`]'s converting path, with the depth-matched shortcut bypassed.
    fn flattened_converting<T: Channel>(sampler: &Sampler<'_>, width: u32, height: u32) -> ImageBuffer<Rgb<T>, Vec<T>>
    where
        Rgb<T>: image::Pixel<Subpixel = T>,
    {
        ImageBuffer::from_fn(width, height, |x, y| {
            let [r, g, b] = sampler.rgb(x, y);

            Rgb([T::from_unit(r.to_unit()), T::from_unit(g.to_unit()), T::from_unit(b.to_unit())])
        })
    }

    #[test]
    fn the_depth_matched_copy_carries_exactly_what_the_conversion_would_have() {
        // Every value an eight-bit channel can hold, not a sample of them: the shortcut's whole claim is that the
        // round trip through `widen`, a divide by 65535 and a multiply by 255 is the identity, and the only honest
        // way to assert that is over the full domain.
        let eight: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(256, 1, |x, _| Rgb([x as u8, 255 - x as u8, (x as u8).wrapping_mul(7)]));
        let source = DynamicImage::ImageRgb8(eight);
        let sampler = Sampler::new(&source);

        let copied: ImageBuffer<Rgb<u8>, Vec<u8>> = flattened(&sampler, 256, 1);
        let converted: ImageBuffer<Rgb<u8>, Vec<u8>> = flattened_converting(&sampler, 256, 1);

        assert_eq!(copied.as_raw(), converted.as_raw(), "the eight-bit shortcut is not the conversion it replaces");

        // The sixteen-bit pairing, over a spread that includes both ends and the values either side of the
        // midpoint, where a reciprocal that was not exactly representable would show first.
        let wide: ImageBuffer<Rgb<u16>, Vec<u16>> = ImageBuffer::from_fn(512, 1, |x, _| {
            let value = (u32::from(x as u16) * 65535 / 511) as u16;
            Rgb([value, 65535 - value, value ^ 0x5555])
        });
        let source = DynamicImage::ImageRgb16(wide);
        let sampler = Sampler::new(&source);

        let copied: ImageBuffer<Rgb<u16>, Vec<u16>> = flattened(&sampler, 512, 1);
        let converted: ImageBuffer<Rgb<u16>, Vec<u16>> = flattened_converting(&sampler, 512, 1);

        assert_eq!(
            copied.as_raw(),
            converted.as_raw(),
            "the sixteen-bit shortcut is not the conversion it replaces"
        );
    }

    #[test]
    fn a_sixteen_bit_source_keeps_the_precision_the_eight_bit_accessor_would_have_narrowed() {
        // 0x1234 and 0x123f are eleven levels apart, which is below what eight bits can express, so
        // `DynamicImage::get_pixel` — typed `Rgba<u8>` — reports one value for both and cannot tell them apart. The
        // sampler reports each in full.
        let mut buffer = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(2, 1);
        buffer.put_pixel(0, 0, Rgb([0x1234, 0x5678, 0x9abc]));
        buffer.put_pixel(1, 0, Rgb([0x123f, 0x5600, 0x9a01]));

        let source = DynamicImage::ImageRgb16(buffer);
        let sampler = Sampler::new(&source);

        assert_eq!(sampler.rgb(0, 0), [0x1234, 0x5678, 0x9abc]);
        assert_eq!(sampler.rgb(1, 0), [0x123f, 0x5600, 0x9a01]);

        // What the dangerous accessor would have given instead: two distinct channels reported as one value,
        // because `DynamicImage`'s `GenericImageView` is typed `Rgba<u8>` and narrows every variant through it.
        use image::GenericImageView as _;
        assert_eq!(
            source.get_pixel(0, 0).0[0],
            source.get_pixel(1, 0).0[0],
            "the accessor this type exists to avoid has stopped narrowing; the reason for the borrow is gone"
        );
    }

    #[test]
    fn an_eight_bit_source_widens_exactly() {
        let mut buffer = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(3, 1);
        buffer.put_pixel(0, 0, Rgb([0, 0, 0]));
        buffer.put_pixel(1, 0, Rgb([128, 1, 254]));
        buffer.put_pixel(2, 0, Rgb([255, 255, 255]));

        let source = DynamicImage::ImageRgb8(buffer);
        let sampler = Sampler::new(&source);

        assert_eq!(sampler.rgb(0, 0), [0, 0, 0]);
        assert_eq!(sampler.rgb(1, 0), [128 * 257, 257, 254 * 257]);
        assert_eq!(sampler.rgb(2, 0), [65535, 65535, 65535], "255 did not widen to the full range");
    }

    #[test]
    fn a_variant_the_loader_does_not_produce_is_converted_up_rather_than_narrowed() {
        let mut buffer = ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(2, 1);
        buffer.put_pixel(0, 0, image::Luma([0x1234]));
        buffer.put_pixel(1, 0, image::Luma([0x12ff]));

        let source = DynamicImage::ImageLuma16(buffer);
        let sampler = Sampler::new(&source);

        assert!(matches!(Sampler::new(&source), Sampler::Owned(_)), "Luma16 did not take the owned path");
        assert_eq!(sampler.rgb(0, 0), [0x1234, 0x1234, 0x1234]);
        assert_eq!(sampler.rgb(1, 0), [0x12ff, 0x12ff, 0x12ff], "the greyscale conversion narrowed the channel");
    }

    /// A one-tile region over the whole of an image, which is what every conversion test here converts.
    fn whole(source: &DynamicImage) -> Tile {
        Tile { x: 0, y: 0, width: source.width(), height: source.height() }
    }

    #[test]
    fn both_normalisations_land_on_the_endpoints_they_document() {
        // A black column beside a white one, so both ends of the range appear in every plane.
        let mut buffer = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(2, 2);
        for y in 0..2 {
            buffer.put_pixel(0, y, Rgb([0, 0, 0]));
            buffer.put_pixel(1, y, Rgb([65535, 65535, 65535]));
        }

        let source = DynamicImage::ImageRgb16(buffer);
        let sampler = Sampler::new(&source);

        let mut unit = vec![0.0; 3 * 4];
        image_to_chw(&mut unit, &sampler, whole(&source), 2, Normalisation::Unit).unwrap();
        assert_eq!(unit, vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);

        let mut signed = vec![0.0; 3 * 4];
        image_to_chw(&mut signed, &sampler, whole(&source), 2, Normalisation::Signed).unwrap();
        assert_eq!(signed, vec![-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0]);
    }

    #[test]
    fn the_layout_is_channel_major_rather_than_interleaved() {
        // One red pixel, one green, one blue and one black, so an interleaved layout and a planar one cannot produce
        // the same buffer.
        let mut buffer = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(2, 2);
        buffer.put_pixel(0, 0, Rgb([255, 0, 0]));
        buffer.put_pixel(1, 0, Rgb([0, 255, 0]));
        buffer.put_pixel(0, 1, Rgb([0, 0, 255]));
        buffer.put_pixel(1, 1, Rgb([0, 0, 0]));

        let source = DynamicImage::ImageRgb8(buffer);
        let sampler = Sampler::new(&source);

        let mut tensor = vec![0.0; 3 * 4];
        image_to_chw(&mut tensor, &sampler, whole(&source), 2, Normalisation::Unit).unwrap();

        // The whole red plane, then the whole green one, then the whole blue one — each of them a 2x2 image in
        // row-major order.
        assert_eq!(tensor[0..4], [1.0, 0.0, 0.0, 0.0], "the red plane is not the first four values");
        assert_eq!(tensor[4..8], [0.0, 1.0, 0.0, 0.0], "the green plane is not the second four");
        assert_eq!(tensor[8..12], [0.0, 0.0, 1.0, 0.0], "the blue plane is not the third four");
    }

    #[test]
    fn a_buffer_of_the_wrong_length_is_refused_rather_than_partly_written() {
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(2, 2, Rgb([255, 255, 255])));
        let sampler = Sampler::new(&source);

        for length in [0, 11, 13] {
            let mut tensor = vec![f32::NAN; length];
            let error = image_to_chw(&mut tensor, &sampler, whole(&source), 2, Normalisation::Unit).unwrap_err();

            assert_eq!(error.expected, 12);
            assert_eq!(error.actual, length);
            assert!(tensor.iter().all(|value| value.is_nan()), "a refused conversion wrote into the buffer");
        }
    }

    #[test]
    fn a_region_shorter_than_the_shape_is_mirrored_out_to_it() {
        // Two pixels wide, converted at a shape of four: the two added columns mirror back across the last pixel, so
        // the row reads red, green, green, red rather than ending in a border the model would enhance.
        let mut buffer = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(2, 1);
        buffer.put_pixel(0, 0, Rgb([255, 0, 0]));
        buffer.put_pixel(1, 0, Rgb([0, 255, 0]));

        let source = DynamicImage::ImageRgb8(buffer);
        let sampler = Sampler::new(&source);

        let mut tensor = vec![0.0; 3 * 16];
        image_to_chw(&mut tensor, &sampler, whole(&source), 4, Normalisation::Unit).unwrap();

        assert_eq!(tensor[0..4], [1.0, 0.0, 0.0, 1.0], "the row was not mirrored back into itself");
        // Every row of the shape is that same mirrored row, the single source row having been mirrored downwards.
        for row in 1..4 {
            assert_eq!(tensor[row * 4..row * 4 + 4], [1.0, 0.0, 0.0, 1.0], "row {row} is not the mirror of the source");
        }
    }

    #[test]
    fn an_eight_bit_and_a_sixteen_bit_version_of_one_picture_agree_to_the_eight_bit_quantisation() {
        let mut eight = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(4, 4);
        let mut sixteen = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(4, 4);

        for y in 0..4u32 {
            for x in 0..4u32 {
                // One picture at two depths: the 16-bit version carries a fraction of a level that the 8-bit file
                // cannot hold, which is exactly the precision the conversion must not throw away.
                let level = (y * 4 + x) * 16;
                eight.put_pixel(x, y, Rgb([level as u8, 255 - level as u8, 128]));
                sixteen.put_pixel(
                    x,
                    y,
                    Rgb([(level * 257 + 120) as u16, ((255 - level) * 257) as u16, 128 * 257 + 120]),
                );
            }
        }

        let eight = DynamicImage::ImageRgb8(eight);
        let sixteen = DynamicImage::ImageRgb16(sixteen);

        let mut from_eight = vec![0.0; 3 * 16];
        let mut from_sixteen = vec![0.0; 3 * 16];
        image_to_chw(&mut from_eight, &Sampler::new(&eight), whole(&eight), 4, Normalisation::Unit).unwrap();
        image_to_chw(&mut from_sixteen, &Sampler::new(&sixteen), whole(&sixteen), 4, Normalisation::Unit).unwrap();

        // One 8-bit level is 1/255 of the range, and the two agree within it — but not exactly, which is the point:
        // the 16-bit values are the ones its file holds rather than the 256 an 8-bit file is limited to.
        let quantisation = 1.0 / 255.0;
        let mut identical = true;

        for (a, b) in from_eight.iter().zip(&from_sixteen) {
            assert!((a - b).abs() <= quantisation, "{a} and {b} disagree by more than one 8-bit level");
            identical &= (a - b).abs() < f32::EPSILON;
        }

        assert!(!identical, "the 16-bit source was narrowed to the levels an 8-bit file would have held");
    }

    /// The red plane of a whole-image conversion at `extent`, as `[0, 1]` fractions, so a row can be read off it.
    fn padded_red(source: &DynamicImage, extent: (u32, u32), norm: Normalisation) -> Vec<f32> {
        let (width, height) = extent;
        let mut tensor = vec![0.0; 3 * (width as usize) * (height as usize)];

        padded_to_chw(&mut tensor, &Sampler::new(source), (source.width(), source.height()), extent, norm).unwrap();
        tensor[..(width as usize) * (height as usize)].to_vec()
    }

    #[test]
    fn a_whole_image_whose_extent_is_its_own_dimensions_reads_back_unchanged() {
        // The identity case, and the one the pipeline hits whenever the target size is already aligned and at least
        // one region: no mirroring at all, every plane the picture itself.
        let source = gradient(5, 3);
        let mut tensor = vec![0.0; 3 * 15];

        padded_to_chw(&mut tensor, &Sampler::new(&source), (5, 3), (5, 3), Normalisation::Unit).unwrap();

        let sampler = Sampler::new(&source);
        for y in 0..3u32 {
            for x in 0..5u32 {
                let [r, g, b] = sampler.rgb(x, y);
                let index = (y * 5 + x) as usize;

                assert_eq!(tensor[index], f32::from(r) / f32::from(u16::MAX), "the red plane differs at ({x}, {y})");
                assert_eq!(tensor[15 + index], f32::from(g) / f32::from(u16::MAX));
                assert_eq!(tensor[30 + index], f32::from(b) / f32::from(u16::MAX));
            }
        }
    }

    #[test]
    fn the_extension_is_the_mirror_of_the_pixels_beside_it_on_each_axis_independently() {
        // A 6x4 picture read at a 10x6 extent. Along a row the four added columns mirror back across the last
        // pixel — 5, 4, 3, 2 — and the two added rows do the same down the column, each axis answering for itself
        // rather than the corner being read from some other rule.
        let source = gradient(6, 4);
        let sampler = Sampler::new(&source);
        let red = padded_red(&source, (10, 6), Normalisation::Unit);

        let at = |x: u32, y: u32| f32::from(sampler.rgb(x, y)[0]) / f32::from(u16::MAX);

        // The added columns of the first row.
        for (offset, mirrored) in [(6, 5), (7, 4), (8, 3), (9, 2)] {
            assert_eq!(red[offset], at(mirrored, 0), "column {offset} is not the mirror of column {mirrored}");
        }

        // The added rows of the first column.
        for (offset, mirrored) in [(4, 3), (5, 2)] {
            assert_eq!(red[offset * 10], at(0, mirrored), "row {offset} is not the mirror of row {mirrored}");
        }

        // And the corner both extensions reach, which is the two mirrors composed rather than a third rule.
        assert_eq!(red[5 * 10 + 9], at(2, 2), "the corner is not the mirror on both axes");
    }

    #[test]
    fn a_whole_image_buffer_of_the_wrong_length_is_refused_naming_both_lengths() {
        let source = gradient(4, 4);

        for length in [0, 3 * 24 - 1, 3 * 24 + 1] {
            let mut tensor = vec![f32::NAN; length];
            let error =
                padded_to_chw(&mut tensor, &Sampler::new(&source), (4, 4), (6, 4), Normalisation::Unit).unwrap_err();

            assert_eq!(error.expected, 3 * 24, "the error did not name the extent's own length");
            assert_eq!(error.actual, length, "the error did not name the length that was supplied");
            assert_eq!((error.width, error.height), (6, 4));
            assert!(tensor.iter().all(|value| value.is_nan()), "a refused conversion wrote into the buffer");
        }
    }

    #[test]
    fn the_signed_normalisation_lands_a_whole_padded_image_inside_the_range_the_model_takes() {
        // The range the diffusion upscaler's driver feeds its model through this conversion. Both endpoints appear in
        // the picture, so the check is that they land exactly on -1 and 1 rather than merely inside them — and that
        // the mirrored extension is normalised by the same rule rather than left at a `[0, 1]` fraction.
        let mut buffer = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(3, 2);
        buffer.put_pixel(0, 0, Rgb([0, 0, 0]));
        buffer.put_pixel(1, 0, Rgb([32768, 32768, 32768]));
        buffer.put_pixel(2, 0, Rgb([65535, 65535, 65535]));
        buffer.put_pixel(0, 1, Rgb([65535, 0, 32768]));
        buffer.put_pixel(1, 1, Rgb([0, 65535, 0]));
        buffer.put_pixel(2, 1, Rgb([32768, 32768, 65535]));

        let source = DynamicImage::ImageRgb16(buffer);
        let mut tensor = vec![0.0; 3 * 5 * 4];
        padded_to_chw(&mut tensor, &Sampler::new(&source), (3, 2), (5, 4), Normalisation::Signed).unwrap();

        assert!(tensor.iter().all(|value| (-1.0..=1.0).contains(value)), "a value landed outside [-1, 1]");
        assert_eq!(tensor[0], -1.0, "black did not land on -1");
        assert_eq!(tensor[2], 1.0, "white did not land on 1");
        // The mirrored columns carry the same range: column 3 mirrors column 2, column 4 mirrors column 1.
        assert_eq!(tensor[3], 1.0, "the mirrored column was not normalised");
        assert!((tensor[4] - 0.0).abs() < 1e-4, "the mirrored mid-grey is at {}", tensor[4]);
    }

    /// A 2x1 model output: a mid-grey pixel beside a near-white one, in the planar layout a model returns.
    const OUTPUT: [f32; 6] = [0.5, 0.9, 0.5, 0.9, 0.5, 0.9];

    #[test]
    fn one_model_output_decoded_at_both_depths_describes_the_same_picture() {
        let mut eight = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(2, 1);
        let mut sixteen = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(2, 1);

        chw_to_image(&mut eight, &OUTPUT, Normalisation::Unit).unwrap();
        chw_to_image(&mut sixteen, &OUTPUT, Normalisation::Unit).unwrap();

        for x in 0..2 {
            for channel in 0..3 {
                let a = eight.get_pixel(x, 0).0[channel].to_unit();
                let b = sixteen.get_pixel(x, 0).0[channel].to_unit();

                // Within one 8-bit level: the same picture, at the two depths that were asked for.
                assert!((a - b).abs() <= 1.0 / 255.0, "the two depths disagree at ({x}, {channel}): {a} and {b}");
            }
        }

        assert_eq!(eight.get_pixel(0, 0).0[0], 128);
        assert_eq!(sixteen.get_pixel(0, 0).0[0], 32768);

        // The signed normalisation is reversed as well as the unit one, and reaches the same pixels from the values
        // the model would have produced under it.
        let signed = OUTPUT.map(|value| value.mul_add(2.0, -1.0));
        let mut from_signed = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(2, 1);
        chw_to_image(&mut from_signed, &signed, Normalisation::Signed).unwrap();

        for x in 0..2 {
            let expected = eight.get_pixel(x, 0).0;
            let actual = from_signed.get_pixel(x, 0).0;
            for channel in 0..3 {
                assert!(expected[channel].abs_diff(actual[channel]) <= 1, "the two normalisations disagree at {x}");
            }
        }
    }

    #[test]
    fn a_value_outside_the_range_clamps_to_the_channel_rather_than_wrapping() {
        // A model that diverges returns magnitudes far outside what it normalises over. Wrapping would turn the
        // brightest of them into black, which is the artefact this clamp exists to prevent.
        let output = [2.0, -1.0, 1000.0, -1000.0, f32::INFINITY, f32::NEG_INFINITY];

        let mut eight = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(2, 1);
        chw_to_image(&mut eight, &output, Normalisation::Unit).unwrap();

        assert_eq!(eight.get_pixel(0, 0).0, [255, 255, 255], "a value above the range did not clamp to the maximum");
        assert_eq!(eight.get_pixel(1, 0).0, [0, 0, 0], "a value below the range did not clamp to zero");

        let mut sixteen = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(2, 1);
        chw_to_image(&mut sixteen, &output, Normalisation::Unit).unwrap();

        assert_eq!(sixteen.get_pixel(0, 0).0, [65535, 65535, 65535]);
        assert_eq!(sixteen.get_pixel(1, 0).0, [0, 0, 0]);
    }

    #[test]
    fn the_result_carries_colour_and_no_alpha_channel() {
        assert_eq!(<Rgb<u8> as image::Pixel>::CHANNEL_COUNT, 3);
        assert_eq!(<Rgb<u16> as image::Pixel>::CHANNEL_COUNT, 3);

        let eight = u8::into_dynamic(ImageBuffer::from_pixel(1, 1, Rgb([1, 2, 3])));
        let sixteen = u16::into_dynamic(ImageBuffer::from_pixel(1, 1, Rgb([1, 2, 3])));

        assert!(!eight.color().has_alpha(), "the 8-bit result claims an opacity the model never returned");
        assert!(!sixteen.color().has_alpha(), "the 16-bit result claims an opacity the model never returned");
        assert_eq!(eight.color().channel_count(), 3);
        assert_eq!(sixteen.color().bits_per_pixel(), 48);
    }

    #[test]
    fn a_decode_into_a_buffer_the_output_does_not_fill_is_refused() {
        let mut dest = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(2, 2);
        let error = chw_to_image(&mut dest, &OUTPUT, Normalisation::Unit).unwrap_err();

        assert_eq!(error.expected, 12);
        assert_eq!(error.actual, 6);
        assert!(dest.pixels().all(|pixel| pixel.0 == [0, 0, 0]), "a refused decode wrote into the buffer");
    }

    /// A tensor of `extent` whose every plane carries a value derived from the position, so a decode that read the
    /// wrong stride lands on a different number rather than on a plausible one.
    fn strided(extent: (u32, u32)) -> Vec<f32> {
        let (width, height) = extent;
        let plane = (width as usize) * (height as usize);

        (0..3 * plane)
            .map(|index| {
                let within = index % plane;
                let (row, column) = (within / width as usize, within % width as usize);

                // Distinct per channel, per row and per column, and inside `[0, 1]` so the unit normalisation is the
                // identity and the expected channel value is readable off the arithmetic.
                ((index / plane) as f32).mul_add(0.03, (row as f32).mul_add(0.07, column as f32 * 0.005))
            })
            .collect()
    }

    #[test]
    fn a_cropped_decode_reads_the_region_a_full_decode_of_that_region_would_have() {
        // The property the whole gain map rests on: the top-left `sw x sh` of a padded square has to be exactly the
        // picture, at the picture's own stride. A decode that walked the destination's width instead of the
        // tensor's would still fill every pixel and still produce an image — sheared, by one column per row.
        let extent = (7, 5);
        let tensor = strided(extent);

        for region in [(7, 5), (4, 5), (7, 2), (4, 3), (1, 1)] {
            let mut cropped = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(region.0, region.1);
            padded_chw_to_image(&mut cropped, &tensor, extent, Normalisation::Unit)
                .expect("a region inside the tensor decodes");

            // The same region, read out of a whole decode of the tensor — which is what the rejected
            // decode-then-crop arrangement would have produced, and the only independent statement of what the
            // right answer is.
            let mut whole = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(extent.0, extent.1);
            chw_to_image(&mut whole, &tensor, Normalisation::Unit).expect("the whole tensor decodes");

            for (x, y, pixel) in cropped.enumerate_pixels() {
                assert_eq!(pixel, whole.get_pixel(x, y), "{region:?} disagreed with a full decode at ({x}, {y})");
            }
        }
    }

    #[test]
    fn a_region_larger_than_the_tensor_is_refused_rather_than_partly_written() {
        // Both axes, separately: a region too wide would read off the end of the red plane and into the green one,
        // which is a picture rather than a panic, and a region too tall would run off the end of the buffer.
        let extent = (4, 4);
        let tensor = strided(extent);

        for region in [(5, 4), (4, 5), (8, 8)] {
            let mut dest = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(region.0, region.1);
            let error =
                padded_chw_to_image(&mut dest, &tensor, extent, Normalisation::Unit).expect_err("{region:?} decoded");

            assert_eq!(error.expected, 3 * (region.0 as usize) * (region.1 as usize));
            assert_eq!(error.actual, tensor.len());
            assert!(dest.pixels().all(|pixel| pixel.0 == [0, 0, 0]), "a refused decode wrote into the buffer");
        }
    }

    #[test]
    fn a_cropped_decode_of_a_tensor_the_extent_does_not_describe_is_refused() {
        // The other half of the shape check, and the one the region cannot cause: the tensor and the extent it was
        // declared at disagree, which is the caller's scratch and its plan having drifted apart.
        let mut dest = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(2, 2);
        let error = padded_chw_to_image(&mut dest, &OUTPUT, (4, 4), Normalisation::Unit).unwrap_err();

        assert_eq!(error.expected, 48);
        assert_eq!(error.actual, 6);
        assert!(dest.pixels().all(|pixel| pixel.0 == [0, 0, 0]), "a refused decode wrote into the buffer");
    }

    #[test]
    fn a_half_transparent_pixel_samples_premultiplied_against_black() {
        let mut buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(3, 1);
        buffer.put_pixel(0, 0, Rgba([200, 100, 50, 255]));
        buffer.put_pixel(1, 0, Rgba([200, 100, 50, 128]));
        buffer.put_pixel(2, 0, Rgba([200, 100, 50, 0]));

        let source = DynamicImage::ImageRgba8(buffer);
        let sampler = Sampler::new(&source);

        assert_eq!(sampler.rgb(0, 0), [widen(200), widen(100), widen(50)], "an opaque pixel was altered");

        // Half transparent is half the value, which is the composite against black the reference also performs.
        let half = sampler.rgb(1, 0);
        for (channel, opaque) in half.iter().zip(sampler.rgb(0, 0)) {
            let expected = u32::from(opaque) * 128 / 255;
            assert!(u32::from(*channel).abs_diff(expected) <= 1, "{channel} is not {opaque} against black at 50%");
        }

        assert_eq!(sampler.rgb(2, 0), [0, 0, 0], "a fully transparent pixel did not composite to black");
    }

    #[test]
    fn the_alpha_comes_back_beside_the_colour_every_model_is_fed() {
        // Every shape a source reaches the sampler in: the two borrowed 8-bit variants, the two borrowed 16-bit ones,
        // and a greyscale-with-alpha source converted up into the owned buffer.
        let mut rgb8 = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(1, 1);
        rgb8.put_pixel(0, 0, Rgb([200, 100, 50]));

        let mut rgba8 = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(2, 1);
        rgba8.put_pixel(0, 0, Rgba([200, 100, 50, 255]));
        rgba8.put_pixel(1, 0, Rgba([200, 100, 50, 64]));

        let mut rgb16 = ImageBuffer::<Rgb<u16>, Vec<u16>>::new(1, 1);
        rgb16.put_pixel(0, 0, Rgb([0x1234, 0x5678, 0x9abc]));

        let mut rgba16 = ImageBuffer::<Rgba<u16>, Vec<u16>>::new(2, 1);
        rgba16.put_pixel(0, 0, Rgba([0x1234, 0x5678, 0x9abc, u16::MAX]));
        rgba16.put_pixel(1, 0, Rgba([0x1234, 0x5678, 0x9abc, 0x1001]));

        let mut luma_alpha = ImageBuffer::<image::LumaA<u8>, Vec<u8>>::new(1, 1);
        luma_alpha.put_pixel(0, 0, image::LumaA([90, 32]));

        let cases = [
            ("8-bit, no alpha", DynamicImage::ImageRgb8(rgb8), vec![u16::MAX]),
            ("8-bit, alpha", DynamicImage::ImageRgba8(rgba8), vec![u16::MAX, widen(64)]),
            ("16-bit, no alpha", DynamicImage::ImageRgb16(rgb16), vec![u16::MAX]),
            ("16-bit, alpha", DynamicImage::ImageRgba16(rgba16), vec![u16::MAX, 0x1001]),
            ("converted, alpha", DynamicImage::ImageLumaA8(luma_alpha), vec![widen(32)]),
        ];

        for (name, source, alphas) in cases {
            let sampler = Sampler::new(&source);

            for (x, alpha) in alphas.into_iter().enumerate() {
                let x = x as u32;
                let (colour, read) = sampler.rgb_and_alpha(x, 0);

                assert_eq!(read, alpha, "{name}: the alpha at {x} is not the source's own");
                assert_eq!(colour, sampler.rgb(x, 0), "{name}: the colour beside the alpha is not the one `rgb` gives");
            }
        }
    }

    /// [`extend_to_chw`] as it was written before it read by rows: one [`Sampler::rgb`] per pixel.
    fn extend_to_chw_per_pixel(
        sampler: &Sampler<'_>,
        origin: (u32, u32),
        source: (u32, u32),
        extent: (u32, u32),
        norm: Normalisation,
    ) -> Vec<f32> {
        let mut dest = vec![0.0_f32; 3 * (extent.0 * extent.1) as usize];
        extend_to_chw_reference(&mut dest, sampler, origin, source, extent, norm);

        dest
    }

    fn extend_to_chw_reference(
        dest: &mut [f32],
        sampler: &Sampler<'_>,
        origin: (u32, u32),
        source: (u32, u32),
        extent: (u32, u32),
        norm: Normalisation,
    ) {
        let plane = (extent.0 * extent.1) as usize;

        for row in 0..extent.1 {
            let y = origin.1 + pad::source_offset(row, source.1);

            for column in 0..extent.0 {
                let x = origin.0 + pad::source_offset(column, source.0);
                let index = (row * extent.0 + column) as usize;
                let [r, g, b] = sampler.rgb(x, y);

                dest[index] = norm.encode(r);
                dest[plane + index] = norm.encode(g);
                dest[2 * plane + index] = norm.encode(b);
            }
        }
    }

    /// Every variant a source can reach the sampler as, at an odd size, with every channel — alpha included — spread
    /// over its whole range, so partial transparency is everywhere rather than at a hand-picked pixel.
    fn every_variant(width: u32, height: u32) -> Vec<(&'static str, DynamicImage)> {
        let mut state = 0x2545_f491_u32;
        let wide = DynamicImage::ImageRgba16(ImageBuffer::from_fn(width, height, |_, _| {
            Rgba([(); 4].map(|()| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 16) as u16
            }))
        }));

        vec![
            ("Rgb8", DynamicImage::ImageRgb8(wide.to_rgb8())),
            ("Rgba8", DynamicImage::ImageRgba8(wide.to_rgba8())),
            ("Rgb16", DynamicImage::ImageRgb16(wide.to_rgb16())),
            ("Rgba16", wide.clone()),
            ("Luma8", DynamicImage::ImageLuma8(wide.to_luma8())),
            ("LumaA8", DynamicImage::ImageLumaA8(wide.to_luma_alpha8())),
            ("Luma16", DynamicImage::ImageLuma16(wide.to_luma16())),
            ("LumaA16", DynamicImage::ImageLumaA16(wide.to_luma_alpha16())),
            ("Rgb32F", DynamicImage::ImageRgb32F(wide.to_rgb32f())),
            ("Rgba32F", DynamicImage::ImageRgba32F(wide.to_rgba32f())),
        ]
    }

    #[test]
    fn reading_by_rows_is_bit_identical_to_reading_pixel_by_pixel_for_every_variant() {
        let (width, height) = (37, 23);

        for (name, source) in every_variant(width, height) {
            let sampler = Sampler::new(&source);

            // The row accessors against `rgb`, contiguous and gathered through a mirrored column table.
            let mut row = vec![[0_u16; 3]; width as usize];
            let columns: Vec<u32> = (0..width + 9).map(|column| pad::source_offset(column, width)).collect();
            let mut gathered = vec![[0_u16; 3]; columns.len()];

            for y in 0..height {
                sampler.row(y, &mut row);
                sampler.row_at(y, &columns, &mut gathered);

                let expected: Vec<[u16; 3]> = (0..width).map(|x| sampler.rgb(x, y)).collect();
                assert_eq!(row, expected, "{name}: row {y} differs from its pixels");

                let expected: Vec<[u16; 3]> = columns.iter().map(|&x| sampler.rgb(x, y)).collect();
                assert_eq!(gathered, expected, "{name}: gathered row {y} differs from its pixels");
            }

            // The two tensor conversions, under both normalisations, compared as bits rather than as floats.
            for norm in [Normalisation::Unit, Normalisation::Signed] {
                let extent = (width + 8, height + 5);
                let mut padded = vec![0.0_f32; 3 * (extent.0 * extent.1) as usize];
                padded_to_chw(&mut padded, &sampler, (width, height), extent, norm).expect("an exact buffer");
                let expected = extend_to_chw_per_pixel(&sampler, (0, 0), (width, height), extent, norm);
                assert!(
                    padded.iter().map(|v| v.to_bits()).eq(expected.iter().map(|v| v.to_bits())),
                    "{name}: the padded tensor differs under {norm:?}"
                );

                let region = Tile { x: 5, y: 3, width: 11, height: 7 };
                let mut tile = vec![0.0_f32; 3 * 13 * 13];
                image_to_chw(&mut tile, &sampler, region, 13, norm).expect("an exact buffer");
                let expected = extend_to_chw_per_pixel(&sampler, (5, 3), (11, 7), (13, 13), norm);
                assert!(
                    tile.iter().map(|v| v.to_bits()).eq(expected.iter().map(|v| v.to_bits())),
                    "{name}: the tile tensor differs under {norm:?}"
                );
            }

            // And the flatten at both depths.
            let fast: ImageBuffer<Rgb<u8>, Vec<u8>> = flattened(&sampler, width, height);
            let slow = flattened_converting::<u8>(&sampler, width, height);
            assert!(fast == slow, "{name}: the 8-bit flatten differs");

            let fast: ImageBuffer<Rgb<u16>, Vec<u16>> = flattened(&sampler, width, height);
            let slow = flattened_converting::<u16>(&sampler, width, height);
            assert!(fast == slow, "{name}: the 16-bit flatten differs");
        }
    }

    // Timing rather than a check: `cargo test --release -p imaging -- --ignored --nocapture row_reads_against`.
    #[test]
    #[ignore = "a measurement, run by hand in release"]
    fn row_reads_against_per_pixel_reads_at_24_megapixels() {
        use std::time::Instant;

        let (width, height) = (6000, 4000);

        for (name, source) in every_variant(width, height).into_iter().take(4) {
            let sampler = Sampler::new(&source);
            let extent = (width + 1, height + 1);
            let mut dest = vec![0.0_f32; 3 * (extent.0 * extent.1) as usize];

            // Both into a buffer already allocated and faulted in, so neither is charged for the other's allocation.
            let old = extend_to_chw_per_pixel(&sampler, (0, 0), (width, height), extent, Normalisation::Unit);
            padded_to_chw(&mut dest, &sampler, (width, height), extent, Normalisation::Unit).expect("exact");

            let started = Instant::now();
            extend_to_chw_reference(&mut dest, &sampler, (0, 0), (width, height), extent, Normalisation::Unit);
            let per_pixel = started.elapsed();
            let started = Instant::now();
            padded_to_chw(&mut dest, &sampler, (width, height), extent, Normalisation::Unit).expect("exact");
            let by_rows = started.elapsed();
            assert!(old == dest, "{name}: the tensors differ");

            let started = Instant::now();
            let old = flattened_converting::<u16>(&sampler, width, height);
            let flatten_per_pixel = started.elapsed();
            let started = Instant::now();
            let new = flattened::<u16>(&sampler, width, height);
            let flatten_by_rows = started.elapsed();
            assert!(old == new, "{name}: the flattens differ");

            println!(
                "{name:>6}: to_chw {per_pixel:>10.2?} -> {by_rows:>10.2?}   flattened<u16> {flatten_per_pixel:>10.2?} -> \
                 {flatten_by_rows:>10.2?}"
            );
        }
    }
}
