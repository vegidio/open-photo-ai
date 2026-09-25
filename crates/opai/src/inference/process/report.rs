//! What a run returns: the picture, and what it was actually executed on.

use crate::image::Picture;
use crate::providers::ExecutionProvider;

#[cfg(doc)]
use super::ProcessOptions;

/// What a run was executed on: what it was asked for, beside what it actually ran on.
///
/// The two can differ without anything failing. A provider this machine does not support, or one that cannot open a
/// particular model, resolves to a CPU run rather than an error, and a downgraded run returns the same image, in the
/// same way, only several times slower.
///
/// Part of what a successful run **returns**, rather than something a caller registers interest in beforehand, so a
/// caller that asked for no progress reporting still gets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReport {
    // A CPU run rather than an error is deliberate. What makes it worth reporting is that it is otherwise invisible:
    // without this, a benchmark attributes the slowness to the model and a user attributes it to the application, and
    // nothing the run produced can tell either of them otherwise.
    //
    // Not a `Stage` on the progress stream: a downgrade is a property of the outcome, not an event during it, and the
    // default caller, which registers no callback, is exactly the one that would lose it.
    //
    // The two halves are one type rather than two loose fields on `Enhanced` so that they travel together: a front end
    // saying *"CUDA could not open this model, so it ran on the CPU"* needs both, and splitting them across the result
    // invites a caller to render one.
    /// What the run asked for — [`ProcessOptions::provider`], unchanged.
    pub requested: ExecutionProvider,

    // A set rather than a list: a chain that takes four handles on one provider — which is what a two-operation,
    // two-pass upscale does — would otherwise name it four times, and nothing a caller does with the answer is served
    // by the multiplicity.
    /// Every provider a session of this run was actually built on, in the order the run first built on each.
    ///
    /// **Every** session, not only the first to be downgraded. Whether a provider can open a model is decided per
    /// model, so one chain can legitimately run one operation on a GPU and the next on the CPU, and a report naming
    /// only one of them would describe a run that did not happen.
    ///
    /// Each provider appears once, however many sessions were built on it. Ordered, so that two runs of the same shape
    /// produce reports that compare equal.
    ///
    /// **Empty is a third outcome, not "ran on the CPU".** An empty chain and a run whose every operation was served
    /// from the store both build no session at all, and reporting [`requested`](Self::requested) as though it had
    /// been used would claim a measurement that was never taken. `report.actual.first().unwrap_or(report.requested)`
    /// is the line this exists to stop being written.
    pub actual: Vec<ExecutionProvider>,
}

/// What a [`ProviderReport`] says about the run, as a single answer.
///
/// Folding reports *across* runs stays the caller's business; reading one is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderVerdict {
    // The report carries two fields and three traps in reading them: an empty `actual` is not "ran on what was asked
    // for", a requested `Auto` makes "downgraded" a meaningless judgement, and the comparison that distinguishes the
    // remaining two is against the whole set rather than its first element. Every consumer that wants to *say*
    // something about a run — a benchmark's NOTE column, a front end's *"CUDA could not open this model, so it ran on
    // the CPU"* — has to get all three right, and the report's own documentation describes them without encoding them.
    // So it is encoded here, once.
    /// Every session was built on the provider that was requested.
    AsRequested(ExecutionProvider),

    /// [`Auto`](ExecutionProvider::Auto) was requested, so "downgraded" is not a meaningful judgement; this is what it
    /// resolved to.
    Resolved(Vec<ExecutionProvider>),

    /// A session was built on something other than what was asked for.
    Downgraded {
        /// What the run asked for.
        requested: ExecutionProvider,
        /// Every provider a session was actually built on.
        actual: Vec<ExecutionProvider>,
    },

    /// No session was built at all, so nothing was executed.
    ///
    /// Never reported as "ran on what was asked for". A run whose every operation was served from the cache, and an
    /// empty chain, both look exactly like this.
    NothingExecuted,
}

impl ProviderReport {
    /// What this report says about the run.
    pub fn verdict(&self) -> ProviderVerdict {
        if self.actual.is_empty() {
            return ProviderVerdict::NothingExecuted;
        }

        if self.requested == ExecutionProvider::Auto {
            return ProviderVerdict::Resolved(self.actual.clone());
        }

        if self.actual.as_slice() == [self.requested] {
            return ProviderVerdict::AsRequested(self.requested);
        }

        ProviderVerdict::Downgraded { requested: self.requested, actual: self.actual.clone() }
    }
}

impl ProviderReport {
    /// The report of a run that asked for `requested` and built a session on each of `built`, in that order.
    pub(crate) fn new(requested: ExecutionProvider, built: impl IntoIterator<Item = ExecutionProvider>) -> Self {
        // The deduplication is here rather than at the fold's call site, because "a provider is named once however
        // many sessions were built on it" is a property of the report rather than of how any one run happened to
        // accumulate its handles — which is also why both drivers fold through this rather than each writing the
        // property out.
        let mut actual: Vec<ExecutionProvider> = Vec::new();
        for provider in built {
            if !actual.contains(&provider) {
                actual.push(provider);
            }
        }

        Self { requested, actual }
    }
}

/// What a run produced: the enhanced picture, and what it was executed on.
///
/// [`Opai::process`](crate::Opai::process)'s return value, mirroring [`ProcessOptions`] on the way in.
#[derive(Debug, Clone)]
pub struct Enhanced {
    // A bundle with named fields rather than a tuple, so that a third thing about a run — whether any step was served
    // from the store, how long inference took as distinct from encoding — can be added without moving a call site. A
    // tuple gets exactly one addition before it stops being readable.
    //
    // Deliberately **not** `#[non_exhaustive]`, unlike this crate's public enums. The attribute would bar an external
    // crate from destructuring this at all, including the `let Enhanced { picture, .. } = ..` form that is how a
    // caller takes the half it wants — and that form is already what makes a field added later source-compatible, so
    // the attribute would cost the idiom and buy nothing.
    /// The enhanced image.
    ///
    /// Its [`identity`](Picture::identity) is composed from the input's and every operation applied — and
    /// deliberately **not** from [`providers`](Self::providers): the same chain over the same image produces the same
    /// pixels whichever processor ran it.
    pub picture: Picture,

    /// What the run was asked to execute on, beside what it actually executed on.
    pub providers: ProviderReport,
}
