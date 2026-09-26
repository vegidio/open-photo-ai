//! What a caller may say about a run beyond the picture and the operations.

use tokio_util::sync::CancellationToken;

use crate::inference::depth::OutputDepth;
use crate::inference::progress::OnInference;
use crate::providers::ExecutionProvider;

/// Everything about a run that has a sensible default.
///
/// Passed as an `Option` to [`Opai::process`](crate::Opai::process): only the picture and the operations have no
/// sensible default, and a caller that wants "just enhance this" passes `None` rather than four values it has no
/// opinion about.
///
/// `..Default::default()` is the documented way to set one field:
///
/// ```
/// # use opai::{OutputDepth, ProcessOptions};
/// let options = ProcessOptions { depth: OutputDepth::Source, ..Default::default() };
/// ```
///
/// Written that way, a field added later is source-compatible for every caller.
#[derive(Clone)]
pub struct ProcessOptions {
    // An `Option` of a bundle is the same idiom `image::save` already uses for its encode options. A bundle rather than
    // five more parameters, so that a field added later is source-compatible for every caller.
    //
    // Deliberately **not** `#[non_exhaustive]`, unlike this crate's public enums: that attribute bars an external crate
    // from struct-literal syntax altogether, including the `..Default::default()` form above — which is the very thing
    // that makes a field added later source-compatible. A `Default` plus that idiom gives the compatibility the
    // attribute would be reached for, and leaves the idiom usable.
    /// What to run on. Defaults to [`ExecutionProvider::Auto`], which resolves to the best this machine supports and,
    /// where a provider throws an error opening or running the model, falls back to the next best — the CPU last.
    /// Any other provider falls back to the CPU where it cannot open the model.
    pub provider: ExecutionProvider,

    /// How many bits per channel the result carries. Defaults to [`OutputDepth::Eight`] — see that type for why, and
    /// for why a front end's own default should be the other one.
    pub depth: OutputDepth,

    /// Where progress reports go. Defaults to none, and a caller that asks for none is charged for none.
    pub on_progress: Option<OnInference>,

    /// The token that stops the run. Defaults to a fresh one nobody holds, so a caller with no cancel button does not
    /// have to make one.
    ///
    /// Borrowed from the caller in spirit though owned here: pass a clone of the token you kept, and cancelling yours
    /// cancels this run. A child token under one parent is how a front end stops every image of an export queue from
    /// one gesture.
    pub cancel: CancellationToken,

    // `false` serves the two callers that need it rather than merely prefer it: one measuring what a model costs —
    // which would otherwise measure a read, or charge the storing of a result to the model — and one processing images
    // it must not persist.
    //
    // A `bool` rather than an enum because there is no third mode anyone has asked for, and per run rather than a
    // setter on the application because a run cannot see a hidden global's value — the reference has to bind its own
    // to a local for that reason.
    /// Whether this run may read from and write to the run cache. Defaults to `true`.
    ///
    /// `false` computes every operation, returns exactly what a cached run would have returned, and leaves **nothing**
    /// behind.
    ///
    /// **Decided once for the whole run**, not per operation, so a run cannot read from the cache and then decline to
    /// write back to it.
    ///
    /// It is a different question from [`Opai::cache_mode`](crate::Opai::cache_mode), which reports what is *backing*
    /// the cache: a run with this set to `false` still reads `Disk` there.
    pub cache: bool,
}

impl std::fmt::Debug for ProcessOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Written out rather than derived: a callback has nothing to print, and the fact a reader of this actually
        // wants is whether one was registered at all.
        f.debug_struct("ProcessOptions")
            .field("provider", &self.provider)
            .field("depth", &self.depth)
            .field("on_progress", &self.on_progress.is_some())
            .field("cancel", &self.cancel)
            .field("cache", &self.cache)
            .finish()
    }
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            provider: ExecutionProvider::Auto,
            depth: OutputDepth::default(),
            on_progress: None,
            cancel: CancellationToken::new(),
            // On, because the whole point of the cache is that a user who re-runs an enhancement does not pay for it
            // twice, and a default of `false` would mean every caller had to know to ask for that.
            cache: true,
        }
    }
}
