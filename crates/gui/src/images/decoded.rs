//! The photographs most recently decoded, kept so the next reader of the same file does not decode it again.
//!
//! One photograph is read by many doors: the drawer's thumbnail, the canvas, a face detection, an autopilot
//! analysis, an enhancement and an export each asked [`opai::image::load_blocking`] for it, so a single photograph
//! was decoded four to seven times — for a 45-megapixel JPEG, several hundred milliseconds each, and for a RAW,
//! seconds. [`Decoded`] holds the last few results so every door after the first is a clone of an `Arc`.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use opai::Picture;
use opai::image::ImageIoError;

use crate::sync::lock;

/// How many decoded photographs are held at once.
///
/// Two: the photograph on the canvas and the one the user just came from, which is the pair a user flicks between.
/// The byte budget below is what actually bounds memory; this only stops a run of small pictures from filling it with
/// entries nobody will ask for again.
const ENTRIES: usize = 2;

/// How many bytes of decoded pixels are held at once, across every entry.
///
/// 512 MiB. A 24-megapixel JPEG decodes to 72 MB and a 45-megapixel one to 135 MB, so two of either fit; a
/// 45-megapixel RAW develops to 16 bits per channel, 270 MB, which fits beside a JPEG. A picture larger than the whole
/// budget — a 100-megapixel sixteen-bit scan is 600 MB — is not kept at all: pinning that much for a *possible* second
/// read would cost more than decoding it again does.
///
/// An entry only costs memory while nothing else holds it. The pixels are shared rather than copied, so while the
/// canvas's photograph is also being enhanced, cached and in use are one buffer; what the budget bounds is what stays
/// behind once every reader has finished.
const BUDGET: usize = 512 << 20;

/// The most recently decoded photographs, keyed on identity. Cheap to clone: every clone is the same cache.
///
/// # Why a key can never serve stale pixels
///
/// The key is the identity the decode *computed* — the XXH3-64 of the bytes it read — never the identity it was asked
/// for. A file rewritten since it was admitted decodes to a new identity and is stored under that, so the old key
/// goes on meaning the old bytes, and a request for the old identity misses and reads the file again, exactly as it
/// did before this cache existed. That is why the loader's hash is kept rather than skipped for a caller that
/// "already knows" the identity: the hash is what makes a hit a statement about the bytes, and it costs a few
/// milliseconds against a decode's hundreds.
#[derive(Debug, Clone)]
pub(crate) struct Decoded {
    /// Most recently used first.
    held: Arc<Mutex<VecDeque<Picture>>>,
    /// At most this many entries: [`ENTRIES`], other than in tests.
    entries: usize,
    /// At most this many bytes of pixels: [`BUDGET`], other than in tests.
    budget: usize,
}

impl Default for Decoded {
    fn default() -> Self {
        Self::bounded(ENTRIES, BUDGET)
    }
}

impl Decoded {
    /// A cache holding at most `entries` pictures and `budget` bytes of pixels.
    fn bounded(entries: usize, budget: usize) -> Self {
        Self { held: Arc::default(), entries, budget }
    }

    /// The photograph `identity` names, read from `path` if it is not already held. Blocking.
    ///
    /// Two readers missing on one photograph at the same moment each decode it: coalescing them would make one
    /// reader wait on a decode it might have been cancelled out of, and the second result simply replaces the first.
    ///
    /// # Errors
    ///
    /// The loader's own, for a file that could not be read or decoded. Nothing is kept for a failure.
    pub(crate) fn load_blocking(&self, identity: &str, path: &Path) -> Result<Picture, ImageIoError> {
        if let Some(picture) = self.get(identity) {
            return Ok(picture);
        }

        let picture = opai::image::load_blocking(path)?;
        self.keep(&picture);

        Ok(picture)
    }

    /// [`load_blocking`](Self::load_blocking), off the runtime's own threads.
    ///
    /// # Errors
    ///
    /// As [`load_blocking`](Self::load_blocking).
    ///
    /// # Panics
    ///
    /// Where the decode itself panics — the task is never cancelled, because nothing drops its handle.
    pub(crate) async fn load(&self, identity: String, path: PathBuf) -> Result<Picture, ImageIoError> {
        let decoded = self.clone();

        crate::task::spawn_blocking(move || decoded.load_blocking(&identity, &path))
            .await
            .expect("decoding a picture does not panic, and nothing cancels its thread")
    }

    /// The photograph held under `identity`, moved to the front, or `None`.
    fn get(&self, identity: &str) -> Option<Picture> {
        let mut held = lock(&self.held);
        let at = held.iter().position(|picture| picture.identity() == identity)?;
        let picture = held.remove(at)?;
        held.push_front(picture.clone());

        Some(picture)
    }

