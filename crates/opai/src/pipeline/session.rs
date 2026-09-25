//! Feeding one tile, or one shaped tensor, to ONNX Runtime.
//!
//! Four calls, because there are four contracts:
//!
//! - [`run_tile`] — a fixed-shape convolutional model: three colour planes, square, the shape recovered from the buffer.
//! - [`run_graph`] — a model whose input is neither three-channel nor square; the caller states both shapes.
//! - [`run_named_outputs`] — a graph that returns several tensors, each read back by name.
//! - [`run_weighted`] — a graph fed a second input that is a value rather than an image.

// The **only** file in this module that names an `ort` type, and the seam that keeps everything above it runnable
// without a runtime: the tile grid, the padding, the decode, the blend, the pass sequence, the progress weighting and
// every refusal are arithmetic exercised on every CI platform, and this is where a runtime becomes necessary.
//
// Giving `run_tile` shape parameters and collapsing the first two was considered and rejected: its shape recovery is a
// guarantee the compiler keeps for a driver that allocates its scratch at exactly the tile shape, and a parameter is a
// guarantee a caller can get wrong. Four contracts, four functions.

use ort::session::Session;
use ort::value::TensorRef;

use crate::sessions::SessionHandle;

// Stated rather than inferred, which is the whole of what `run_graph` adds over `run_tile`: a latent-space graph takes
// sixteen channels at an eighth of the image's resolution, and no buffer length says so.
/// The shape of one tensor a graph is run with, batch excluded because every graph here takes exactly one image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphShape {
    /// How many planes. Three for pixels, sixteen for a latent, thirty-three for the packed transformer input.
    pub(crate) channels: usize,
    /// The plane's height in elements.
    pub(crate) height: usize,
    /// The plane's width in elements.
    pub(crate) width: usize,
}

impl GraphShape {
    /// A shape of `channels` planes of `width` by `height`.
    pub(crate) const fn new(channels: usize, width: usize, height: usize) -> Self {
        Self { channels, height, width }
    }

    /// The dimensions ONNX Runtime is given, batch first and always one.
    pub(crate) const fn dims(self) -> [usize; 4] {
        [1, self.channels, self.height, self.width]
    }

    /// How many floats a tensor of this shape holds.
    pub(crate) const fn len(self) -> usize {
        self.channels * self.height * self.width
    }
}

impl std::fmt::Display for GraphShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[1,{},{},{}]", self.channels, self.height, self.width)
    }
}

/// Runs `handle`'s model over one tile, writing what it produced into `output`.
pub(crate) fn run_tile(handle: &SessionHandle<Session>, input: &[f32], output: &mut [f32]) -> Result<(), ort::Error> {
    // Derived from the buffer rather than passed in, so it cannot disagree with the geometry the driver actually
    // partitioned at: the scratch is `3 * shape * shape` floats and the driver allocates it at exactly the tile shape.
    let shape = ((input.len() / 3) as f64).sqrt() as usize;

    // **The input is borrowed, not copied.** `TensorRef::from_array_view` takes a shape and a slice, so the driver's
    // input scratch — allocated once for the whole run — becomes the tensor the runtime reads with no allocation at
    // all. That is what the driver's out-parameter shape is for.
    let tensor = TensorRef::from_array_view(([1usize, 3, shape, shape], input))?;

    // **The lock is taken per tile rather than held across a pass.** Serializing two *tiles* of one model against one
    // device costs nothing; serializing two *images* would make an export queue's second photograph wait out the whole
    // of the first — minutes. Per tile it interleaves them at tile granularity, for one uncontended lock of tens of
    // nanoseconds against hundreds of milliseconds of work.
    let mut session = handle.session();
    // The graph's tensor names are deliberately not spelled. `Session::run` with positional inputs uses the session's
    // own declared names, so a conventionally exported graph — one tensor in, one out, which is every fixed-shape
    // model this project ships — needs no name here, and a graph that declared something else still runs.
    let outputs = session.run(ort::inputs![tensor])?;

    // Positionally, and without panicking on a graph that declared no output: `SessionOutputs`' own indexing
    // panics there, and a malformed graph is a message rather than a crash on a user's machine.
    let value = outputs.values().next().ok_or_else(|| ort::Error::new("the model returned no output tensor"))?;
    // **The output is copied once, and I/O binding is not used.** `try_extract_tensor` borrows the runtime's own
    // buffer, which is freed when the outputs drop, so the data has to leave it. The copy is ~12 MB per tile at a 4x
    // pass against a per-tile inference cost of a couple of hundred milliseconds — under a percent — and plain `run`
    // keeps this as small as it can be. `TensorRefMut::from_array_view_mut` is there if measurement ever says
    // otherwise, and it is a change to this function alone.
    let (_, produced) = value.try_extract_tensor::<f32>()?;

    // `copy_from_slice` panics on a length mismatch, and the length is the model's answer rather than this
    // crate's: a graph whose output scale disagrees with the pass it is serving would take the process down.
    if produced.len() != output.len() {
        return Err(ort::Error::new(format!(
            "the model returned {} floats where {} were expected for a {shape}x{shape} tile",
            produced.len(),
            output.len()
        )));
    }

    output.copy_from_slice(produced);

    Ok(())
}

