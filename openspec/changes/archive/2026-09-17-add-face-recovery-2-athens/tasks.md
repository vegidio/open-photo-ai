## 1. A graph fed a value beside its image

- [x] 1.1 Add `session::run_weighted(handle, input, input_shape, weight: f32, output, output_shape)` to
      `inference/session.rs` beside `run_graph`, binding the image positionally and the weight as a `[1]` tensor, and
      reading the output back through the existing `accept` checker. Add `Backend::run_weighted` in
      `inference/process.rs`, implement it on `Opai` in `lib.rs`, and add it to all six test doubles
      (`inference/test_support.rs`, `inference/process.rs`, `inference/execute.rs`, `models/upscale/osaka/mod.rs`,
      `models/upscale/osaka/region.rs`, `models/detection/newyork/mod.rs`) — `unimplemented!()` with a stated reason
      on the five that double single-input models. Write the doc comment to say why the inputs are positional when
      `run_named_outputs`' outputs are named (design D1). Verify `cargo build -p opai --all-targets` is clean, that
      the two failures `accept` already covers — a length disagreement and no tensor at all — are exercised for this
      call with **no ONNX Runtime loaded**, and that `run_tile`, `run_graph` and `run_named_outputs` are unedited.

## 2. Athens' geometry, each piece on its own arithmetic

- [x] 2.1 Write `models/face_recovery/athens/transform.rs`: the 2x3 `f32` affine type, the ArcFace template as five
      literal points at a reference size of 512, its scaling to a caller-supplied tile size, the least-squares
      similarity fit (`calculateSimilarityTransform`) including the degenerate fallback to pure translation when the
      source norm is below 1e-10, and the inverse with its singular fallback to the identity. Verify with tests that
      the five template points are the reference's literals in the documented landmark order; that landmarks already
      at the template's positions fit the identity; that a known rotation and uniform scale are recovered; that the
      fit produces no shear when the sources are shear-free; that coincident landmarks translate rather than produce
      NaN; and that a point round-trips through the transform and its inverse.
- [x] 2.2 Write `models/face_recovery/athens/warp.rs`: walk a `tile x tile` destination, map each pixel through the
      inverse transform, sample `Sampler` bilinearly with **reflection** at the edges (`reflectCoord`), and write the
      result into a planar CHW `f32` tensor under `Normalisation::Signed`, with no intermediate image (design D2).
      Keep the per-pixel expression ungrouped as the reference's is. Verify with tests that an identity transform
      over a known image reproduces its pixels in the tensor exactly; that a half-pixel offset interpolates between
      two known neighbours; that a transform reaching past the border reflects rather than clamping or filling; that
      a 16-bit source is not narrowed on the way in; and that the tensor's range is `[-1, 1]`.
- [x] 2.3 Write `models/face_recovery/athens/mask.rs`: the feathered ellipse as `Vec<f32>` alpha — 1.0 inside 0.7 of
      the half-extent, linear falloff to 0.0 at 1.0 — then a separable Gaussian at σ = 15 over a radius of 3σ, with a
      `LazyLock` holding the 512 plane (design D3). Verify with tests that the unblurred plane is 1.0 at the centre,
      0.0 at each corner and symmetric about both axes; that the blur preserves symmetry and leaves no value outside
      `[0, 1]`; that the blurred rim is monotonically decreasing outwards along an axis; and that the cached plane is
      built once.
- [x] 2.4 Write `models/face_recovery/athens/composite.rs`: walk the bounding box expanded by
      `0.5 * max(width, height)` and clipped to the image, map each destination pixel **forward** into the aligned
      square, skip it outside the square or below an alpha of 0.001, sample the mask and the restored CHW tensor
      bilinearly there, and write the weighted mix over what the buffer already holds, generic over `Channel`
      (design D4). Verify with tests that under an identity transform the written window is exactly the box plus its
      margin clipped to the image; that nothing outside it is touched; that a fully opaque mask centre writes the
      restoration and a zero-alpha rim leaves the background; that a second face composites over the first's result;
      and that a box at the image's edge writes nothing outside the image.

## 3. The pipeline and the seam

