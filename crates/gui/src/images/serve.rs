//! How an opened image's pixels reach the window: what a request may ask for, where the pixels come from,
//! and what crosses back.
//!
//! A request names an admitted file's *identity*, never a path, so the window can only reach what the user
//! opened.

// Pixels are served over a custom URI scheme rather than base64 over IPC — which is what the reference
// implementation does, at the cost of a Blob/object-URL/LRU/revoke apparatus in its frontend.

use std::borrow::Cow;
use std::path::PathBuf;

use image::DynamicImage;
use image::imageops::FilterType;
use tauri::{AppHandle, Manager};

use super::crop::Crop;
use super::decoded::Decoded;
use super::files::Opened;
use super::renditions::{Claim, Rendition, Renditions};
use crate::task::spawn_blocking;

// Must stay in sync with `tauri.conf.json`'s `img-src`, which needs both platform forms of it.
/// The URI scheme an image's pixels are served over. Registered in `src/lib.rs`.
pub(crate) const SCHEME: &str = "opai";

/// Length of an XXH3-64 identity as [`opai`] spells it: 16 lowercase hex characters.
pub(super) const IDENTITY_LENGTH: usize = 16;

// ── What a request asks for ───────────────────────────────────────────────────────────────────────────────────────

/// What a request for an image's pixels asked for: an identity, a bound, and the framing to apply before
/// the bound.
// `Hash` because this is what `Renditions` is keyed on. `Crop` is `Eq + Hash` for exactly this reason; see
// `Crop` for why its angle is an integer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Asked {
    // All three, and deliberately nothing else. The crop is carried in the request rather than held anywhere,
    // which is what keeps this a key that cannot go stale — the alternative, a `set_crop` command holding it
    // on this side, would leave every rendition already produced under a key describing a framing that had
    // changed.
    /// Which admitted file. Never a location — see the module documentation.
    pub(super) identity: String,
    /// The cap on the picture's longest edge, or zero for the picture's own dimensions.
    pub(super) bound: u32,
    /// How the photograph is framed, or `None` for the whole of it.
    pub(super) crop: Option<Crop>,
}

/// What `<path>?size=<n>&crop=<c>` asked for, or `None` if it asked for nothing this application can serve.
///
/// Both platform forms of the URL (`opai://localhost/...` and `http://opai.localhost/...`) have the same
/// path and query, and are read the same way.
///
/// The path must be exactly [`IDENTITY_LENGTH`] lowercase hex characters, so any real filesystem path is
/// refused before the registry is consulted. `size` absent or `0` means the image's own dimensions, and
/// `crop` absent means the whole photograph. Either one present and malformed is refused outright rather
/// than read as absent.
fn parse(uri: &tauri::http::Uri) -> Option<Asked> {
    // The path and the query are all that is read, which is what lets `convertFileSrc` be the only thing that knows
    // the difference between the platform forms.
    let identity = uri.path().trim_start_matches('/');

    // Lowercase hex, spelled as one predicate rather than an is_ascii_hexdigit + uppercase-rejection pair.
    if identity.len() != IDENTITY_LENGTH
        || !identity.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return None;
    }

    // Absent or `0` is the reference's convention. Malformed is refused rather than read as "no bound", so a
    // malformed URL can't silently serve a full-resolution photograph where a thumbnail was expected.
    let bound = match uri.query().and_then(size_of) {
        // No `size` at all, which is the image's own dimensions.
        None => 0,
        Some(bound) => bound?,
    };

    // As strict, for the same reason turned the other way round: read as "no crop", a malformed one would serve the
    // whole photograph where a framing was asked for — a wrong answer that looks like a right one.
    let crop = match uri.query().and_then(crop_of) {
        // No `crop` at all, which is the whole photograph.
        None => None,
        Some(crop) => Some(crop?),
    };

    Some(Asked { identity: identity.to_string(), bound, crop })
}

/// The `size` parameter's value, if the query carries one: `None` for absent, `Some(None)` for present and
/// malformed.
///
/// Hand-rolled rather than via a query-string crate, since the only query this application ever builds is
/// this one parameter.
fn size_of(query: &str) -> Option<Option<u32>> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("size="))
        .map(|value| if value.is_empty() { None } else { value.parse().ok() })
}

/// The `crop` parameter's value, if the query carries one: `None` for absent, `Some(None)` for present and
/// malformed.
///
/// `<left>,<top>,<width>,<height>,<millidegrees>,<flips>`, where `<flips>` is one of `-`, `h`, `v` or `hv`
/// — six positional fields, no percent-encoding and nothing optional. Positional rather than JSON in the
/// query, which would let one encoder serve both doors: it is unreadable in a log line, and percent-encoding
/// it would make the cache key sensitive to the order the encoder happened to write the fields in. See
/// design.md D4.
///
/// The whole of it has to parse. A seventh field, a missing one, an unknown flip spelling or a rectangle
/// with no area are each the refusal above rather than a field quietly defaulted, which is what makes the
/// frontend's spelling of this and this parser one grammar rather than two that overlap.
fn crop_of(query: &str) -> Option<Option<Crop>> {
    query.split('&').find_map(|pair| pair.strip_prefix("crop=")).map(|value| {
        let mut fields = value.split(',');
        let left = fields.next()?.parse().ok()?;
        let top = fields.next()?.parse().ok()?;
        let width = fields.next()?.parse().ok()?;
        let height = fields.next()?.parse().ok()?;
        let millidegrees = fields.next()?.parse().ok()?;

        let (flip_horizontal, flip_vertical) = match fields.next()? {
            "-" => (false, false),
            "h" => (true, false),
            "v" => (false, true),
            "hv" => (true, true),
            _ => return None,
        };

        // A seventh field is a spelling this application does not produce, and reading the first six of it
        // would be serving a framing nobody asked for.
        if fields.next().is_some() {
            return None;
        }

        Crop::new(left, top, width, height, millidegrees, flip_horizontal, flip_vertical)
    })
}

// ── Where the pixels come from ────────────────────────────────────────────────────────────────────────────────────

/// Why a request's pixels could not be produced.
///
/// Kept as three distinct cases, each with its own status code in [`respond`]: an identity this
/// application never admitted is a bug in the interface; an admitted file that won't open is a photograph
/// on a drive that's been ejected; a displaced result is an address the window kept after this
/// application stopped holding what it named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// No admitted file and no run carries this identity — including a request naming a location rather
    /// than an identity.
    Unknown,
    /// An admitted file that could not be read or decoded.
    Unreadable,
    /// An enhanced result this application produced and no longer holds — a later run having displaced it,
    /// or the image it was made from having been closed.
    Displaced,
}

/// Where the pixels for an identity are to be found: an admitted file not yet decoded, or an enhanced
/// result already held (shared, not copied — see design.md D4).
enum Pixels {
    /// An admitted file, not yet decoded: its identity, where it is, and the cache to decode it through.
    File(String, PathBuf, Decoded),
    /// The enhanced result this application is holding.
    Held(opai::Picture),
}

/// Resolves `identity` to [`Pixels`], or the [`Refusal`] for why not.
///
/// Checks the run slot before [`Opened`]: it's the cheaper lookup, and an enhanced identity can never
/// collide with an admitted file's, since one describes bytes and the other is composed from those plus
/// every operation applied.
fn locate(opened: &Opened, runs: &crate::enhance::Runs, identity: &str) -> Result<Pixels, Refusal> {
    match runs.resolve(identity) {
        Some(crate::enhance::Resident::Picture(picture)) => return Ok(Pixels::Held(picture)),
        // Produced by this application and no longer held, a later run having displaced it or its image
        // having been closed — refused rather than reported as unknown, so a stale address reads as "this is
        // gone" rather than "you asked for something that never was".
        Some(crate::enhance::Resident::Displaced) => return Err(Refusal::Displaced),
        None => {}
    }

    opened
        .resolve(identity)
        .map(|path| Pixels::File(identity.to_string(), path, opened.decoded().clone()))
        .ok_or(Refusal::Unknown)
}

/// Decodes the file behind `pixels`, or hands back a held result unchanged.
///
/// The only place [`Refusal::Unreadable`] is produced, since a held result was never on disk to begin
/// with.
///
/// `bound` is the rendition's: only a full-size one — what the canvas asks for — keeps what it decoded. A bounded
/// one, a drawer thumbnail among them, is served from a decode already held but keeps nothing, so a drawer full of
/// thumbnails does not evict the canvas's photograph from the cache.
fn decode(pixels: Pixels, bound: u32) -> Result<opai::Picture, Refusal> {
    match pixels {
        Pixels::Held(picture) => Ok(picture),
        // Not logged: `opai` records the failure itself, naming the file and the reason.
        //
        // Through the decoded cache, so the canvas and every run that follows share one decode.
        Pixels::File(identity, path, decoded) => {
            let loaded = if bound == 0 {
                decoded.load_blocking(&identity, &path)
            } else {
                decoded.load_passing_blocking(&identity, &path)
            };

            loaded.map_err(|_| Refusal::Unreadable)
        }
    }
}

