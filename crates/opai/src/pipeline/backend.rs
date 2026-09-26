//! Where a run's sessions come from, and how a tensor is run against one.

use std::future::Future;

use super::session::{GraphShape, NamedOutput};
use crate::error::SessionError;
use crate::models::ArtifactId;
use crate::providers::ExecutionProvider;
use crate::providers::profile::EpProfile;
use crate::sessions::{Interest, SessionHandle};

// The seam that keeps every driver and every pipeline runnable without a runtime. The chain never names an `ort`
// type: it asks a backend for a session and hands the backend a tile. Production instantiates it over this
// application's session cache and `session::run_tile`; the chain's own suite instantiates it over a fake that records
// what it was asked for and enlarges each pixel into a block.
//
// That is what makes the things most likely to be wrong — the refusals, the ordering, the three progress weightings,
// the depth resolution, the identity composition, what a cancelled run returns — checked properties on every CI
// platform, with no ONNX Runtime, no GPU, no model file and no network: "the second operation's refusal happens
// before the first one's model is installed" is a test rather than an argument.
/// Where a chain's sessions come from, and how a tile is run against one.
pub(crate) trait Backend: Send + Sync + 'static {
    // One method per contract, for the reason `session`'s header gives. A method added here takes one arm in
    // `stub_backend_runs` too, plus its name at each fake that does not run it.

    /// The session type, which production instantiates at `ort::session::Session`.
    type Session: Send + Sync + 'static;

    // Generic for the reason the tiled driver is generic in it: an `ort::Error` cannot be constructed without a loaded
    // runtime, so a suite that has none could not otherwise drive a failing model at all.

    /// What a failed tile reports, which production instantiates at `ort::Error`.
    type Error: std::error::Error + Send + Sync + 'static;

    /// The session for `artifact`, installing its files first if they are not on disk.
    ///
    /// `interest` is what the run brings to that install: where its progress goes, and the run's own cancellation —
    /// which withdraws it from the install rather than failing it, so a transfer is stopped by the last run waiting
    /// on it going away. See [`sessions::Interest`](Interest).
    fn acquire(
        &self,
        artifact: &ArtifactId,
        profile: &EpProfile,
        requested: ExecutionProvider,
        interest: &Interest,
    ) -> impl Future<Output = Result<SessionHandle<Self::Session>, SessionError>> + Send;

    /// Records that the session built on `provider` threw an error running `artifact`, so an `Auto` request falls
    /// back to the next provider of its ladder from now on. Returns whether anything was newly declined, which is
    /// what tells a driver a retry would run somewhere different.
    ///
    /// A default that declines nothing, for the fakes that never exercise the fallback.
    fn decline(&self, _artifact: &ArtifactId, _provider: ExecutionProvider) -> bool {
        false
    }

    /// Runs one tile against `handle`, writing what the model produced into `output`.
    fn run_tile(handle: &SessionHandle<Self::Session>, input: &[f32], output: &mut [f32]) -> Result<(), Self::Error>;

    /// Runs one tensor of `input_shape` against `handle`, writing a result of `output_shape` into `output`.
    ///
    /// Reached by a pipeline whose stages are neither one session nor one shape: several graphs addressed by role,
    /// each with its own input and output extents, run as stages of a single pass over a region.
    fn run_graph(
        handle: &SessionHandle<Self::Session>,
        input: &[f32],
        input_shape: GraphShape,
        output: &mut [f32],
        output_shape: GraphShape,
    ) -> Result<(), Self::Error>;

    /// Runs one tensor of `input_shape` against `handle`, writing each of `outputs` back by the name it carries.
    ///
    /// For a graph that returns **several** tensors rather than one. The only contract that spells tensor names;
    /// [`session::run_named_outputs`](super::session::run_named_outputs) says why.
    ///
    /// Nothing is written on a failure, including for the outputs that were present.
    fn run_named_outputs(
        handle: &SessionHandle<Self::Session>,
        input: &[f32],
        input_shape: GraphShape,
        outputs: &mut [NamedOutput<'_>],
    ) -> Result<(), Self::Error>;

    /// Runs one tensor of `input_shape` against `handle` **with `weight` beside it**, writing a result of
    /// `output_shape` into `output`.
    ///
    /// For a graph whose second input is a value rather than pixels — a face restorer takes the aligned face and a
    /// fidelity weight. The inputs are positional and the weight is an `f32` rather than a buffer;
    /// [`session::run_weighted`](super::session::run_weighted) says why both of those are the opposite answer from
    /// [`run_named_outputs`](Self::run_named_outputs)'s.
    fn run_weighted(
        handle: &SessionHandle<Self::Session>,
        input: &[f32],
        input_shape: GraphShape,
        weight: f32,
        output: &mut [f32],
        output_shape: GraphShape,
    ) -> Result<(), Self::Error>;
}