/// Runs `handle`'s model over one tensor of `input_shape`, writing what it produced into `output`.
///
/// [`run_tile`]'s sibling for the graphs whose geometry a buffer length cannot carry. `output` must already be
/// `output_shape.len()` floats long.
///
/// # Errors
///
/// [`ort::Error`] where the input does not match `input_shape`, where the run itself failed, where the graph returned
/// no tensor, or where what it returned is not `output_shape.len()` floats. `output` is untouched in every case.
pub(crate) fn run_graph(
    handle: &SessionHandle<Session>,
    input: &[f32],
    input_shape: GraphShape,
    output: &mut [f32],
    output_shape: GraphShape,
) -> Result<(), ort::Error> {
    // The three properties `run_tile`'s comments defend hold here too: the input is borrowed, the output is copied
    // once, and the session lock is taken per call rather than held across a run. A three-graph region therefore takes
    // three short locks rather than one long one, which preserves the interleaving that lets an export queue's second
    // image make progress against the first. The tensor names are not spelled, for `run_tile`'s reason.
    let tensor = TensorRef::from_array_view((input_shape.dims(), input))?;

    let mut session = handle.session();
    let outputs = session.run(ort::inputs![tensor])?;

    accept_first(&outputs, output, output_shape)
}

/// Runs `handle`'s model over one tensor of `input_shape` **and one value beside it**, writing a result of
/// `output_shape` into `output`.
///
/// For a graph whose second input is a weight rather than pixels: a CodeFormer-style restorer takes the aligned face
/// and a fidelity. `output` must already be `output_shape.len()` floats long.
///
/// # Errors
///
/// [`ort::Error`] where the input does not match `input_shape`, where the weight tensor could not be built, where the
/// run itself failed, where the graph returned no tensor, or where what it returned is not `output_shape.len()`
/// floats. `output` is untouched in every case.
pub(crate) fn run_weighted(
    handle: &SessionHandle<Session>,
    input: &[f32],
    input_shape: GraphShape,
    weight: f32,
    output: &mut [f32],
    output_shape: GraphShape,
) -> Result<(), ort::Error> {
    // `run_tile`'s three properties hold here too — the image tensor is borrowed, the output is copied once, the lock
    // is taken per call — and the result is read back through the same `accept` checker `run_graph` uses, so the two
    // failures that seam already has are this one's too and stay exercisable with no runtime.
    let tensor = TensorRef::from_array_view((input_shape.dims(), input))?;

    // The weight is an `f32` rather than a second buffer: it is one value, and the signature says so. Widening
    // `run_graph` to a list of input tensors instead would give every existing caller a slice to build for one tensor,
    // a way to pass two by mistake, and a weight whose meaning is its position in a list rather than its name in a
    // signature.
    //
    // A `[1]` tensor, which is the shape the graph declares for it — not a scalar, and not the image tensor's rank.
    // Held in a binding so the slice it views outlives the run, as the image tensor's buffer does.
    let weight = [weight];
    let weight = TensorRef::from_array_view(([1usize], weight.as_slice()))?;

    let mut session = handle.session();
    // Positional, the opposite call from `run_named_outputs`. That function spells names because three output tensors
    // of comparable shape and interchangeable type are the same thing to the runtime, so a re-export that reordered
    // them would be read wrongly with nothing able to notice. Here the two inputs are `[1,3,512,512]` and `[1]`: they
    // differ in rank and in extent by orders of magnitude, so an export that swapped them fails at the runtime with a
    // shape error on its first run rather than producing a plausible wrong answer. Naming them would buy nothing and
    // would instead make this call fail on a graph that declared its inputs under names other than the ones this
    // crate spelled — which is the argument `run_tile` and `run_graph` already make.
    let outputs = session.run(ort::inputs![tensor, weight])?;

    accept_first(&outputs, output, output_shape)
}