- [x] 3.1 Write `models/face_recovery/athens/mod.rs`: the tile size, the graph's normalisation, the `Model<B>` impl
      naming one artifact against Athens' profile, and the `ImagePipeline<B>` impl — dispatch on `ChannelDepth` into
      a generic body, refuse a zero-area image with `InferenceError::Untileable`, read the source through `Sampler`
      into an `ImageBuffer<Rgb<T>>`, then per face: align (2.1, 2.2), `run_weighted` with
      `fidelity.unwrap_or(Fidelity::MAX)` (design D7), composite (2.4), with two progress steps and three
      cancellation checks per face and **no reserved head** (design D6). `stages()` answers the face count. Verify it
      builds and that an empty selection returns the source at the requested depth rather than the source untouched.
- [x] 3.2 Add `FaceRecovery::pipeline<B>` as the family's contract seam — an Athens arm reaching 3.1 and a Santorini
      arm returning the existing `UnsupportedReason::FamilyNotImplemented`, with the doc comment recording why no new
      reason is published and what the seam becomes in slice 3 (design D8) — construct it from
      `FaceRecoveryParams`, and move `Self::FaceRecovery(_)` out of `Operation::pipeline`'s six-family arm into it.
      Verify with tests that an Athens operation resolves to a pipeline, that a Santorini one is still refused before
      anything is installed, and that the six remaining families are unchanged.

## 4. The measured provider tuning

- [x] 4.1 Replace `FaceRecoveryVariant::profile`'s `EpProfile::default()` with a match reaching a `profile` function
      in `athens/mod.rs` — `CpuAndGpu` compute units at both precisions, sequential execution, `cuda_prefer_nhwc` at
      FP16 only — carrying `athens.go`'s prose: the 1.57x CoreML figure and the op mix behind it, the 10-14x Neural
      Engine tell, the -20.6%/-13.2% graph and -8.5%/-4.6% end-to-end execution-mode figures, **both** NHWC pairs
      labelled (graph -7.7%/+5.9%, end to end -4%/+8%, design D10), the CoreML execution-mode exception, and the two
      CUDA defaults that must not change. Santorini stays on the defaults. Verify with tests mirroring
      `athens_test.go` that both precisions declare `CpuAndGpu` and sequential execution, that only FP16 declares
      NHWC, and that Santorini declares nothing at either precision.

## 5. The pipeline end to end

- [x] 5.1 Test `Athens` against a purpose-built fake backend — **no ONNX Runtime** — that records the weight it was
      given and returns a known tensor: a run over two faces performs two graph runs and composites both; the weight
      the graph receives is the operation's fidelity, and an operation carrying none receives `Fidelity::MAX`; an
      empty selection runs no graph, reports 1.0 and returns the source at the requested depth; a zero-area image is
      refused; progress never decreases, reaches 1.0 once and reports two steps per face; a cancellation at each of
      the three points produces no image at all; and a run at `ChannelDepth::Sixteen` returns a 16-bit image.

## 6. The benchmark harness stops timing a no-op

- [x] 6.1 Add the auxiliary detection pass to `crates/perf`: after the picture is loaded and before the first model
      is measured, and **only** when a face-recovery row was selected, run `Detection::newyork` at the sweep's
      precision through `Opai::execute`, and rebuild each face-recovery row over the faces it found through
      `FaceRecovery::athens`/`santorini` with the values the row already carries (design D11). Keep the failure
      rather than discarding it. Verify with tests that a sweep selecting no face-recovery row makes no detection
      pass, that two face-recovery rows share one pass, and that the rebuilt operation carries the faces and the same
      fidelity the row was selected with.
- [x] 6.2 Report what the pass found and fail the rows that depended on it: the face count joins the conditions in
      the report header, and a detection failure — or a photograph with no face in it — fails every face-recovery row
      with that reason rather than measuring it. Verify with tests that the header states the count, that a failed or
      empty detection fails exactly those rows and no others, that the sweep continues and reports the rest, and that
      the time taken by the pass appears in no model's measurement.

## 7. Verification

- [x] 7.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` and verify
      all three are clean, then `cargo doc -p opai --no-deps` and verify no new rustdoc warning against `HEAD`.
      Verify the doctrine holds: no `use` in `models/face_recovery/athens/` reaches into another model's directory,
      `inference/` gained no file any model or family owns, and `models/mod.rs`'s list of promotion candidates
      parked at model tier names this slice's four movable files. No frontend scripts — this change touches no
      frontend file.