// ── Producing the pixels asked for ────────────────────────────────────────────────────────────────────────────────

/// JPEG quality previews are encoded at: 90, matching the reference's `previewJpegQuality`.
const PREVIEW_QUALITY: u8 = 90;

/// How far the fast prefilter in [`bounded`] scales down before Lanczos3 takes over.
///
/// 3, chosen by measurement: a two-stage downscale of a 6000x3377 photo to a 384-pixel bound scores 44.2
/// dB PSNR against a straight Lanczos3 pass, at roughly a third of the cost. Two is faster but visibly
/// softer; four buys a decibel nobody asked for.
const PREFILTER_FACTOR: u32 = 3;

/// Scales `pixels` down so its longest edge is at most `bound`, preserving the aspect ratio. Behind every
/// thumbnail, and behind the preview canvas, which draws at the size of its own element rather than the
/// photograph's.
///
/// Lives here rather than in `opai::image` because scaling a picture to fit something that draws it is a
/// question about an interface, not about reading a file — the reference draws the same line, with no
/// resize in its core library.
///
/// # Caps, never enlarges
///
/// A `bound` at or above the picture's longest edge (including `0`) returns the picture unchanged,
/// **borrowed rather than cloned**: this is the common path — the canvas usually asks for no bound at all
/// — and a `DynamicImage` clone can be tens to hundreds of megabytes for no change in pixels. This is a
/// deliberate divergence from the reference's `imaging.Resize`, which treats the bound as a target and
/// would enlarge a smaller picture.
///
/// # Lanczos3, over a prefiltered picture
///
/// Lanczos3 matches the reference's filter and is the right choice for a downscale, but it's also the
/// most expensive filter the `image` crate offers, and single-threaded — measured at about 160ms to
/// filter a 6000x3377 JPEG down to 384px, against about 70ms to decode it. A large reduction is therefore
/// prefiltered first with the crate's fast integer-averaging `thumbnail`, down to [`PREFILTER_FACTOR`]
/// times the bound, so Lanczos3 then runs over a picture about nine times smaller (cutting that 160ms to
/// roughly 60). A smaller reduction skips the prefilter, since there's nothing to save and the extra step
/// would cost quality for it.
///
/// Colour type is preserved by both stages, so a 16-bit developed RAW stays 16-bit and an alpha channel
/// survives — which is what lets [`render`] branch on colour type afterwards and be right.
fn bounded(pixels: &DynamicImage, bound: u32) -> Cow<'_, DynamicImage> {
    let longest = pixels.width().max(pixels.height());

    if bound == 0 || bound >= longest {
        // Borrowed: the common path, and a clone here would double peak memory for the length of the encode
        // that follows, for no benefit since the caller only reads the result.
        return Cow::Borrowed(pixels);
    }

    // Saturating, so a bound near `u32::MAX` can't wrap and prefilter a picture down to nothing.
    let prefilter = bound.saturating_mul(PREFILTER_FACTOR);

    // `thumbnail` and `resize` both fit inside a square box while preserving aspect ratio, so the box bounds
    // whichever edge is longer; both share `resize_dimensions`, so the two stages round the same way.
    let source = if prefilter < longest {
        Cow::Owned(pixels.thumbnail(prefilter, prefilter))
    } else {
        Cow::Borrowed(pixels)
    };

    Cow::Owned(source.resize(bound, bound, FilterType::Lanczos3))
}

/// The photograph reduced so `crop` will frame it at about `bound`, and the crop scaled to match.
///
/// The factor is `bound / max(crop.width, crop.height)`, **capped at one** so nothing is ever enlarged —
/// the same rule [`bounded`] states for a bound above the picture's own longest edge, reached from the
/// framing's dimensions rather than the photograph's because the framing is what is being served.
///
/// The reduction goes through [`bounded`] rather than a second downscale of this module's own, and the
/// crop is then scaled by the reduction **actually achieved** rather than by the one asked for: `bounded`
/// rounds, and scaling by the requested factor would put the rectangle a pixel off the framing the
/// unbounded request shows.
///
/// An unbounded request — what the canvas asks for — takes the borrowed path with the crop untouched, so
/// its dimensions are exactly the rectangle's. That is what lets the window size its box from the crop
/// before the pixels arrive. See design.md D5, D9.
fn reduced(pixels: &DynamicImage, bound: u32, crop: Crop) -> (Cow<'_, DynamicImage>, Crop) {
    let framing = crop.width().max(crop.height());

    // No bound at all, or one the framing already fits inside. `framing` cannot be zero — `Crop` refuses a
    // rectangle with no area — so there is no division to guard.
    if bound == 0 || bound >= framing {
        return (Cow::Borrowed(pixels), crop);
    }

    let factor = f64::from(bound) / f64::from(framing);
    let longest = pixels.width().max(pixels.height());

    // At least one pixel: a framing far smaller than the bound over a photograph far larger than the
    // framing can round the whole source away.
    let source_bound = ((f64::from(longest) * factor).round() as u32).max(1);
    let reduced = bounded(pixels, source_bound);

    // What the resize actually did, on the axis it is measured on. Zero-width pixels are not something a
    // decoder produces, and a source left unreduced answers exactly one.
    let achieved = f64::from(reduced.width()) / f64::from(pixels.width().max(1));

    (reduced, crop.scaled(achieved))
}

/// Produces the rendition an admitted file (or held result) was asked for, on the calling thread: load,
/// bound, encode.
///
/// # Always decoded and re-encoded, never passed through
///
/// Even an unbounded JPEG is decoded and re-encoded, because the canvas must show what the enhancement
/// pipeline will operate on, and that's what [`opai::image::load`] defines: it routes on content rather
/// than extension, and develops RAW to 16 bits per channel. A passthrough would show the webview's own
/// (possibly wrong) interpretation of the file instead.
///
/// # JPEG, unless the picture has alpha
///
/// A deliberate divergence from the reference, which encodes JPEG unconditionally and so composites
/// transparency away in previews while export keeps it. The check runs after the downscale and the
/// framing, both of which preserve colour type, so the paths can't disagree about whether there's an
/// alpha channel — and a turned photograph, which always carries one, is therefore served as PNG.
///
/// # A framing is reduced before it is turned, not after
///
/// A request carrying both a bound and a crop reduces the *source* so the framing will land near the
/// bound, scales the crop by the reduction, and only then flips, turns and cuts. Rotating sixty
/// megapixels to draw a 144-pixel miniature does the work and throws it away; this is what makes a
/// cropped thumbnail cost about what an uncropped one costs, and it is why the reference doesn't crop
/// bounded renditions at all. See [`reduced`] and design.md D5.
fn render(pixels: Pixels, asked: &Asked) -> Result<Rendition, Refusal> {
    let picture = decode(pixels, asked.bound)?;

    // Bound out here rather than inside the arm, so a framing that changed nothing can borrow the reduction
    // instead of copying it.
    let reduction = asked.crop.map(|crop| reduced(picture.pixels(), asked.bound, crop));

    let framed = match &reduction {
        None => Cow::Borrowed(picture.pixels()),
        Some((source, crop)) => super::crop::frame(source, *crop),
    };

    // Still applied to the framing, and usually a no-op borrow after `reduced` has already brought it to
    // about the bound: what this guarantees is that the longest edge is never *over* it, whatever the
    // reduction's rounding and the clamped rectangle left.
    let bounded = bounded(&framed, asked.bound);

    let (format, media_type, options) = if bounded.color().has_alpha() {
        (opai::ImageFormat::Png, "image/png", None)
    } else {
        (
            opai::ImageFormat::Jpeg,
            "image/jpeg",
            Some(opai::EncodeOptions::Jpeg { quality: PREVIEW_QUALITY }),
        )
    };

    // Nothing is wrong with the file — it loaded. Reported as unreadable all the same: from the window's side the
    // outcome is the same, and there's no third answer the spec asks for. Not logged: `opai` records the failure.
    let bytes = opai::image::encode_blocking(&bounded, format, options).map_err(|_| Refusal::Unreadable)?;

    Ok(Rendition { bytes: bytes.into(), media_type })
}

// ── Answering the window ──────────────────────────────────────────────────────────────────────────────────────────