/// Reads a run's **first** output tensor back into `output`, checking it against `output_shape`.
///
/// # Errors
///
/// [`ort::Error`] where the returned tensor is not `f32`, or where it is not `output_shape.len()` floats. `output`
/// is untouched in every case.
fn accept_first(
    outputs: &ort::session::SessionOutputs<'_>,
    output: &mut [f32],
    output_shape: GraphShape,
) -> Result<(), ort::Error> {
    // The tail `run_graph` and `run_weighted` share: the two differ in what they feed the graph and in nothing they do
    // with what comes back. Positionally, as `run_tile` does — `Session::run` with positional inputs uses the session's
    // own declared names, so a graph that renamed its single output is still read correctly.
    let value = outputs.values().next();
    // The extract sits inside the `Option` so that a graph returning nothing reaches `accept` as `None` rather than
    // being reported here, which is what puts both of this seam's own failures in one place that needs no runtime to
    // exercise.
    let produced = match &value {
        Some(value) => Some(value.try_extract_tensor::<f32>()?.1),
        None => None,
    };

    // Converted here rather than raised here, so a caller sees one error type and the disagreement itself stays
    // checkable without a runtime.
    accept(output, produced, output_shape).map_err(|error| ort::Error::new(error.to_string()))
}

// The name carries the identity and the buffer's length carries the arity, which between them is what catches a
// re-export: `accept_named` rejects a name the graph does not declare and a tensor whose length the buffer does not
// match.
/// One output tensor a caller wants back, named, with the buffer it is to be written into.
pub(crate) struct NamedOutput<'a> {
    /// The tensor's name, as the graph declares it — `loc`, `conf`, `landmarks`.
    pub(crate) name: &'a str,
    /// Where the tensor's values are written, and whose length is what the graph's result is checked against.
    pub(crate) buffer: &'a mut [f32],
}

/// Runs `handle`'s model over one tensor of `input_shape` and writes each of `outputs` back **by name**.
///
/// # Errors
///
/// [`ort::Error`] where the input does not match `input_shape`, where the run itself failed, or where
/// [`accept_named`] rejects what came back. **No buffer is written on any failure**, including the outputs that were
/// present.
pub(crate) fn run_named_outputs(
    handle: &SessionHandle<Session>,
    input: &[f32],
    input_shape: GraphShape,
    outputs: &mut [NamedOutput<'_>],
) -> Result<(), ort::Error> {
    // `run_tile` and `run_graph` deliberately spell no tensor names, because a conventionally exported one-in-one-out
    // graph needs none. **That argument does not survive three outputs.** A RetinaFace graph returns `loc`, `conf` and
    // `landmarks` — three tensors of three lengths and three meanings — and taking them positionally makes the result
    // depend on the order they happen to be declared in. A re-export that reordered them would decode confidences as
    // box offsets and return detections that are *wrong* rather than absent, with nothing at any layer able to
    // notice. The name is the only thing that ties a buffer to the meaning the caller wanted, so this call spells one
    // per output.
    let tensor = TensorRef::from_array_view((input_shape.dims(), input))?;

    let mut session = handle.session();
    // The input is still positional, because there is still exactly one of it.
    let produced = session.run(ort::inputs![tensor])?;

    // Every named tensor extracted before anything is written, so that a graph missing the third one leaves the first
    // two untouched — the guarantee `accept_named` is written for, which a write-as-you-go loop could not give.
    let mut extracted: Vec<(&str, Option<&[f32]>)> = Vec::with_capacity(outputs.len());
    for output in outputs.iter() {
        let value = match produced.get(output.name) {
            Some(value) => Some(value.try_extract_tensor::<f32>()?.1),
            None => None,
        };

        extracted.push((output.name, value));
    }

    // Converted here rather than raised here, so that a caller of this seam sees one error type and the
    // disagreement itself stays checkable without a runtime.
    accept_named(outputs, &extracted).map_err(|error| ort::Error::new(error.to_string()))
}

// This crate's own error rather than an `ort::Error`, for the reason `GraphOutput` gives. Naming the output is the whole
// point of addressing them by name: "the model returned 33600 floats where 67200 were expected" says nothing about
// which of three tensors is wrong.
/// Why one of a graph's named outputs could not be accepted: [`GraphOutput`]'s sibling for the call that addresses
/// outputs by name. Every variant names the **output**.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum NamedOutputError {
    /// The graph declares no output under this name — a re-export that renamed or dropped a tensor.
    #[error("the model declares no output named {name}")]
    Absent {
        /// The name the caller asked for.
        name: String,
    },
    /// The named output returned a tensor of a different length than the caller's buffer.
    #[error("the model returned {produced} floats for {name} where {expected} were expected")]
    Length {
        /// Which output disagreed.
        name: String,
        /// How many floats arrived.
        produced: usize,
        /// How many the caller's buffer holds.
        expected: usize,
    },
    // A caller's mistake rather than a model's, separate for the reason `GraphOutput::Buffer` is separate: one says the
    // export disagrees with what was asked for, the other says the request never made sense. Two buffers under one name
    // cannot both be the tensor the graph declares, and accepting the request would silently write one of them and
    // leave the other as it was.
    /// The caller listed the same name twice.
    #[error("the caller listed {name} more than once")]
    Duplicate {
        /// The name that appeared twice.
        name: String,
    },
}

