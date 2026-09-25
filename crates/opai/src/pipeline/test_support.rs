//! The fixtures every suite that drives a pipeline without a runtime shares: backends that stand in for ONNX Runtime,
//! and the filter model one of them plays.
//!
//! Beside the contract because they are its doubles: a model's own suite, the tiled filter's and the drivers' all
//! implement or instantiate `Backend` through these. The pictures they run are `imaging::test_support`'s.

use image::Rgb;
use imaging::tensor::{Channel, Sampler};

/// The [`Backend`](super::Backend) run seams a suite does not exercise, stubbed as unreachable.
///
/// Every fake in this crate implements the whole of `Backend` while driving one or two of its four run seams, so the
/// rest are written out as `unreachable!` — which is the right answer (a test that reached one is failing, not being
/// served something invented) written the wrong number of times. Five suites carried byte-identical copies, and the
/// trait's own doc used to claim that its four-method shape meant "every existing double keeps compiling"; adding
/// `run_named_outputs` and `run_weighted` falsified that in three files at once.
///
/// This is where the signatures live now, so the next seam added to the trait is one arm here plus its name in the
/// list at each site that does not drive it — a word, not a pasted body. The list stays per-site because `$why` is
/// per-seam: a suite that skips two seams for two different reasons says both, in place, which is the part worth
/// saying.
///
/// ```ignore
/// impl Backend for Fake {
///     // ... the seams this suite actually drives ...
///     stub_backend_runs!("nothing here runs a graph with more than one output"; run_named_outputs, run_weighted);
/// }
/// ```
macro_rules! stub_backend_runs {
    (@seam run_tile, $why:literal) => {
        fn run_tile(
            _handle: &$crate::sessions::SessionHandle<Self::Session>,
            _input: &[f32],
            _output: &mut [f32],
        ) -> Result<(), Self::Error> {
            unreachable!($why)
        }
    };
    (@seam run_graph, $why:literal) => {
        fn run_graph(
            _handle: &$crate::sessions::SessionHandle<Self::Session>,
            _input: &[f32],
            _input_shape: $crate::pipeline::session::GraphShape,
            _output: &mut [f32],
            _output_shape: $crate::pipeline::session::GraphShape,
        ) -> Result<(), Self::Error> {
            unreachable!($why)
        }
    };
    (@seam run_named_outputs, $why:literal) => {
        fn run_named_outputs(
            _handle: &$crate::sessions::SessionHandle<Self::Session>,
            _input: &[f32],
            _input_shape: $crate::pipeline::session::GraphShape,
            _outputs: &mut [$crate::pipeline::session::NamedOutput<'_>],
        ) -> Result<(), Self::Error> {
            unreachable!($why)
        }
    };
    (@seam run_weighted, $why:literal) => {
        fn run_weighted(
            _handle: &$crate::sessions::SessionHandle<Self::Session>,
            _input: &[f32],
            _input_shape: $crate::pipeline::session::GraphShape,
            _weight: f32,
            _output: &mut [f32],
            _output_shape: $crate::pipeline::session::GraphShape,
        ) -> Result<(), Self::Error> {
            unreachable!($why)
        }
    };
    ($why:literal; $($seam:ident),+ $(,)?) => {
        $( stub_backend_runs!(@seam $seam, $why); )+
    };
}

pub(crate) use stub_backend_runs;

/// A backend that satisfies the seam and runs nothing.
///
/// What the two dispatch seams in [`models`](crate::models) are tested through. Both are generic in the backend for
/// the same reason everything below them is — so the suite drives them with no ONNX Runtime present — but what they
/// are asked is which pipeline an operation reaches, which is answered before a session exists. Every method here is
/// therefore unreachable rather than fake: a test that ran one would be testing something else.
pub(crate) struct NoBackend;

impl super::Backend for NoBackend {
    type Session = ();
    type Error = std::io::Error;

    async fn acquire(
        &self,
        _artifact: &crate::models::ArtifactId,
        _profile: &crate::providers::profile::EpProfile,
        _requested: crate::providers::ExecutionProvider,
        _interest: &crate::sessions::Interest,
    ) -> Result<crate::sessions::SessionHandle<Self::Session>, crate::error::SessionError> {
        unreachable!("the dispatch seams are asked which pipeline an operation reaches, never for a session")
    }

    stub_backend_runs!("the dispatch seams run nothing"; run_tile, run_graph, run_named_outputs, run_weighted);
}

/// What the fake filter model does to every value it is shown, in the graph's own `[0, 1]` range.
pub(crate) const DARKENS: f32 = 0.5;

/// 480x200 at 256/16 is two tiles side by side: the first over columns `0..256`, the second moved back to start at
/// 224. Left of 224 only the first wrote, and from 256 on only the second did.
pub(crate) const PAIR: (u32, u32) = (480, 200);

// The filter fixtures: shared by `filter`'s own suite, which proves the contract once, and by each family that runs it,
// which proves it hands the right guard to the right models. Three callers, so one copy here rather than one in each —
// the promotion rule applied to a fixture.

/// A session standing in for a filter graph: it halves every value it is shown, and can be told to explode on one
/// tile.
pub(crate) struct FakeSession {
    // Halved rather than copied through, so that the model's output is distinguishable from the photograph
    // everywhere — which is what lets a test tell a kept tile from a filtered one. Nothing here is presented as a
    // denoising or a sharpening.
    /// How many tiles have been run, which is how a tile is picked out to explode.
    pub(crate) runs: std::sync::atomic::AtomicUsize,
    /// Which tile, counting from zero, returns `value` in one element of its output.
    explodes: Option<(usize, f32)>,
}

/// A backend for a filter graph with **no ONNX Runtime**.
pub(crate) struct Fake;

impl super::Backend for Fake {
    type Session = FakeSession;
    type Error = std::io::Error;

    async fn acquire(
        &self,
        _artifact: &crate::models::ArtifactId,
        _profile: &crate::providers::profile::EpProfile,
        _requested: crate::providers::ExecutionProvider,
        _interest: &crate::sessions::Interest,
    ) -> Result<crate::sessions::SessionHandle<Self::Session>, crate::error::SessionError> {
        unreachable!("the pipeline is handed its sessions; it never acquires one")
    }

    /// The tile seam, which is the only one a filter graph takes.
    fn run_tile(
        handle: &crate::sessions::SessionHandle<Self::Session>,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(), Self::Error> {
        let session = handle.session();
        let index = session.runs.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // The scale is 1, so the two buffers are one shape: a pipeline that had asked for any other would fail here
        // rather than be read past the end of.
        assert_eq!(input.len(), output.len(), "a filter tile was run at a scale other than 1");

        for (value, source) in output.iter_mut().zip(input) {
            *value = source * DARKENS;
        }

        if let Some((tile, value)) = session.explodes
            && tile == index
        {
            // One element rather than the whole tile: a blow-up is usually a region, and the guard must catch one
            // value among the tile's two hundred thousand.
            output[output.len() / 2] = value;
        }

        Ok(())
    }

    stub_backend_runs!("a filter run takes the tile seam"; run_graph, run_named_outputs, run_weighted);
}

/// A handle on a fake filter session, exploding to `explodes` on the tile it names.
pub(crate) fn session(explodes: Option<(usize, f32)>) -> crate::sessions::SessionHandle<FakeSession> {
    crate::sessions::SessionHandle::held(
        FakeSession { runs: std::sync::atomic::AtomicUsize::new(0), explodes },
        crate::providers::ExecutionProvider::Cpu,
    )
}

/// `source`'s pixel at `(x, y)` as the fake filter model returns it, at channel type `T`.
pub(crate) fn darkened<T: Channel>(source: &Sampler<'_>, x: u32, y: u32) -> [T; 3]
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    source.rgb(x, y).map(|value| T::from_unit(value.to_unit() * DARKENS))
}

/// `source`'s own pixel at `(x, y)`, at channel type `T`.
pub(crate) fn original<T: Channel>(source: &Sampler<'_>, x: u32, y: u32) -> [T; 3]
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    source.rgb(x, y).map(|value| T::from_unit(value.to_unit()))
}
