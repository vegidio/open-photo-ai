//! The identity of what a run produced, and the slot a result that is not a picture is stored under.

use imaging::ChannelDepth;
use rust_sak::crypto::xxh3_string;

use crate::models::Operation;

/// The identity of the pixels `operations` produce from the pixels `identity` describes, at `depth`.
///
/// The result of a run is **not** the input's pixels, so it must not carry the input's identity. A chained pair of
/// runs agrees with a single run over the concatenated operations. The same chain at 8 and at 16 bits per channel
/// never shares an identity.
///
/// An empty chain leaves the identity untouched, whatever depth was asked for: applying nothing derives nothing.
///
/// XXH3-64 through `rust-sak`'s `crypto`, which is already what a loaded image's identity is — so an identity stays
/// one fixed-width value however long the chain.
pub(crate) fn identity_after(identity: &str, operations: &[Operation], depth: ChannelDepth) -> String {
    // A deliberate divergence: the reference hands the input hash back on the output, so a second run over the result
    // looks its operations up under the original image's identity and gets the first run's picture.
    //
    // **Folded rather than joined**, one operation at a time, each hash over the previous one. That is what makes a
    // chained pair of runs agree with a single run over the concatenated operations — `f(f(h, a), b)` is by
    // construction what `f(h, [a, b])` computes — which a flat join of every tag would not give: joining produces
    // `hash(h, "a|b")` for the one and `hash(hash(h, "a"), "b")` for the other.
    //
    // The depth is in here because the same chain at 8 and at 16 bits per channel produces two images that are not
    // interchangeable, so they cannot carry one identity: uncached that is a mislabel, and keyed on it is the wrong
    // picture served. It is the **resolved** depth, so `OutputDepth::Source` over a 16-bit
    // photograph and `OutputDepth::Sixteen` agree — they are the same pixels.
    //
    // It is folded at **every** step rather than appended once to the finished fold, and the difference is not
    // cosmetic. Appending would give `[a]` at 8 followed by `[b]` at 16 the same value as `[a]` at 16 followed by `[b]`
    // at 16, while the first chain's second operation ran over an 8-bit intermediate. Folding per step makes the
    // intermediate carry its own depth, so the two diverge at the first operation and never meet again.
    operations.iter().fold(identity.to_string(), |identity, operation| {
        // The separators keep two adjacent fields from running together into a third reading — an identity ending in
        // `ab` beside a tag starting with `c` must not hash the same as one ending in `a` beside `bc`.
        xxh3_string(&format!("{identity}|{}|{}", operation.cache_tag(), depth.tag()))
    })
}

// Changing it to something a depth could spell is the edit `identity_of_data`'s collision test exists to fail.
/// What tells a data key apart from a chain key, in the position a [`ChannelDepth::tag`] occupies.
///
/// A word rather than a digit, and that is the whole of the mechanism: the depth tags are `"8"` and `"16"`, so no
/// chain key can be folded from a third value however its identity and its operations vary.
pub(super) const DATA_DISCRIMINATOR: &str = "data";

/// The slot the result of running `operation` over the picture `identity` describes is stored under.
///
/// [`identity_after`]'s sibling for the path whose result is **not** a picture. It folds the identity, the
/// operation's [`cache_tag`](Operation::cache_tag) and [`DATA_DISCRIMINATOR`] the same way a chain step folds its
/// own, so the same operation over a different photograph and a different operation over the same one are each their
/// own entry, and neither is served the other's answer.
///
/// **The depth is no part of it**, unlike a chain key: this path produces no pixels, so bits per channel is not a
/// property its result has. **Neither is the execution provider**, matching the chain's own argument — the same
/// analysis of the same photograph is the same analysis whichever processor produced it.
pub(crate) fn identity_of_data(identity: &str, operation: &crate::models::Subject) -> String {
    // One operation rather than a chain: a result that is not an image is not fed to the next operation, so there is no
    // prefix to fold and nothing a second run could be a continuation of.
    //
    // Why it lives here rather than in the driver that calls it: the property that actually has to hold is a
    // relationship *between* the two derivations. No identity and operation may fold to the same string through both,
    // or a stored picture would be served where a result belongs. A property of a pair is not checkable from inside
    // one of them — a test asserting it has to see both — and a later edit to either is then visibly an edit to a pair
    // rather than a local change to one driver's key.
    //
    // The read and the write both call this, so they cannot disagree about a key neither of them owns — the chain's
    // rule too; see `cache`.
    //
    // `operation` is a `Subject` — the union over both carriers — rather than an `Analysis`, although only the data
    // path calls it. That is what the property being checked requires: no identity and operation may fold to the same
    // string through this and through `identity_after`, and an *image* operation is exactly the case that would
    // collide if the discriminator were ever dropped. A parameter narrowed to the data carrier would make that test
    // unwritable, which is the collision it exists to catch.
    //
    // The same separators, for the same reason: two adjacent fields must not run together into a third reading.
    xxh3_string(&format!("{identity}|{}|{DATA_DISCRIMINATOR}", operation.cache_tag()))
}