/// Writes each extracted tensor into the buffer that named it, or reports why it cannot be accepted.
///
/// `extracted` is what the graph produced for each name, in the order `outputs` lists them, with `None` where the
/// graph declared no such output.
///
/// **Nothing is written on any failure, including for the outputs that were present.**
///
/// # Errors
///
/// [`NamedOutputError`], having written nothing.
fn accept_named(outputs: &mut [NamedOutput<'_>], extracted: &[(&str, Option<&[f32]>)]) -> Result<(), NamedOutputError> {
    // Pure, and split out from `run_named_outputs` for exactly that: the failures this seam has that are not the
    // runtime's own are checked properties on a runner with no runtime rather than branches nothing ever takes.
    //
    // The caller's own mistake first, and before anything is measured against a model's answer: two buffers under one
    // name is a request that has no correct outcome.
    for (index, output) in outputs.iter().enumerate() {
        if outputs[..index].iter().any(|earlier| earlier.name == output.name) {
            return Err(NamedOutputError::Duplicate { name: output.name.to_string() });
        }
    }

    for (output, (name, produced)) in outputs.iter().zip(extracted) {
        debug_assert_eq!(output.name, *name, "the extraction was not in the order the outputs were listed");

        let Some(produced) = produced else {
            return Err(NamedOutputError::Absent { name: (*name).to_string() });
        };

        if produced.len() != output.buffer.len() {
            return Err(NamedOutputError::Length {
                name: (*name).to_string(),
                produced: produced.len(),
                expected: output.buffer.len(),
            });
        }
    }

    // Only once every one of them is known good, which is what makes the guarantee above a property of the order
    // rather than of each branch remembering to undo its predecessors. A caller handed two of three tensors has two
    // thirds of a detection and no way to tell it from a whole one, and `copy_from_slice` on a length mismatch would
    // take the process down on a user's machine besides.
    for (output, (_, produced)) in outputs.iter_mut().zip(extracted) {
        let produced = produced.expect("an absent output was rejected above");
        output.buffer.copy_from_slice(produced);
    }

    Ok(())
}

// This crate's own error rather than an `ort::Error` built where the disagreement is found, and that is what makes the
// check a tested one: constructing an `ort::Error` loads the runtime's API table, so a function returning one is a
// function no CI runner can call. `run_graph` converts it at the seam, where a runtime is loaded by definition.
/// Why what a graph returned could not be accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum GraphOutput {
    /// The graph returned no tensor at all — a malformed or mismatched export.
    #[error("the model returned no output tensor for a {shape} result")]
    Absent {
        /// The shape the graph was run for.
        shape: GraphShape,
    },
    /// The graph returned a tensor of a different length than the caller declared.
    #[error("the model returned {produced} floats where {expected} were expected for a {shape} result")]
    Length {
        /// How many floats arrived.
        produced: usize,
        /// How many the declared shape requires.
        expected: usize,
        /// The shape the graph was run for.
        shape: GraphShape,
    },
    // A caller's mistake rather than a model's, and separate from `Length` for that reason: one says the export
    // disagrees with the shape it was run for, the other says the caller does.
    /// The caller's own buffer is not the length the shape it declared requires.
    #[error("the caller's buffer holds {buffer} floats, but the {shape} it declared needs {expected}")]
    Buffer {
        /// How long the buffer the caller supplied is.
        buffer: usize,
        /// How many the declared shape requires.
        expected: usize,
        /// The shape the caller declared.
        shape: GraphShape,
    },
}