    /// Holds `picture` at the front, evicting from the back until both bounds hold again.
    fn keep(&self, picture: &Picture) {
        let size = weight(picture);

        if size > self.budget {
            return;
        }

        let mut held = lock(&self.held);
        held.retain(|kept| kept.identity() != picture.identity());
        held.push_front(picture.clone());

        let mut total: usize = held.iter().map(weight).sum();
        while held.len() > self.entries || total > self.budget {
            let Some(evicted) = held.pop_back() else { break };
            total -= weight(&evicted);
        }
    }
}

/// The bytes a picture's pixels occupy.
fn weight(picture: &Picture) -> usize {
    picture.pixels().as_bytes().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{DynamicImage, RgbImage};

    /// A picture of `width` x `height` RGB pixels, `3 * width * height` bytes, named `identity`.
    fn picture(identity: &str, width: u32, height: u32) -> Picture {
        Picture::new("/pictures/holiday.jpg", DynamicImage::ImageRgb8(RgbImage::new(width, height)), identity)
    }

    fn held(decoded: &Decoded) -> Vec<String> {
        lock(&decoded.held).iter().map(|picture| picture.identity().to_string()).collect()
    }

    #[test]
    fn a_hit_is_the_same_pixels_rather_than_a_copy() {
        let decoded = Decoded::default();
        let kept = picture("aaaaaaaaaaaaaaaa", 4, 4);
        decoded.keep(&kept);

        let hit = decoded.get("aaaaaaaaaaaaaaaa").expect("held");

        assert!(Arc::ptr_eq(&hit.shared_pixels(), &kept.shared_pixels()), "a hit copied the photograph");
        assert!(decoded.get("bbbbbbbbbbbbbbbb").is_none(), "an identity never kept was found");
    }

    #[test]
    fn the_least_recently_used_is_evicted_past_the_entry_count() {
        let decoded = Decoded::default();
        decoded.keep(&picture("aaaaaaaaaaaaaaaa", 4, 4));
        decoded.keep(&picture("bbbbbbbbbbbbbbbb", 4, 4));

        // Reading the older one makes the newer one the least recently used.
        decoded.get("aaaaaaaaaaaaaaaa").expect("held");
        decoded.keep(&picture("cccccccccccccccc", 4, 4));

        assert_eq!(held(&decoded), ["cccccccccccccccc", "aaaaaaaaaaaaaaaa"]);
    }

    #[test]
    fn a_picture_larger_than_the_budget_is_not_kept() {
        // A 4x4 picture is 48 bytes and a 5x5 one 75, against a budget of 64.
        let decoded = Decoded::bounded(ENTRIES, 64);
        decoded.keep(&picture("aaaaaaaaaaaaaaaa", 4, 4));
        decoded.keep(&picture("bbbbbbbbbbbbbbbb", 5, 5));

        assert_eq!(held(&decoded), ["aaaaaaaaaaaaaaaa"], "an oversized picture was kept, or displaced another");
    }

    #[test]
    fn the_byte_budget_evicts_before_the_entry_count_does() {
        // Two 48-byte pictures against a budget of 64: the count allows both, the budget one.
        let decoded = Decoded::bounded(ENTRIES, 64);
        decoded.keep(&picture("aaaaaaaaaaaaaaaa", 4, 4));
        decoded.keep(&picture("bbbbbbbbbbbbbbbb", 4, 4));

        assert_eq!(held(&decoded), ["bbbbbbbbbbbbbbbb"]);
    }

    #[test]
    fn a_file_rewritten_since_it_was_read_is_keyed_under_its_new_identity() {
        // Staleness is keyed out: the cache stores what the decode computed, so a request for the identity the file
        // had reads it again rather than being served the new bytes under the old name.
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = crate::images::test_support::write_image(&dir, "holiday.png", 8, 8, opai::ImageFormat::Png);
        let decoded = Decoded::default();

        let first = decoded.load_blocking("0000000000000000", &path).expect("readable");
        assert_ne!(first.identity(), "0000000000000000");
        assert_eq!(held(&decoded), [first.identity().to_string()], "kept under the identity it was asked for");

        // And the identity it did compute is a hit from then on.
        let again = decoded.load_blocking(first.identity(), &path).expect("readable");
        assert!(Arc::ptr_eq(&again.shared_pixels(), &first.shared_pixels()), "a second read decoded again");
    }

    #[test]
    fn a_failure_is_not_kept() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let decoded = Decoded::default();

        assert!(decoded.load_blocking("aaaaaaaaaaaaaaaa", &dir.path().join("gone.png")).is_err());
        assert!(held(&decoded).is_empty());
    }
}