/// Serves an opened image's pixels to the window. The handler registered under [`SCHEME`] in `src/lib.rs`.
///
/// Asynchronous, so a 100-megapixel scan's decode doesn't freeze the window: the response goes back
/// through `responder` whenever it's ready, and the webview waits the way it waits for any other URL.
///
/// # Cached both by the response header and by [`Renditions`]
///
/// A successful rendition carries a year-long immutable cache directive, safe because the
/// identity+bound+crop key can never come to mean different pixels. That's honoured by WebView2 on Windows, but macOS and
/// Linux serve `opai://` outside the webview's resource cache, so [`Renditions`] also holds what's already
/// been produced — see that type for what it costs without it.
///
/// # The two refusals are different status codes
///
/// **404** for an identity this application never admitted (including a request naming a location rather
/// than an identity), and **410** for an admitted file that will no longer open, or an enhanced result no
/// longer held — superseded by a later run, or released when its image was closed. Neither carries a cache
/// directive, since either might resolve on retry.
pub(crate) fn serve<R: tauri::Runtime>(
    context: tauri::UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let app = context.app_handle().clone();
    let asked = parse(request.uri());

    // spawn_blocking even for a cache hit: the state lives on AppHandle, so it can't be checked from the
    // window's own thread first without putting a map lookup there on every request.
    spawn_blocking(move || {
        let outcome = match &asked {
            Some(asked) => produce(&app, asked),
            // Unparseable request: names no admitted identity, so treated the same as an unknown one.
            None => Err(Refusal::Unknown),
        };

        responder.respond(respond(outcome));
    });
}

/// The rendition for a request: from [`Renditions`] if already produced, from [`render`] otherwise. Split
/// out from [`serve`] so it's testable without a webview.
///
/// # Entitlement is decided before the cache, not after it
///
/// [`locate`] runs first, deliberately: an enhanced result stops being served the moment a later run
/// displaces it, even though a rendition of it produced a moment earlier may still sit in [`Renditions`]
/// under a key (identity + bound + crop) that's technically still valid. Checking entitlement first is what makes
/// that displacement actually take effect. It costs an admitted file nothing, since its entitlement never
/// changes.
fn produce<R: tauri::Runtime>(app: &AppHandle<R>, asked: &Asked) -> Result<Rendition, Refusal> {
    let pixels = locate(&app.state::<Opened>(), &app.state::<crate::enhance::Runs>(), &asked.identity)?;

    // Bound rather than used inline: the guard below borrows from it, so the `State` has to outlive the match.
    let renditions = app.state::<Renditions>();

    let producing = match renditions.claim(asked) {
        Claim::Ready(rendition) => return Ok(rendition),
        Claim::Produce(producing) => producing,
    };

    // A refusal drops the guard on the way out, which releases the claim and wakes whatever was waiting on it.
    let rendition = render(pixels, asked)?;
    producing.keep(&rendition);

    Ok(rendition)
}

/// The response a rendition or a refusal crosses as. Split from [`serve`] so the status, media type and
/// cache directive can be asserted without a running application, a webview or a responder.
fn respond(outcome: Result<Rendition, Refusal>) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Response, StatusCode, header};

    // The success arm keeps a builder of its own because it alone carries a cache directive — which is the
    // one distinction between these responses worth reading. Every refusal is the same plain-text shape, so
    // what actually differs about them is a status and a sentence.
    let (status, body): (StatusCode, &[u8]) = match outcome {
        Ok(rendition) => {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, rendition.media_type)
                // A year, and immutable. See this function's caller for why that's safe rather than hopeful.
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                // The one place the encoded bytes are actually copied: Tauri's `body` takes ownership, and
                // everything before this point shares them.
                .body(rendition.bytes.to_vec())
                .expect("the response is built from constants and cannot be malformed");
        }
        Err(Refusal::Unknown) => (StatusCode::NOT_FOUND, b"no image with that identity has been opened"),
        Err(Refusal::Unreadable) => (StatusCode::GONE, b"that image was opened and can no longer be read"),
        // Gone rather than not found: the window is drawing an address this application handed it, and the
        // answer is "superseded" rather than "invented". Shares its status with Unreadable but not its body.
        Err(Refusal::Displaced) => {
            (StatusCode::GONE, b"that enhanced result has been superseded and is no longer held")
        }
    };

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body.to_vec())
        .expect("the response is built from constants and cannot be malformed")
}

#[cfg(test)]
mod tests {
    use super::super::files::describe_and_admit;
    use super::*;
    use crate::images::test_support::{admit, write_image};

    use image::GenericImageView;
    use opai::ImageFormat;
    use tempfile::{TempDir, tempdir};