/// Writes what a graph produced into `output`, or reports why it cannot be accepted.
///
/// **Everything is measured against `shape` rather than against `output`'s length**, the buffer first.
///
/// **Nothing is written on any failure.**
///
/// # Errors
///
/// [`GraphOutput`], having written nothing.
fn accept(output: &mut [f32], produced: Option<&[f32]>, shape: GraphShape) -> Result<(), GraphOutput> {
    // Pure, and split out from `run_graph` for exactly that: the three ways a graph's output can be unusable — none at
    // all, one of the wrong length, and a caller's buffer that does not match the shape it declared — are the
    // failures of a single-output run that are not the runtime's own (`accept_named` holds the named-output call's),
    // so this is what makes them checked properties on a runner with no runtime rather than branches nothing ever
    // takes.
    //
    // Measured against `shape` because that is what makes the declared output shape load-bearing instead of
    // decorative: comparing the model's result against whatever buffer arrived would accept a caller that declared
    // one geometry and allocated another, and the shape would then be doing nothing but appearing in the error
    // message. The caller's own mistake first, so that a buffer which disagrees with the declared shape is reported as
    // that rather than as the model returning the wrong length.
    if output.len() != shape.len() {
        return Err(GraphOutput::Buffer { buffer: output.len(), expected: shape.len(), shape });
    }

    let Some(produced) = produced else {
        return Err(GraphOutput::Absent { shape });
    };

    if produced.len() != shape.len() {
        return Err(GraphOutput::Length { produced: produced.len(), expected: shape.len(), shape });
    }

    // Only after every check: `copy_from_slice` panics on a length mismatch, and the length is the model's answer
    // rather than this crate's, so a graph whose output disagrees with the shape it was run for would otherwise take
    // the process down on a user's machine. A partial write would be worse still: half a restored latent beside the
    // previous region's tail is an image, and nothing downstream could tell it from a finished one.
    output.copy_from_slice(produced);

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::{GraphShape, NamedOutput, NamedOutputError, accept, accept_named};

    use image::{DynamicImage, ImageBuffer, Rgb};

    use imaging::tensor::Normalisation;
    use imaging::{ChannelDepth, RunTile, TileGeometry, run_tiled};

    /// A source large enough to be more than one tile at the default geometry, so there is a second tile to compare
    /// the first against.
    fn source() -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(600, 400, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 0])))
    }

    #[test]
    fn every_tile_is_handed_the_same_input_buffer_rather_than_a_fresh_copy() {
        // The property the borrowed tensor rests on: the driver's input scratch is one allocation for the whole run,
        // so `TensorRef::from_array_view` over it is a view of the same memory on every tile rather than a copy the
        // runtime reads once. The borrow itself needs a live run to observe; what is checkable without a runtime is
        // that there is exactly one buffer to borrow.
        let mut addresses = Vec::new();
        let mut lengths = Vec::new();

        let mut record = |input: &[f32], output: &mut [f32]| -> Result<(), Infallible> {
            addresses.push(input.as_ptr() as usize);
            lengths.push(input.len());
            output.fill(0.0);
            Ok(())
        };

        run_tiled(
            &source(),
            TileGeometry::default(),
            2,
            ChannelDepth::Eight,
            Normalisation::Unit,
            None,
            &|| false,
            &mut record as &mut RunTile<'_, Infallible>,
        )
        .unwrap();

        assert!(addresses.len() > 1, "the source was one tile, so nothing was reused");
        assert!(
            addresses.windows(2).all(|pair| pair[0] == pair[1]),
            "the input buffer moved between tiles: {addresses:?}"
        );
        // The shape the tensor is built at is fixed for the whole run, which is the other half of why one view is
        // enough: a length that changed per tile would mean a tensor rebuilt around a different buffer.
        assert!(lengths.windows(2).all(|pair| pair[0] == pair[1]), "the tile shape changed between tiles");
    }

    /// A canary value no accepted write leaves behind, so "nothing was written" is checked rather than assumed.
    const UNTOUCHED: f32 = -12.5;

    #[test]
    fn a_stated_shape_carries_a_channel_count_a_buffer_length_could_not() {
        // The case `run_tile`'s `sqrt(len / 3)` cannot serve, and the reason this seam exists: a latent is sixteen
        // planes at an eighth of the image's resolution, and 16*120*120 is also 3*400*192 — nothing in the length
        // says which.
        let latent = GraphShape::new(16, 120, 120);

        assert_eq!(latent.dims(), [1, 16, 120, 120]);
        assert_eq!(latent.len(), 16 * 120 * 120);
        assert_eq!(latent.to_string(), "[1,16,120,120]");
    }

    #[test]
    fn a_stated_shape_need_not_be_square_or_match_its_graphs_other_side() {
        // The three shapes one diffusion region actually runs at: pixels in, a compressed latent out, a packed
        // thirty-three-channel latent through the middle. Two of the three are neither three-channel nor at the
        // image's own resolution, and the encode's two sides differ in both channels and resolution.
        let pixels = GraphShape::new(3, 960, 960);
        let latent = GraphShape::new(16, 120, 120);
        let packed = GraphShape::new(33, 120, 120);

        assert_eq!(pixels.len(), 3 * 960 * 960);
        assert_ne!(pixels.len(), latent.len(), "the encode's two sides were the same length");
        assert_eq!(packed.len(), 2 * latent.len() + 120 * 120, "the packed input is two latents and a mask plane");

        // And a shape that is not square at all, which nothing in the tiled contract can express.
        let oblong = GraphShape::new(16, 160, 96);
        assert_eq!(oblong.dims(), [1, 16, 96, 160]);
        assert_eq!(oblong.len(), 16 * 96 * 160);
    }

    #[test]
    fn a_graph_returning_the_declared_length_is_written_through_whole() {
        let shape = GraphShape::new(16, 2, 2);
        let produced: Vec<f32> = (0..shape.len()).map(|value| value as f32).collect();
        let mut output = vec![UNTOUCHED; shape.len()];

        accept(&mut output, Some(&produced), shape).expect("a result of the declared length is accepted");

        assert_eq!(output, produced);
    }

    #[test]
    fn a_graph_returning_the_wrong_number_of_values_is_an_error_naming_both_lengths() {
        // The one place a mismatched or malformed export becomes visible before the pixels are wrong.
        let shape = GraphShape::new(16, 2, 2);
        let mut output = vec![UNTOUCHED; shape.len()];

        for produced in [vec![1.0_f32; shape.len() - 1], vec![1.0_f32; shape.len() + 1], Vec::new()] {
            let error = accept(&mut output, Some(&produced), shape)
                .expect_err("a result of the wrong length was accepted")
                .to_string();

            assert!(error.contains(&produced.len().to_string()), "the error did not name what arrived: {error}");
            assert!(error.contains(&shape.len().to_string()), "the error did not name what was expected: {error}");
            assert!(error.contains("[1,16,2,2]"), "the error did not name the shape it was run for: {error}");
            assert!(
                output.iter().all(|value| *value == UNTOUCHED),
                "a rejected result was partially written: {output:?}"
            );
        }
    }

    #[test]
    fn a_graph_returning_no_tensor_at_all_is_an_error_rather_than_a_panic() {
        // `SessionOutputs`' own indexing panics on an empty result, so a malformed graph has to be turned into a
        // message here or it is a crash with nothing to report.
        let shape = GraphShape::new(3, 4, 4);
        let mut output = vec![UNTOUCHED; shape.len()];

        let error = accept(&mut output, None, shape)
            .expect_err("a graph returning nothing was accepted")
            .to_string();

        assert!(error.contains("no output tensor"), "the error did not say what was missing: {error}");
        assert!(error.contains("[1,3,4,4]"), "the error did not name the shape it was run for: {error}");
        assert!(output.iter().all(|value| *value == UNTOUCHED), "an absent result was written: {output:?}");
    }

    #[test]
    fn a_caller_whose_buffer_does_not_match_the_shape_it_declared_is_told_so() {
        let shape = GraphShape::new(16, 4, 4);
        let produced = vec![1.0_f32; shape.len()];

        for length in [shape.len() - 1, shape.len() + 1, 0] {
            let mut output = vec![UNTOUCHED; length];

            let error = accept(&mut output, Some(&produced), shape)
                .expect_err("a buffer that does not match the declared shape was accepted")
                .to_string();

            assert!(error.contains(&length.to_string()), "the error did not name the buffer: {error}");
            assert!(error.contains(&shape.len().to_string()), "the error did not name what the shape needs: {error}");
            assert!(output.iter().all(|value| *value == UNTOUCHED), "a rejected call wrote to the buffer");
        }
    }

    #[test]
    fn a_weighted_run_carries_the_same_two_failures_a_declared_shape_run_does() {
        // `run_weighted` adds an input and no new way for a result to be unusable: it reads what came back through
        // the very `accept` a declared-shape run does, so its two failures are these — and they stay checkable with
        // **no ONNX Runtime**. Driven at the restoration graph's own shape, which is where this call is actually used.
        let shape = GraphShape::new(3, 512, 512);

        let mut output = vec![UNTOUCHED; shape.len()];
        let error = accept(&mut output, None, shape)
            .expect_err("a restoration graph returning nothing was accepted")
            .to_string();

        assert!(error.contains("no output tensor"), "the error did not say what was missing: {error}");
        assert!(error.contains("[1,3,512,512]"), "the error did not name the shape it was run for: {error}");
        assert!(output.iter().all(|value| *value == UNTOUCHED), "an absent restoration was written");

        let produced = vec![1.0_f32; shape.len() - 1];
        let error = accept(&mut output, Some(&produced), shape)
            .expect_err("a restoration of the wrong length was accepted")
            .to_string();

        assert!(error.contains(&produced.len().to_string()), "the error did not name what arrived: {error}");
        assert!(error.contains(&shape.len().to_string()), "the error did not name what was expected: {error}");
        assert!(output.iter().all(|value| *value == UNTOUCHED), "a rejected restoration was partially written");
    }

    /// The three buffers a detection graph's outputs are read into, at a small anchor count, all filled with the
    /// canary so that "nothing was written" is checked rather than assumed.
    fn detection_buffers() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        (vec![UNTOUCHED; 4 * 4], vec![UNTOUCHED; 4 * 2], vec![UNTOUCHED; 4 * 10])
    }

    /// The three outputs a detection run names, over `loc`, `conf` and `landmarks` in that order.
    fn named<'a>(loc: &'a mut [f32], conf: &'a mut [f32], landmarks: &'a mut [f32]) -> [NamedOutput<'a>; 3] {
        [
            NamedOutput { name: "loc", buffer: loc },
            NamedOutput { name: "conf", buffer: conf },
            NamedOutput { name: "landmarks", buffer: landmarks },
        ]
    }

    #[test]
    fn three_named_outputs_are_each_written_at_their_own_length() {
        // The case the whole call exists for: three tensors of three different lengths and three different meanings,
        // each matched to the name the caller asked for rather than to a position.
        let (mut loc, mut conf, mut landmarks) = detection_buffers();
        let produced_loc: Vec<f32> = (0..16).map(|value| value as f32).collect();
        let produced_conf: Vec<f32> = (0..8).map(|value| value as f32 + 100.0).collect();
        let produced_landmarks: Vec<f32> = (0..40).map(|value| value as f32 + 200.0).collect();

        let mut outputs = named(&mut loc, &mut conf, &mut landmarks);
        accept_named(
            &mut outputs,
            &[
                ("loc", Some(&produced_loc)),
                ("conf", Some(&produced_conf)),
                ("landmarks", Some(&produced_landmarks)),
            ],
        )
        .expect("three outputs of the declared lengths are accepted");

        assert_eq!(loc, produced_loc);
        assert_eq!(conf, produced_conf);
        assert_eq!(landmarks, produced_landmarks);
    }

    #[test]
    fn a_named_output_the_graph_does_not_declare_is_an_error_naming_it_and_writes_nothing() {
        // The re-export that renamed or dropped a tensor. What makes this worth checking beyond the message is the
        // second assertion: `conf` and `landmarks` were both present, and neither may be written, because two thirds
        // of a detection is indistinguishable from a whole one.
        let (mut loc, mut conf, mut landmarks) = detection_buffers();
        let produced_conf = vec![1.0_f32; 8];
        let produced_landmarks = vec![1.0_f32; 40];

        let mut outputs = named(&mut loc, &mut conf, &mut landmarks);
        let error = accept_named(
            &mut outputs,
            &[("loc", None), ("conf", Some(&produced_conf)), ("landmarks", Some(&produced_landmarks))],
        )
        .expect_err("a missing output was accepted");

        assert!(error.to_string().contains("loc"), "the error did not name the missing output: {error}");
        assert!(matches!(error, NamedOutputError::Absent { .. }));

        for (name, buffer) in [("loc", &loc), ("conf", &conf), ("landmarks", &landmarks)] {
            assert!(buffer.iter().all(|value| *value == UNTOUCHED), "{name} was written on a rejected call");
        }
    }

    #[test]
    fn a_named_output_of_the_wrong_length_is_an_error_naming_both_lengths_and_writes_nothing() {
        // A graph re-exported at a different anchor count.
        let (mut loc, mut conf, mut landmarks) = detection_buffers();
        let produced_loc = vec![1.0_f32; 16];
        let produced_conf = vec![1.0_f32; 8];
        let short_landmarks = vec![1.0_f32; 30];

        let mut outputs = named(&mut loc, &mut conf, &mut landmarks);
        let error = accept_named(
            &mut outputs,
            &[
                ("loc", Some(&produced_loc)),
                ("conf", Some(&produced_conf)),
                ("landmarks", Some(&short_landmarks)),
            ],
        )
        .expect_err("an output of the wrong length was accepted");

        let message = error.to_string();
        assert!(message.contains("landmarks"), "the error did not name the output: {message}");
        assert!(message.contains("30"), "the error did not name what arrived: {message}");
        assert!(message.contains("40"), "the error did not name what was expected: {message}");

        for (name, buffer) in [("loc", &loc), ("conf", &conf), ("landmarks", &landmarks)] {
            assert!(buffer.iter().all(|value| *value == UNTOUCHED), "{name} was written on a rejected call");
        }
    }

    #[test]
    fn a_caller_listing_one_name_twice_is_told_so_and_nothing_is_written() {
        let mut first = vec![UNTOUCHED; 4];
        let mut second = vec![UNTOUCHED; 4];
        let produced = vec![1.0_f32; 4];

        let mut outputs = [
            NamedOutput { name: "conf", buffer: &mut first },
            NamedOutput { name: "conf", buffer: &mut second },
        ];
        let error = accept_named(&mut outputs, &[("conf", Some(&produced)), ("conf", Some(&produced))])
            .expect_err("a duplicated name was accepted");

        assert!(error.to_string().contains("conf"), "the error did not name the repeated output: {error}");
        assert!(matches!(error, NamedOutputError::Duplicate { .. }));
        assert!(
            first.iter().chain(second.iter()).all(|value| *value == UNTOUCHED),
            "a rejected call wrote a buffer"
        );
    }

    #[test]
    fn outputs_are_matched_by_name_rather_than_by_the_order_the_graph_declares_them() {
        // The failure this seam exists to prevent, stated as the property that prevents it: a graph declaring its
        // three outputs in another order still fills each caller buffer with the tensor whose name it asked for. The
        // extraction is presented in the caller's order because that is what `run_named_outputs` builds, and it
        // builds it by looking each name up rather than by walking what the graph returned.
        let (mut loc, mut conf, mut landmarks) = detection_buffers();
        let produced_loc = vec![7.0_f32; 16];
        let produced_conf = vec![8.0_f32; 8];
        let produced_landmarks = vec![9.0_f32; 40];

        let mut outputs = named(&mut loc, &mut conf, &mut landmarks);
        accept_named(
            &mut outputs,
            &[
                ("loc", Some(&produced_loc)),
                ("conf", Some(&produced_conf)),
                ("landmarks", Some(&produced_landmarks)),
            ],
        )
        .expect("the lookup is by name, so a declaration order is not a contract");

        // Each buffer carries the value of the tensor it named, and no two of them carry the same one — which a
        // positional read of a reordered graph would break.
        assert!(loc.iter().all(|value| *value == 7.0));
        assert!(conf.iter().all(|value| *value == 8.0));
        assert!(landmarks.iter().all(|value| *value == 9.0));
    }

    #[test]
    fn the_models_result_is_measured_against_the_declared_shape_rather_than_against_the_buffer() {
        let shape = GraphShape::new(16, 4, 4);
        let mut output = vec![UNTOUCHED; shape.len()];

        let error =
            accept(&mut output, Some(&vec![1.0_f32; shape.len() - 1]), shape).expect_err("a short result was accepted");

        assert!(
            matches!(error, super::GraphOutput::Length { .. }),
            "reported as something other than the model's: {error}"
        );
    }
}
