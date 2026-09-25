//! Installing the files the application downloads at runtime, and recording what each one put on disk.
//!
//! Two kinds of thing are installed through the one pipeline. A **release archive** — the ONNX Runtime and the NVIDIA
//! libraries — is pinned in this repository, fetched from a GitHub release and expanded. A **model** is a bare
//! `.onnx` with nothing to expand, and its hash is published rather than pinned; [`model`] is where that difference
//! lives.

pub(crate) mod artifact;
pub(crate) mod install;
pub(crate) mod manifest;
pub(crate) mod model;
pub(crate) mod release;

#[cfg(test)]
pub(crate) mod test_server;

// The two travel together because they are one question asked from both ends — a caller that wants to hear about a
// transfer is a caller that wants it to happen — and because both of them have to reach the same four layers down to
// where the bytes move.
/// What one install is run under: where its progress goes, and what stops it.
///
/// A caller with neither takes the default: no reporting, and a transfer that runs to completion.
#[derive(Clone, Default)]
pub(crate) struct Installing {
    /// Where this install's progress is reported, or `None` for one nobody is watching.
    pub(crate) on_progress: Option<crate::progress::OnProgress>,
    /// Cancelled when the install is no longer wanted, which stops the transfer where it stands and leaves its
    /// partial on disk for the next one to resume onto.
    ///
    /// The token is the **install's**, not any one caller's. Several requests can be waiting on one install, and what
    /// stops it is all of them going away rather than any one of them; the session cache is what composes the one
    /// from the many. See [`crate::sessions`].
    pub(crate) cancel: tokio_util::sync::CancellationToken,
}

// Two values rather than a `bool`: `models: true` at a call site says nothing about what is true, and this is the one
// initialization decision where a reader getting it backwards loads unverified weights.
//
// Three things it deliberately is not, each of which the other placement would cost:
//
// - Not a field of `ProcessOptions`. That is where `cache` lives, because the cache is a property of a *run* — two
//   runs in one process can legitimately differ. This is a property of the machine's **state**: it decides what is on
//   disk, which every later run in every later process inherits. Per run, one run could install unverified weights
//   that the next run, with the declaration off, would find recorded and use.
// - Not a setter. A value a run cannot see would be changeable between the check and the install.
// - Not an environment variable. Unreachable from a front end, which is genuinely attractive — but it is process-wide
//   state a test would have to set and unset around itself, in a suite that runs tests in parallel threads, and it is
//   exactly the invisible global the two points above argue against.
/// What a process does with a model whose files are already on disk.
///
/// Declared once, in [`InitOptions::models`](crate::InitOptions::models), and in force for the life of that
/// application handle. It exists for one workflow: a model that has been re-exported and not yet published cannot
/// otherwise be run at all, because the file under test does not match the published hash and is replaced by the
/// published weights before anything can measure it. That is the case for benchmarking a change to a model, and it is
/// the case for debugging one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ModelTrust {
    /// Every model file is checked against the SHA-256 published for it. The ordinary rule, and the default.
    #[default]
    Published,

    // Verifying downloaded files even under trust is a deliberate divergence from the reference implementation, whose
    // flag also suppresses the check on bytes it just fetched: the workflow this exists for is a file the operator put
    // there, which the on-disk half serves completely, and bytes arriving over a network are the ones an attacker can
    // choose.
    /// A model **every** file of which is already on disk is used as it is, whatever its contents.
    ///
    /// Four things this does not reach, each of them required by `model-install`'s specification:
    ///
    /// - A model with any file missing is installed from the published sources and verified, whole.
    /// - A file the application itself downloaded is verified in every process.
    /// - The ONNX Runtime and the GPU libraries, which stay pinned, hashed and verified whatever this says.
    /// - The next process. A process that did not declare this reinstalls the model from the published sources and
    ///   verifies it — one debugging session cannot silently poison a machine.
    ///
    /// Each time it is taken, a `warn` naming the model is written.
    LocalFiles,
}