    /// A picture carrying an alpha channel, for the tests about transparency surviving the round trip.
    fn write_transparent(dir: &TempDir, name: &str, width: u32, height: u32) -> PathBuf {
        let path = dir.path().join(name);
        let pixels = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 0])));

        opai::image::save_blocking(&pixels, &path, ImageFormat::Png, None).expect("writable");
        path
    }

    // ── Where the pixels come from ────────────────────────────────────────────────────────────────────────────────

    /// Where `identity`'s pixels are, against a registry alone and a slot holding no run.
    ///
    /// The scheme resolves through both doors; these are the tests of the one that reads from disk, so the other is
    /// empty. The enhanced half has its own tests below.
    fn located(opened: &Opened, identity: &str) -> Result<Pixels, Refusal> {
        locate(opened, &crate::enhance::Runs::default(), identity)
    }

    #[test]
    fn an_identity_that_was_never_admitted_is_unknown() {
        // A bug in the interface, which asked for something it was never given — and the reason the two refusals are
        // kept apart: this one is nothing to do with the file, because there is no file.
        let opened = Opened::default();

        assert_eq!(located(&opened, "0123456789abcdef").map(|_| ()), Err(Refusal::Unknown));
    }

    #[test]
    fn an_admitted_file_that_has_gone_away_is_unreadable_rather_than_unknown() {
        // A photograph on a drive that has been ejected. The interface is not wrong to have asked, so it must not be
        // told the same thing it would be told for an identity it invented.
        let dir = tempdir().expect("a temporary directory");
        let path = write_image(&dir, "holiday.png", 16, 16, ImageFormat::Png);
        let opened = Opened::default();
        let identity = admit(&opened, &path);

        std::fs::remove_file(&path).expect("removable");

        assert_eq!(
            located(&opened, &identity).and_then(|pixels| decode(pixels, 0)).map(|_| ()),
            Err(Refusal::Unreadable)
        );
    }

    #[test]
    fn an_admitted_file_resolves_to_its_own_pixels() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_image(&dir, "holiday.png", 40, 30, ImageFormat::Png);
        let opened = Opened::default();
        let identity = admit(&opened, &path);

        let picture = located(&opened, &identity)
            .and_then(|pixels| decode(pixels, 0))
            .expect("an admitted, readable file");

        assert_eq!(picture.dimensions(), (40, 30));
        assert_eq!(picture.identity(), identity, "the file served is not the file admitted");
    }

    // ── What a request asks for ───────────────────────────────────────────────────────────────────────────────────

    /// The URL a request arrives as, in the form macOS and Linux produce.
    fn asked(url: &str) -> Option<Asked> {
        parse(&url.parse::<tauri::http::Uri>().expect("the test should name a valid URL"))
    }

    #[test]
    fn a_request_names_an_identity_and_a_bound() {
        assert_eq!(
            asked("opai://localhost/0123456789abcdef?size=100"),
            Some(Asked { identity: "0123456789abcdef".to_string(), bound: 100, crop: None })
        );
    }

    #[test]
    fn both_platform_forms_of_the_url_ask_for_the_same_thing() {
        // The one difference `convertFileSrc` is allowed to know about, and the reason nothing here branches on the
        // platform: the path and the query are the same on both sides of it.
        assert_eq!(
            asked("opai://localhost/cafebabecafebabe?size=144"),
            asked("http://opai.localhost/cafebabecafebabe?size=144"),
        );
    }

    #[test]
    fn an_absent_or_zero_size_is_the_images_own_dimensions() {
        // The reference's convention, and what the preview canvas asks for.
        for url in ["opai://localhost/cafebabecafebabe", "opai://localhost/cafebabecafebabe?size=0"] {
            assert_eq!(asked(url).map(|request| request.bound), Some(0), "{url} did not ask for the whole picture");
        }
    }

    #[test]
    fn a_malformed_size_is_refused_rather_than_read_as_no_bound() {
        // Read as "no bound", a front end that built a broken URL would be served a full-resolution photograph for
        // every thumbnail in the drawer and never find out.
        for url in [
            "opai://localhost/cafebabecafebabe?size=large",
            "opai://localhost/cafebabecafebabe?size=",
            "opai://localhost/cafebabecafebabe?size=-1",
            "opai://localhost/cafebabecafebabe?size=99999999999999999999",
        ] {
            assert_eq!(asked(url), None, "{url} was accepted");
        }
    }

    #[test]
    fn a_request_with_no_crop_asks_for_the_whole_photograph() {
        // The property the whole slice rests on: until something can set a crop, every URL this window
        // builds is the one it built before, and parses to the same request.
        for url in [
            "opai://localhost/cafebabecafebabe",
            "opai://localhost/cafebabecafebabe?size=0",
            "opai://localhost/cafebabecafebabe?size=144",
        ] {
            assert_eq!(asked(url).map(|request| request.crop), Some(None), "{url} was read as carrying a framing");
        }
    }

    #[test]
    fn a_crop_round_trips_every_field_it_carries() {
        let crop = asked("opai://localhost/cafebabecafebabe?size=144&crop=10,20,30,40,-1500,hv")
            .expect("a well-formed request")
            .crop
            .expect("a request carrying a crop");

        // Against a `Crop` built from the six fields rather than read back one accessor at a time: the type
        // is `Eq` because it is half of two cache keys, so comparing the whole value pins every field at
        // once — and pins that no seventh one was invented. The angle is negative because the dialog turns
        // both ways, and an integer, so nothing about the URL depends on how a float was printed.
        assert_eq!(crop, Crop::new(10, 20, 30, 40, -1_500, true, true).expect("a rectangle with area"));
    }

    #[test]
    fn each_spelling_of_the_flips_is_read_as_itself() {
        for (spelling, expected) in
            [("-", (false, false)), ("h", (true, false)), ("v", (false, true)), ("hv", (true, true))]
        {
            let url = format!("opai://localhost/cafebabecafebabe?crop=0,0,10,10,0,{spelling}");
            let crop = asked(&url).expect("a well-formed request").crop.expect("a request carrying a crop");
            let (flip_horizontal, flip_vertical) = expected;

            assert_eq!(
                crop,
                Crop::new(0, 0, 10, 10, 0, flip_horizontal, flip_vertical).expect("a rectangle with area"),
                "{spelling} was read as something else"
            );
        }
    }

    #[test]
    fn a_malformed_crop_is_refused_rather_than_read_as_no_crop() {
        // The same argument the malformed bound above makes, turned the other way round: read as "no
        // crop", a front end that built a broken URL would be served the whole photograph where a framing
        // was asked for, and would draw it into the framing's box.
        for url in [
            // Nothing at all, and too few fields.
            "opai://localhost/cafebabecafebabe?crop=",
            "opai://localhost/cafebabecafebabe?crop=0,0,10,10,0",
            // A seventh field this application does not produce.
            "opai://localhost/cafebabecafebabe?crop=0,0,10,10,0,-,7",
            // A rectangle with no area, which `Crop` refuses to describe at all.
            "opai://localhost/cafebabecafebabe?crop=0,0,0,10,0,-",
            "opai://localhost/cafebabecafebabe?crop=0,0,10,0,0,-",
            // A flip spelling that is not one of the four.
            "opai://localhost/cafebabecafebabe?crop=0,0,10,10,0,x",
            "opai://localhost/cafebabecafebabe?crop=0,0,10,10,0,vh",
            "opai://localhost/cafebabecafebabe?crop=0,0,10,10,0,",
            // A float angle, which is the spelling the integer millidegrees exists to keep out.
            "opai://localhost/cafebabecafebabe?crop=0,0,10,10,1.5,-",
            // A negative origin, an unparseable extent, and a number no `u32` holds.
            "opai://localhost/cafebabecafebabe?crop=-1,0,10,10,0,-",
            "opai://localhost/cafebabecafebabe?crop=0,0,wide,10,0,-",
            "opai://localhost/cafebabecafebabe?crop=0,0,99999999999999999999,10,0,-",
        ] {
            assert_eq!(asked(url), None, "{url} was accepted");
        }
    }

    #[test]
    fn both_platform_forms_of_the_url_read_one_crop_the_same_way() {
        // The one platform-shaped surface this slice has, and the parser is what depends on the path and
        // the query being identical on both sides of `convertFileSrc`.
        assert_eq!(
            asked("opai://localhost/cafebabecafebabe?size=144&crop=10,20,30,40,-1500,hv"),
            asked("http://opai.localhost/cafebabecafebabe?size=144&crop=10,20,30,40,-1500,hv"),
        );
    }

    #[test]
    fn a_request_naming_a_location_rather_than_an_identity_is_refused() {
        // The whole of what stops the webview reading the machine. A path is not 16 hexadecimal characters, so it
        // never reaches the registry, whatever it points at.
        for url in [
            "opai://localhost//Users/someone/Pictures/holiday.jpg",
            "opai://localhost/../../../etc/passwd",
            "opai://localhost/%2Fetc%2Fpasswd",
        ] {
            assert_eq!(asked(url), None, "{url} was accepted");
        }
    }

    #[test]
    fn a_malformed_identity_is_refused() {
        for url in [
            "opai://localhost/",                       // nothing at all
            "opai://localhost/0123456789abcde",        // fifteen characters
            "opai://localhost/0123456789abcdef0",      // seventeen
            "opai://localhost/0123456789ABCDEF",       // the right bytes in the wrong case
            "opai://localhost/0123456789abcdeg",       // not hexadecimal
            "opai://localhost/0123456789abcdef/extra", // an identity with something after it
        ] {
            assert_eq!(asked(url), None, "{url} was accepted");
        }
    }

    #[test]
    fn a_query_this_application_does_not_build_does_not_confuse_the_bound_or_the_crop() {
        // Each parameter found among others, and a parameter that merely ends in one of their names left alone.
        // None of these is a URL this application builds; what matters is that a stray one cannot silently change
        // what is served.
        //
        // `crop=1` stood in for the stray parameter here until this slice, when `crop` became a real one — so it
        // moved from "ignored" to "refused", which is the strictness the parameter is specified with rather than a
        // regression. The stray is now a name neither parameter can ever have.
        assert_eq!(asked("opai://localhost/cafebabecafebabe?size=64&zoom=1").map(|r| r.bound), Some(64));
        assert_eq!(asked("opai://localhost/cafebabecafebabe?maxsize=64").map(|r| r.bound), Some(0));
        assert_eq!(
            asked("opai://localhost/cafebabecafebabe?zoom=1&crop=0,0,10,10,0,-")
                .and_then(|r| r.crop)
                .map(|c| c.width()),
            Some(10)
        );
        assert_eq!(asked("opai://localhost/cafebabecafebabe?precrop=nonsense").map(|r| r.crop), Some(None));
    }

    // ── The downscale ─────────────────────────────────────────────────────────────────────────────────────────────
    //
    // These came from `opai::image::resize`, with the crate. The contract they pin did not change when the function
    // moved; what is new is the prefilter, and the last three are about it.

    /// A picture of a known size, built rather than decoded so these tests depend on no fixture.
    fn image_of(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        }))
    }

    #[test]
    fn a_landscape_picture_is_bounded_on_its_width() {
        let source = image_of(3000, 2000);
        let scaled = bounded(&source, 100);

        assert_eq!(scaled.width(), 100, "the longest edge is not the bound");
        // 2000 * 100 / 3000, to the nearest pixel: the ratio is preserved rather than the box filled.
        assert_eq!(scaled.height(), 67);
    }

    #[test]
    fn a_portrait_picture_is_bounded_on_its_height() {
        // The orientation the Go application needs a branch for, and which this does not: the same square box bounds
        // whichever edge is longer.
        let source = image_of(2000, 3000);
        let scaled = bounded(&source, 100);

        assert_eq!(scaled.height(), 100, "the longest edge is not the bound");
        assert_eq!(scaled.width(), 67);
    }

    #[test]
    fn a_square_picture_is_bounded_on_both_edges() {
        let source = image_of(800, 800);
        let scaled = bounded(&source, 200);

        assert_eq!((scaled.width(), scaled.height()), (200, 200));
    }

    #[test]
    fn a_bound_larger_than_the_picture_does_not_enlarge_it() {
        // The deliberate divergence from the reference, which would hand back a blurred 2000x1500 copy of this.
        let source = image_of(640, 480);
        let enlarged = bounded(&source, 2000);

        assert_eq!((enlarged.width(), enlarged.height()), (640, 480));

        // And it is the same picture rather than a copy of it: this is the path the canvas takes for every
        // photograph it draws, where a clone would be a second decoded buffer for no change in pixels.
        assert!(matches!(enlarged, Cow::Borrowed(_)), "an unenlarged picture was copied");
    }

    #[test]
    fn a_bound_exactly_at_the_longest_edge_returns_the_picture_as_it_is() {
        // The boundary of the case above, asserted on the pixels rather than the dimensions: a resize to the same
        // size still runs a filter over every pixel, and Lanczos3 at scale 1 is not the identity.
        let source = image_of(640, 480);
        let scaled = bounded(&source, 640);

        assert_eq!((scaled.width(), scaled.height()), (640, 480));
        assert_eq!(scaled.as_bytes(), source.as_bytes(), "the picture was filtered rather than left alone");
    }

    #[test]
    fn a_bound_of_zero_returns_the_picture_as_it_is() {
        // What "no bound" arrives as from a front end that has nothing to say about size. An empty picture is never
        // what was meant.
        let source = image_of(64, 48);
        let scaled = bounded(&source, 0);

        assert_eq!((scaled.width(), scaled.height()), (64, 48));
        assert_eq!(scaled.as_bytes(), source.as_bytes());
    }

    #[test]
    fn the_colour_type_survives_the_downscale() {
        // The two properties the rendition path depends on: a developed RAW stays 16-bit, and a transparent picture
        // is still transparent afterwards — which is what lets the encoder branch on the colour type and be right.
        // Both ratios here are past `PREFILTER_FACTOR`, so this covers the prefilter as well as the filter.
        let deep =
            DynamicImage::ImageRgb16(image::ImageBuffer::from_pixel(64, 64, image::Rgb([30_000u16, 20_000, 10_000])));
        assert!(bounded(&deep, 16).as_rgb16().is_some(), "16 bits per channel were narrowed by a downscale");

        let transparent = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(64, 64, image::Rgba([255, 0, 0, 0])));
        let scaled = bounded(&transparent, 16);
        assert!(scaled.color().has_alpha(), "the alpha channel was dropped by a downscale");
        assert_eq!(scaled.to_rgba8().get_pixel(0, 0)[3], 0, "a fully transparent picture came back opaque");
    }

    #[test]
    fn the_prefilter_does_not_change_the_size_the_picture_comes_out_at() {
        // The one thing a second resampling step could plausibly break: two roundings where there was one. Both
        // stages go through the `image` crate's own `resize_dimensions`, and this is what says so — across ratios
        // either side of `PREFILTER_FACTOR`, and on sizes that do not divide evenly.
        //
        // Bounds at or above the longest edge are excluded rather than unlucky: `bounded` returns the picture
        // untouched there, while `resize` enlarges it, and that divergence is the point of the two tests above.
        for (width, height) in [(300u32, 200u32), (401, 299), (199, 301), (80, 80), (123, 57)] {
            for bound in [4u32, 10, 38, 50, 100] {
                if bound >= width.max(height) {
                    continue;
                }

                let source = image_of(width, height);
                let scaled = bounded(&source, bound);
                let directly = source.resize(bound, bound, FilterType::Lanczos3);

                assert_eq!(
                    (scaled.width(), scaled.height()),
                    (directly.width(), directly.height()),
                    "a {width}x{height} picture bounded to {bound} came out at a different size than one stage would"
                );
            }
        }
    }

    #[test]
    fn the_prefilter_is_not_reached_by_a_small_reduction() {
        // Below the factor there is nothing to save, and the extra resampling would cost quality for it. Asserted on
        // the pixels: this must be exactly what one Lanczos3 pass produces.
        let source = image_of(1000, 750);
        let scaled = bounded(&source, 500);

        assert_eq!(scaled.as_bytes(), source.resize(500, 500, FilterType::Lanczos3).as_bytes());
    }

    #[test]
    fn the_prefiltered_downscale_is_indistinguishable_from_one_lanczos3_pass() {
        // What the prefilter is allowed to cost. A gradient rather than a flat fill, so there is high-frequency
        // detail for a cheaper filter to alias on; 40 dB is already past where a difference is visible, and the
        // measurement on a real 6000x3377 photograph was 44.2.
        let source = image_of(1200, 800);

        let scaled = bounded(&source, 100).to_rgb8();
        let directly = source.resize(100, 100, FilterType::Lanczos3).to_rgb8();

        let squared: f64 = scaled
            .pixels()
            .zip(directly.pixels())
            .flat_map(|(a, b)| (0..3).map(move |channel| f64::from(a[channel]) - f64::from(b[channel])))
            .map(|difference| difference * difference)
            .sum();

        let mean = squared / (scaled.pixels().len() * 3) as f64;
        let psnr = 10.0 * (255.0f64 * 255.0 / mean).log10();

        assert!(psnr > 40.0, "the prefilter cost more than it is allowed to: {psnr:.1} dB");
    }

    // ── The rendition ─────────────────────────────────────────────────────────────────────────────────────────────

    /// The rendition `identity` is asked for at `bound`, and the picture it decodes back to.
    ///
    /// One helper for both doors: an admitted file resolves out of `opened`, a held result out of `runs`, and
    /// what `render` then does with the pixels is the same either way. The two named wrappers below say which
    /// door a test came through, which is the part worth reading at the call site.
    fn rendered_from(
        opened: &Opened,
        runs: &crate::enhance::Runs,
        identity: &str,
        bound: u32,
    ) -> (Rendition, DynamicImage) {
        let pixels = locate(opened, runs, identity).expect("the identity should resolve");
        let rendition = render(pixels, &Asked { identity: identity.to_string(), bound, crop: None })
            .expect("a resolvable, readable identity should render");
        let decoded = image::load_from_memory(&rendition.bytes).expect("what was served should be an image");

        (rendition, decoded)
    }

    /// The rendition an admitted file is asked for at `bound`, and the picture it decodes back to.
    fn rendered(opened: &Opened, identity: &str, bound: u32) -> (Rendition, DynamicImage) {
        rendered_from(opened, &crate::enhance::Runs::default(), identity, bound)
    }

    /// The rendition an admitted file is asked for at `bound` with `crop` applied, and its picture.
    ///
    /// Injected here rather than dragged in through a URL, which is the whole of how this slice is verified:
    /// nothing in the application can set a crop until the dialog lands, so every framing these tests see is
    /// one they named themselves.
    fn framed(opened: &Opened, identity: &str, bound: u32, crop: Crop) -> (Rendition, DynamicImage) {
        let pixels = locate(opened, &crate::enhance::Runs::default(), identity).expect("the identity should resolve");
        let asked = Asked { identity: identity.to_string(), bound, crop: Some(crop) };
        let rendition = render(pixels, &asked).expect("a resolvable, readable identity should render");
        let decoded = image::load_from_memory(&rendition.bytes).expect("what was served should be an image");

        (rendition, decoded)
    }

    /// An admitted 400x200 photograph, and the registry holding it.
    fn admitted(dir: &TempDir) -> (Opened, String) {
        let path = write_image(dir, "holiday.png", 400, 200, ImageFormat::Png);
        let opened = Opened::default();
        let identity = admit(&opened, &path);

        (opened, identity)
    }

    #[test]
    fn an_unbounded_framing_is_served_at_exactly_the_rectangles_dimensions() {
        let dir = tempdir().expect("a temporary directory");
        let (opened, identity) = admitted(&dir);

        // The canvas's own request, and the one the window sizes its box from before the pixels arrive: it
        // takes the unreduced path, so the rectangle's dimensions are the answer exactly rather than about.
        let crop = Crop::new(50, 25, 120, 90, 0, false, false).expect("legal");
        let (_, picture) = framed(&opened, &identity, 0, crop);

        assert_eq!(picture.dimensions(), (120, 90), "an unbounded framing was not served at the rectangle");
    }

    #[test]
    fn a_bounded_framing_lands_on_the_bound_and_shows_what_the_unbounded_one_shows() {
        let dir = tempdir().expect("a temporary directory");
        let (opened, identity) = admitted(&dir);

        let crop = Crop::new(40, 20, 200, 100, 0, false, false).expect("legal");
        let (_, whole) = framed(&opened, &identity, 0, crop);
        let (_, thumbnail) = framed(&opened, &identity, 64, crop);

        let longest = thumbnail.width().max(thumbnail.height());
        assert!(longest.abs_diff(64) <= 1, "a bounded framing came out at {longest} on its longest edge, not 64");

        // The reduction costs up to a pixel on each edge — which is what the reference sidesteps by not
        // cropping bounded renditions at all, leaving its sidebar showing the uncropped photograph beside a
        // cropped canvas. What must not differ is the framing: the same part of the photograph, at two sizes.
        let ratio = f64::from(thumbnail.width()) / f64::from(thumbnail.height());
        let expected = f64::from(whole.width()) / f64::from(whole.height());
        assert!(
            (ratio - expected).abs() < 0.05,
            "the bounded framing has a different shape from the unbounded one"
        );

        // The reduced copy is sampled from the same region, so the middle of the framing is about the same
        // colour in both. The fixture is a gradient, so a rectangle taken from elsewhere reads as a different
        // one rather than as a slightly softer copy.
        let middle = whole.get_pixel(whole.width() / 2, whole.height() / 2);
        let small = thumbnail.get_pixel(thumbnail.width() / 2, thumbnail.height() / 2);

        for channel in 0..3 {
            assert!(
                middle[channel].abs_diff(small[channel]) <= 8,
                "the bounded framing shows a different part of the photograph from the unbounded one"
            );
        }
    }

    #[test]
    fn a_turned_framing_is_served_as_png_because_it_carries_alpha() {
        let dir = tempdir().expect("a temporary directory");
        let (opened, identity) = admitted(&dir);

        // A turn exposes corners no source pixel covers, so the framing carries alpha where the photograph
        // did not — and JPEG would composite exactly those corners away against a colour nobody chose.
        let crop = Crop::new(0, 0, 10_000, 10_000, 30_000, false, false).expect("legal");
        let (rendition, picture) = framed(&opened, &identity, 0, crop);

        assert_eq!(rendition.media_type, "image/png", "a framing carrying transparency was served as JPEG");
        assert!(picture.color().has_alpha(), "the transparency a turn produced was composited away");
        assert_eq!(picture.get_pixel(0, 0)[3], 0, "a corner no source pixel covers was served as a colour");
    }

    #[test]
    fn a_request_carrying_no_crop_is_served_exactly_as_it_was() {
        let dir = tempdir().expect("a temporary directory");
        let (opened, identity) = admitted(&dir);

        // The slice's own acceptance criterion, in the one place it can be asserted rather than looked at: a
        // request with no framing is byte-for-byte the response it has always been, bounded and unbounded
        // alike. A framing of the whole photograph is the same bytes again, which is what makes opening the
        // dialog and dismissing it cost nothing.
        let whole = Crop::new(0, 0, 400, 200, 0, false, false).expect("legal");

        for bound in [0, 64] {
            let (uncropped, _) = rendered(&opened, &identity, bound);
            let (unframed, _) = framed(&opened, &identity, bound, whole);

            assert_eq!(uncropped.media_type, unframed.media_type, "a framing that changed nothing changed the format");
            assert_eq!(uncropped.bytes, unframed.bytes, "a framing that changed nothing changed the pixels served");
        }
    }

    // ── The enhanced result ───────────────────────────────────────────────────────────────────────────────────────

    /// The identity of the photograph the enhanced results below were made from — what a release names, since
    /// the window closes a photograph rather than a result. See [`crate::enhance::release_enhanced`].
    const SOURCE: &str = "5ec0ffee5ec0ffee";

    /// A run that produced a picture of the given size, and the identity it produced.
    ///
    /// Driven through [`crate::enhance::Runs`]' own doors rather than by reaching inside it, so what these tests
    /// exercise is the arrangement the enhancement commands actually make.
    fn held(runs: &crate::enhance::Runs, run: &str, identity: &str, width: u32, height: u32) -> String {
        runs.start(run, SOURCE);
        let kept = runs.finish(
            run,
            opai::Picture::new(
                "/pictures/holiday.jpg",
                DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
                    image::Rgb([(x % 256) as u8, (y % 256) as u8, 200])
                })),
                identity,
            ),
        );

        assert!(kept, "the run that was just started did not keep what it produced");

        identity.to_string()
    }

    /// The rendition a held result is asked for at `bound`, and the picture it decodes back to.
    fn rendered_result(runs: &crate::enhance::Runs, identity: &str, bound: u32) -> (Rendition, DynamicImage) {
        rendered_from(&Opened::default(), runs, identity, bound)
    }

    #[test]
    fn a_held_result_is_served_at_its_own_dimensions_when_no_bound_is_asked_for() {
        let runs = crate::enhance::Runs::default();
        let identity = held(&runs, "run-1", "aaaaaaaaaaaaaaaa", 300, 200);

        // The enhanced pane draws at the photograph's own size, so a bounded result would be unusable — design.md D4.
        let (rendition, decoded) = rendered_result(&runs, &identity, 0);

        assert_eq!((decoded.width(), decoded.height()), (300, 200));
        assert_eq!(
            rendition.media_type, "image/jpeg",
            "a result with no alpha was not encoded as an admitted file is"
        );
    }

    #[test]
    fn a_held_result_is_bounded_on_its_longest_edge_when_one_is() {
        let runs = crate::enhance::Runs::default();
        let identity = held(&runs, "run-1", "aaaaaaaaaaaaaaaa", 400, 200);

        // By the same route and in the same terms as an admitted file's, which is the whole of what the spec asks:
        // there is one rendition path and the source of the pixels is the only thing that differs.
        let (_, decoded) = rendered_result(&runs, &identity, 100);

        assert_eq!((decoded.width(), decoded.height()), (100, 50));
    }

    #[test]
    fn a_result_a_later_run_displaced_is_refused_as_no_longer_held() {
        let runs = crate::enhance::Runs::default();
        let identity = held(&runs, "run-1", "aaaaaaaaaaaaaaaa", 64, 48);

        assert!(matches!(locate(&Opened::default(), &runs, &identity), Ok(Pixels::Held(_))));

        // A second run, which drops what it displaces at the moment it is asked for.
        runs.start("run-2", SOURCE);

        assert_eq!(
            locate(&Opened::default(), &runs, &identity).map(|_| ()),
            Err(Refusal::Displaced),
            "a displaced result is still reachable, or has stopped being distinguishable from one never produced"
        );
    }

    #[test]
    fn a_result_released_when_its_image_closed_is_served_as_gone_rather_than_not_found() {
        // The whole of the change, from the command the window calls to the status the scheme answers: the
        // pixels stop being served when the photograph is closed, and the address a pane may still be drawing
        // reads "this is gone" rather than "you asked for something that never was". D2.
        let app = tauri::test::mock_builder()
            .manage(crate::enhance::Runs::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build");
        let runs = app.state::<crate::enhance::Runs>();

        let identity = held(&runs, "run-1", "aaaaaaaaaaaaaaaa", 64, 48);
        assert!(matches!(locate(&Opened::default(), &runs, &identity), Ok(Pixels::Held(_))));

        // The command itself, naming the photograph the window closed rather than the result it produced.
        crate::enhance::release_enhanced(SOURCE.to_string(), app.state(), crate::command::Traceparent::default());

        let refusal = locate(&Opened::default(), &runs, &identity).map(|_| ());
        assert_eq!(refusal, Err(Refusal::Displaced), "a released result is still reachable");

        let response = respond(Err(Refusal::Displaced));
        assert_eq!(response.status(), 410, "a released result reads as one that never existed");
        assert_ne!(response.status(), respond(Err(Refusal::Unknown)).status());
    }

    #[test]
    fn an_identity_no_run_produced_is_unknown_rather_than_displaced() {
        // The two the spec requires an interface drawing a stale address to be able to tell apart.
        let runs = crate::enhance::Runs::default();
        held(&runs, "run-1", "aaaaaaaaaaaaaaaa", 16, 16);

        assert_eq!(locate(&Opened::default(), &runs, "bbbbbbbbbbbbbbbb").map(|_| ()), Err(Refusal::Unknown));
    }

    #[test]
    fn the_two_refusals_a_window_draws_a_stale_address_into_are_different_responses() {
        let unknown = respond(Err(Refusal::Unknown));
        let displaced = respond(Err(Refusal::Displaced));

        assert_eq!(unknown.status(), tauri::http::StatusCode::NOT_FOUND);
        assert_eq!(
            displaced.status(),
            tauri::http::StatusCode::GONE,
            "a superseded result reads as one that never was"
        );

        // Neither carries a cache directive: a refusal must not outlive the state that produced it.
        assert!(unknown.headers().get(tauri::http::header::CACHE_CONTROL).is_none());
        assert!(displaced.headers().get(tauri::http::header::CACHE_CONTROL).is_none());

        // And the two `GONE`s say different things, which is what the log and a bug report read.
        assert_ne!(displaced.body(), respond(Err(Refusal::Unreadable)).body());
    }

    #[test]
    fn an_enhanced_identity_is_served_while_resident_and_refused_once_displaced() {
        // The `gui-images` delta, end to end: what the window may READ widens to the results this application
        // produced, and only while it still holds them.
        let runs = crate::enhance::Runs::default();
        let identity = held(&runs, "run-1", "aaaaaaaaaaaaaaaa", 120, 90);

        let (_, decoded) = rendered_result(&runs, &identity, 0);
        assert_eq!((decoded.width(), decoded.height()), (120, 90), "a result this application holds was not served");

        let displaced = held(&runs, "run-2", "bbbbbbbbbbbbbbbb", 60, 45);

        assert_eq!(locate(&Opened::default(), &runs, &identity).map(|_| ()), Err(Refusal::Displaced));
        assert!(matches!(locate(&Opened::default(), &runs, &displaced), Ok(Pixels::Held(_))));
    }

    #[test]
    fn a_result_widens_what_may_be_drawn_and_not_what_may_be_reached() {
        // The other half of the delta, and the one that matters: an enhanced result is pixels this application
        // computed, never a file it went and read. Nothing outside the admitted set became nameable.
        let dir = tempdir().expect("a temporary directory");
        let outside = write_image(&dir, "not-opened.png", 8, 8, ImageFormat::Png);
        let opened = Opened::default();
        let runs = crate::enhance::Runs::default();

        held(&runs, "run-1", "aaaaaaaaaaaaaaaa", 16, 16);

        // A file on the machine that nobody opened, named by the identity it would carry if anyone had.
        let identity = opai::image::identity_blocking(&outside).expect("readable");

        assert_eq!(
            locate(&opened, &runs, &identity).map(|_| ()),
            Err(Refusal::Unknown),
            "holding a result made a file the user never opened reachable"
        );

        // And a location is still not an identity, so the parse refuses it before the registry is consulted at all.
        assert!(parse(&format!("opai://localhost{}", outside.display()).parse().expect("a URI")).is_none());
    }

    #[test]
    fn a_rendition_of_a_displaced_result_is_not_served_from_the_cache() {
        // The ordering `produce` documents, asserted at the seam it is made in: the entitlement is decided before the
        // rendition cache is consulted, so pixels this application has stopped holding stop being served even though
        // the cache entry is as true as it ever was — an identity, a bound and a crop cannot go stale.
        let runs = crate::enhance::Runs::default();
        let identity = held(&runs, "run-1", "aaaaaaaaaaaaaaaa", 64, 48);
        let renditions = Renditions::default();
        let asked = Asked { identity: identity.clone(), bound: 0, crop: None };

        let (rendition, _) = rendered_result(&runs, &identity, 0);
        match renditions.claim(&asked) {
            Claim::Produce(producing) => producing.keep(&rendition),
            Claim::Ready(_) => panic!("an empty cache answered"),
        }
        assert!(matches!(renditions.claim(&asked), Claim::Ready(_)), "the rendition was not kept");

        runs.start("run-2", SOURCE);

        // The cache would still answer; the resolution no longer does, and it is what runs first.
        assert_eq!(locate(&Opened::default(), &runs, &identity).map(|_| ()), Err(Refusal::Displaced));
    }

    // ── What the crop inherits from the cache being keyed on the request ──────────────────────────────────────────
    //
    // Nothing in `Renditions` was touched by this slice. The crop joined `Asked`, which is what the map is keyed
    // on, so these assert a property rather than an implementation: what the key covers is everything that
    // determines the pixels, and it now covers the framing too.

    /// An application serving one admitted 400x200 photograph, and its identity.
    ///
    /// Through `produce` rather than `render`, which is the only way the rendition cache is in the picture at
    /// all — `render` is the miss path and knows nothing about what has already been produced.
    fn serving(dir: &TempDir) -> (tauri::App<tauri::test::MockRuntime>, String, PathBuf) {
        let path = write_image(dir, "holiday.png", 400, 200, ImageFormat::Png);
        let opened = Opened::default();
        let identity = admit(&opened, &path);

        let app = tauri::test::mock_builder()
            .manage(opened)
            .manage(crate::enhance::Runs::default())
            .manage(Renditions::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build");

        (app, identity, path)
    }

    #[test]
    fn one_identity_bound_and_framing_is_rendered_once_and_served_twice() {
        let dir = tempdir().expect("a temporary directory");
        let (app, identity, path) = serving(&dir);

        let asked = Asked {
            identity,
            bound: 64,
            crop: Some(Crop::new(40, 20, 200, 100, -1_500, true, false).expect("legal")),
        };

        let first = produce(app.handle(), &asked).expect("an admitted, readable identity should render");

        // The file is gone, so anything that decoded it again would fail rather than answer. What comes back is
        // therefore what was kept — which is the whole of why this cache exists on macOS and Linux, where the
        // webview's own does not sit in front of a custom scheme.
        std::fs::remove_file(&path).expect("removable");

        let second = produce(app.handle(), &asked).expect("a framing already produced should be served again");

        assert_eq!(first, second, "a framing already produced was rendered a second time, or served differently");
    }

    #[test]
    fn two_framings_of_one_photograph_are_two_entries() {
        let dir = tempdir().expect("a temporary directory");
        let (app, identity, _) = serving(&dir);

        let left = Asked {
            identity: identity.clone(),
            bound: 0,
            crop: Some(Crop::new(0, 0, 100, 200, 0, false, false).expect("legal")),
        };
        let right = Asked {
            identity: identity.clone(),
            bound: 0,
            crop: Some(Crop::new(300, 0, 100, 200, 0, false, false).expect("legal")),
        };
        let whole = Asked { identity, bound: 0, crop: None };

        let left_bytes = produce(app.handle(), &left).expect("renders").bytes;
        let right_bytes = produce(app.handle(), &right).expect("renders").bytes;
        let whole_bytes = produce(app.handle(), &whole).expect("renders").bytes;

        // Neither serves the other's framing, which is what stops a crop the user changed showing the one they
        // had — the property that used to need invalidation and here needs nothing, because the key covers it.
        assert_ne!(left_bytes, right_bytes, "two framings of one photograph were served one rendition");
        assert_ne!(left_bytes, whole_bytes, "a framing was served the whole photograph's rendition");

        // And the framing already produced is still the framing already produced, after the other two.
        assert_eq!(produce(app.handle(), &left).expect("renders").bytes, left_bytes, "an entry was displaced");
    }

    #[test]
    fn a_framing_of_an_identity_never_admitted_is_refused_exactly_as_an_unframed_one_is() {
        let dir = tempdir().expect("a temporary directory");
        let (app, _, _) = serving(&dir);

        // Carrying a crop widens what may be ASKED for and not what may be REACHED: the entitlement is decided
        // from the identity alone, before the cache and before any framing is applied.
        let unknown = "bbbbbbbbbbbbbbbb".to_string();
        let framed = Asked {
            identity: unknown.clone(),
            bound: 0,
            crop: Some(Crop::new(0, 0, 10, 10, 0, false, false).expect("legal")),
        };
        let unframed = Asked { identity: unknown, bound: 0, crop: None };

        assert_eq!(produce(app.handle(), &framed).map(|_| ()), Err(Refusal::Unknown));
        assert_eq!(
            produce(app.handle(), &framed).map(|_| ()),
            produce(app.handle(), &unframed).map(|_| ()),
            "a framing changed what a request for an identity nobody opened is answered with"
        );
    }

    #[test]
    fn a_bounded_rendition_caps_the_longest_edge_and_keeps_the_ratio() {
        let dir = tempdir().expect("a temporary directory");
        let opened = Opened::default();
        let identity = admit(&opened, &write_image(&dir, "holiday.png", 3000, 2000, ImageFormat::Png));

        let (rendition, decoded) = rendered(&opened, &identity, 100);

        assert_eq!((decoded.width(), decoded.height()), (100, 67));
        assert_eq!(rendition.media_type, "image/jpeg", "a picture with no alpha is a JPEG");
    }

    #[test]
    fn an_unbounded_rendition_is_the_images_own_dimensions() {
        let dir = tempdir().expect("a temporary directory");
        let opened = Opened::default();
        let identity = admit(&opened, &write_image(&dir, "holiday.png", 320, 240, ImageFormat::Png));

        let (_, decoded) = rendered(&opened, &identity, 0);

        assert_eq!((decoded.width(), decoded.height()), (320, 240));
    }

    #[test]
    fn a_bound_larger_than_the_image_does_not_enlarge_it() {
        let dir = tempdir().expect("a temporary directory");
        let opened = Opened::default();
        let identity = admit(&opened, &write_image(&dir, "small.png", 64, 48, ImageFormat::Png));

        let (_, decoded) = rendered(&opened, &identity, 2000);

        assert_eq!((decoded.width(), decoded.height()), (64, 48));
    }

    #[test]
    fn a_transparent_source_comes_back_with_its_transparency() {
        // The deliberate divergence from the reference, which encodes JPEG unconditionally and so previews this
        // photograph with its transparency composited away while exporting the same file keeps it.
        let dir = tempdir().expect("a temporary directory");
        let opened = Opened::default();
        let identity = admit(&opened, &write_transparent(&dir, "logo.png", 64, 64));

        let (rendition, decoded) = rendered(&opened, &identity, 32);

        assert_eq!(rendition.media_type, "image/png", "a JPEG cannot carry an alpha channel");
        assert!(decoded.color().has_alpha(), "the alpha channel was composited away");
        assert_eq!(decoded.to_rgba8().get_pixel(0, 0)[3], 0, "a fully transparent source came back opaque");
    }

    #[test]
    fn a_format_the_webview_cannot_decode_is_served_as_one_it_can() {
        // The whole reason this is not Tauri's own `asset:` protocol, which would stream the file's own bytes and
        // display as nothing. TIFF and HEIF are two of the three the spec names: no webview decodes either, and
        // `opai` decodes both. The third is camera RAW, which the test below reaches by its colour type.
        let dir = tempdir().expect("a temporary directory");

        for (name, format) in [("scan.tiff", ImageFormat::Tiff), ("photo.heif", ImageFormat::Heif)] {
            let opened = Opened::default();
            let identity = admit(&opened, &write_image(&dir, name, 200, 100, format));

            let (rendition, decoded) = rendered(&opened, &identity, 0);

            assert_eq!(rendition.media_type, "image/jpeg", "{name} was not served as something a webview can draw");
            assert_eq!((decoded.width(), decoded.height()), (200, 100), "{name} came back the wrong size");
        }
    }

    #[test]
    fn a_sixteen_bit_source_is_narrowed_for_the_window_rather_than_refused() {
        // Camera RAW's half of the scenario above, reached through the property that makes RAW hard rather than
        // through a RAW file. `opai::image::load` develops every RAW to 16 bits per channel, and the `image` crate's
        // JPEG encoder refuses that colour type outright — which is `design.md` D9's whole reason for encoding
        // through `opai` instead. A 16-bit PNG puts the same colour type in front of the same encoder.
        //
        // A DNG would exercise the demosaic too, and `opai` is where that is owned: it synthesises one from a known
        // CFA and asserts the development, behind ~190 lines of `#[cfg(test)]` fixture builder that nothing outside
        // that crate can reach. A third copy of it here would be an expensive way to re-test someone else's decode;
        // what this crate has to get right is that it hands 16-bit pixels to an encoder that narrows them.
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("developed.png");
        let deep = DynamicImage::ImageRgb16(image::ImageBuffer::from_fn(80, 60, |x, y| {
            image::Rgb([(x * 800) as u16, (y * 1000) as u16, 30_000])
        }));
        opai::image::save_blocking(&deep, &path, ImageFormat::Png, None).expect("writable");

        // The fixture really is 16-bit once loaded back, or the narrowing below was never asked for.
        assert!(
            opai::image::load_blocking(&path).expect("decodable").pixels().as_rgb16().is_some(),
            "the fixture came back narrower than 16 bits, so this asserts nothing"
        );

        let opened = Opened::default();
        let identity = admit(&opened, &path);

        let (rendition, decoded) = rendered(&opened, &identity, 40);

        assert_eq!(rendition.media_type, "image/jpeg", "a picture with no alpha is a JPEG, 16-bit or not");
        assert_eq!((decoded.width(), decoded.height()), (40, 30));
    }

    #[test]
    fn the_same_request_twice_serves_the_same_pixels() {
        // What the immutable cache directive promises. An identity describes bytes and the bound is in the URL, so a
        // given URL cannot come to mean different pixels — which is what lets the webview keep them.
        let dir = tempdir().expect("a temporary directory");
        let opened = Opened::default();
        let identity = admit(&opened, &write_image(&dir, "holiday.png", 128, 96, ImageFormat::Png));

        assert_eq!(rendered(&opened, &identity, 64).0, rendered(&opened, &identity, 64).0);
    }

    // ── The response ──────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_served_rendition_tells_the_window_it_may_keep_it() {
        let response = respond(Ok(Rendition { bytes: vec![1, 2, 3].into(), media_type: "image/jpeg" }));

        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-type"], "image/jpeg");
        assert_eq!(response.headers()["cache-control"], "public, max-age=31536000, immutable");
        assert_eq!(response.body(), &vec![1, 2, 3]);
    }

    #[test]
    fn the_two_refusals_are_distinguishable_and_neither_is_cached() {
        // The spec requires them told apart: one is a bug in the interface, which asked for something it was never
        // given, and the other is a photograph on a drive that has been ejected. Neither is cached, because a file
        // that has gone away may well come back when the drive is plugged in again.
        let unknown = respond(Err(Refusal::Unknown));
        let unreadable = respond(Err(Refusal::Unreadable));

        assert_eq!(unknown.status(), 404);
        assert_eq!(unreadable.status(), 410);
        assert_ne!(unknown.status(), unreadable.status(), "the two refusals arrive as one answer");

        for response in [&unknown, &unreadable] {
            assert!(!response.headers().contains_key("cache-control"), "a refusal was cached");
            assert!(!response.body().is_empty(), "a refusal says nothing about itself");
        }
    }

    // ── The whole seam, against real photographs ──────────────────────────────────────────────────────────────────

    /// Runs the whole seam over the directory named by `OPAI_IMAGE_SAMPLES_DIR`, and does nothing at all when that
    /// variable is unset.
    ///
    /// The same shape, and for the same reason, as `opai`'s own `every_raw_file_in_the_samples_directory_loads_and_
    /// describes`: the fixtures above prove the seam, the routing and the refusals, but every one of them is a
    /// picture this crate wrote moments earlier. What they cannot show is that a folder of photographs off a real
    /// camera describes, admits and renders — which is the thing a person actually does with this application.
    ///
    /// **Never in CI**: the files belong to whoever shot them.
    ///
    /// ```text
    /// OPAI_IMAGE_SAMPLES_DIR=~/Desktop/test cargo test -p gui -- --nocapture samples_directory
    /// ```
    ///
    /// Each file is checked against a **second, independent route to the same facts**: the identity and the
    /// dimensions are compared with what `opai::image::load` produces from the decoded picture, rather than with the
    /// header probe and the file hash the record was built from. An agreement between two spellings of one call would
    /// prove nothing; an agreement between the listing path and the decoding path is the property the registry, the
    /// cache key and every later slice depend on.
    #[test]
    fn every_image_in_the_samples_directory_describes_admits_and_renders() {
        let Some(directory) = std::env::var_os("OPAI_IMAGE_SAMPLES_DIR") else {
            return;
        };

        let paths: Vec<PathBuf> = std::fs::read_dir(&directory)
            .expect("the samples directory should be readable")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.is_file() && !path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.'))
            })
            .collect();
        assert!(!paths.is_empty(), "{directory:?} holds no files");

        let opened = Opened::default();
        let records =
            tauri::async_runtime::block_on(describe_and_admit(paths.clone(), &opened)).expect("the runtime is alive");

        assert_eq!(records.len(), paths.len(), "a file the directory holds was not described");
        println!("describing {} files from {directory:?}", records.len());

        for record in &records {
            let path = PathBuf::from(&record.path);
            let loaded = opai::image::load_blocking(&path).unwrap_or_else(|err| panic!("{}: {err}", record.path));
            let identity = record.identity.as_deref().expect("a readable photograph has an identity");

            assert_eq!(identity, loaded.identity(), "{} lists under one identity and loads under another", record.path);
            assert_eq!(
                (record.width, record.height),
                (Some(loaded.dimensions().0), Some(loaded.dimensions().1)),
                "{} was listed at a size it does not decode to",
                record.path
            );
            assert_eq!(record.size, Some(std::fs::metadata(&path).expect("readable").len()));

            // One bounded rendition and one unbounded, which is the drawer's request and the canvas's.
            let (_, thumbnail) = rendered(&opened, identity, 100);
            let (_, full) = rendered(&opened, identity, 0);

            assert_eq!(thumbnail.width().max(thumbnail.height()), 100, "{} was not bounded at 100", record.path);
            assert_eq!((full.width(), full.height()), loaded.dimensions(), "{} lost pixels unbounded", record.path);

            println!(
                "  {:24} {identity}  {}x{}  ->  {}x{}",
                path.file_name().expect("a file has a name").to_string_lossy(),
                loaded.dimensions().0,
                loaded.dimensions().1,
                thumbnail.width(),
                thumbnail.height(),
            );
        }
    }
}
