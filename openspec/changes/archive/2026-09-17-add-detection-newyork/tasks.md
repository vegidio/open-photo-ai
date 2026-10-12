## 1. The pipeline seam splits

- [x] 1.1 In `inference/pipeline.rs`, split `Pipeline<B>` into the three traits design.md D1 spells: `Model<B>`
      carrying `required` and `sessions`, `ImagePipeline<B>: Model<B>` carrying `stages` and the image `run`, and
      `DataPipeline<B>: Model<B>` carrying `type Output` and a `run` with **no** `depth` parameter. Add
      `SharedData<B, T>` beside `Shared<B>`. Repoint the two existing implementations — `PassSequence` in
      `models/upscale/passes.rs:65` and `Diffusion` in `models/upscale/osaka/mod.rs:191` — splitting each into a
      `Model<B>` impl and an `ImagePipeline<B>` impl with no body edited. Rewrite the module header to describe three traits
      rather than one, keeping its "two contracts" account of why `sessions` answers as pairs. Verify
      `cargo build -p opai --all-targets` is clean, that `Model<B>` is object-safe by naming a `&dyn Model<B>` in the
      file, and that every upscale test passes unedited.
- [x] 1.2 Lift the acquisition block out of `chain::run` into `pipeline::acquire` with the signature design.md D2
      gives — the install reporter over `required()`, then one `backend.acquire` per `(artifact, profile)` pair from
      `sessions()`, returning the handles — and call it from `chain::run` in place of the lifted lines. **No
      behaviour changes and the `spawn_blocking` hop stays in `chain::run`.** Verify by running
      `cargo test -p opai inference::chain` and confirming **every** existing chain test passes with **no edit to any
      test**, particularly the ones covering the cache rewind over a prefix that would not decode, the three progress
      weightings, the refusal-before-install ordering, what a cancelled run returns, and that every session is held
      until the chain finishes. A test that needs editing means the lift was not a lift — stop and re-scope.

## 2. A graph with more than one output

- [x] 2.1 Add `NamedOutput<'a> { name, buffer }` and the pure `accept_named` checker to `inference/session.rs`
      beside `accept`, then `session::run_named_outputs` over them and `Backend::run_named_outputs` on the trait,
      addressing outputs **by name** for the reason design.md D5 gives. Implement it on `Opai` and on all four test
      doubles (`inference/test_support.rs`, `chain.rs`, `models/upscale/osaka/mod.rs`,
      `models/upscale/osaka/region.rs`). Write the doc comment to say why this one spells tensor names when the other
      two deliberately do not. Verify `accept_named`'s three failures — a named output the graph does not declare, a
      named output of the wrong length, and a caller buffer that disagrees — are each covered by a test that runs with
      **no ONNX Runtime loaded**, and that each writes nothing at all, including to the outputs that were present.

## 3. The execute path

- [x] 3.1 Add the sealed `sealed::DataModel` trait and the public `DataOperation` in `models/operation.rs` per
      design.md D3, with `type Output`, `as_operation` and the generic `pipeline<B>`. Implement it for `Detection`
      with `Output = Faces`, its `pipeline` returning `Err(Unsupported)` for now so the trait compiles ahead of task
      5.5. Document on `DataOperation` that this crate is the only implementor and what an implementing type provides.
      Verify `cargo build -p opai` is clean and that a test asserting `<Detection as DataOperation>::Output` is
      `Faces` compiles — and that an attempt to implement `DataOperation` outside the crate does not, checked by a
      `compile_fail` doctest.
- [x] 3.2 Add `ExecuteOptions` and `Executed<T>` and write the driver in a new `inference/execute.rs`: the `info`
      log bracket matching `chain::process`'s fields, `ChainProgress::new(on_progress, 1)`, `pipeline::acquire`, the
      `spawn_blocking` hop, the handles held until return, and the `ProviderReport` folded out of them. **No
      `RunCache` is constructed and no depth is resolved.** Write the module header stating what it deliberately does
      not do and why, as `chain.rs`'s does. Verify it builds and that `grep -n "RunCache\|ChannelDepth\|identity_after"
      crates/opai/src/inference/execute.rs` returns nothing.
- [x] 3.3 Add `Opai::execute<O: DataOperation>` delegating to the driver, export `ExecuteOptions`, `Executed` and
      `DataOperation` from `lib.rs`, and pin the new surface in `crates/opai/tests/public_api.rs` the way
      `Opai::process` is pinned there — through a function pointer that fixes the exact signature, plus construction
      and destructuring of `Executed`. Verify `cargo test -p opai --test public_api` passes and that `Opai::process`,
      `ProcessOptions` and `Enhanced` are unchanged in that file.

## 4. The refusal stops being a deferral

- [x] 4.1 Add the `UnsupportedReason` variant saying the operation's result is not an image and naming `execute` as
      where it belongs, and move `Operation::pipeline`'s `Self::Detection(_)` out of the seven-family arm into its
      own returning that reason. Keep the remaining six on `FamilyNotImplemented`. Verify a test asserts the two
      refusals are distinguishable **without reading the error text** — one match arm each — and that
      `chain::plan` still refuses a chain containing a detection operation before anything is installed.

## 5. New York

- [x] 5.1 Write `models/detection/newyork/input.rs`: fit the image to 640 preserving aspect using
      `image::imageops::resize` with `Lanczos3`, place it at the top-left of the square, fill the remainder with the
      negative per-channel means, and write BGR planar CHW `f32` minus (104, 117, 123). Return the tensor beside the
      **integer** resize dimensions the rescale in 5.4 needs. Refuse a zero-area image rather than falling back to the
      square, which is the reference's defensive path rather than its intent. Verify with tests that a landscape, a
      portrait and a square image each produce a `3 * 640 * 640` tensor whose padding region is exactly the negative
      means, that the channel order is BGR by feeding a known solid colour, and that a zero-area image is refused.
- [x] 5.2 Write `models/detection/newyork/anchors.rs` generating the priors for 640 — strides 8/16/32 over 80x80,
      40x40 and 20x20 with min sizes {16,32}, {64,128} and {256,512}, normalised over the image size, in the
      reference's order. Verify a test asserts the count is exactly **16,800**, that the first and last prior's
      centres and extents match values transcribed from the reference's `anchors_test.go`, and that the ordering is
      level-major then row-major then size, since the decode indexes into the model's outputs by this position.
- [x] 5.3 Write `models/detection/newyork/decode.rs`: one anchor's box from the four `loc` values (variance 0.1 on
      the centre offsets, 0.2 on the exponentiated extents) and its five landmarks from the ten `landmarks` values
      (variance 0.1), both **per anchor** rather than over the whole array, for the reason design.md D6 records.
      Verify with tests transcribed from the reference's `decode_test.go` that a zero-offset anchor decodes back to
      the prior itself and that a known non-zero offset decodes to the reference's values.
- [x] 5.4 Write `models/detection/newyork/filter.rs`: keep anchors whose `conf[i*2 + 1]` is strictly greater than
      0.5, decode only those, scale by 640, suppress overlaps at IoU strictly greater than 0.4 with the Pascal-VOC
      `+1` on both the areas and the intersection, and rescale into the source's coordinate space using the integer
      resize dimensions from 5.1. Sort by confidence with a **stable** sort so equal scores keep anchor order. Verify
      with tests transcribed from `filter_test.go` that four overlapping candidates for one face collapse to the
      highest-scoring one, that two candidates overlapping less than the threshold both survive, that no candidate
      above the threshold yields an empty set rather than an error, that coordinates are not clamped to the image
      bounds, and that the returned order is descending confidence.
- [x] 5.5 Write `models/detection/newyork/mod.rs`: the `TARGET_SIZE` constant, the `DataPipeline<B, Output = Faces>`
      implementation running 5.1, one `run_named_outputs` call for `loc`/`conf`/`landmarks`, and 5.2–5.4, reporting
      progress at 0.2, 0.6 and 1.0 and checking cancellation at the same three points; plus the `Model<B>` impl
      naming the single artifact and its profile. Point `Detection`'s `sealed::DataModel::pipeline` from 3.1 at it,
      completing the family's contract seam. Verify with a purpose-built fake backend — **no ONNX Runtime** — that
      returns synthetic `loc`/`conf`/`landmarks` tensors: a run over a known image produces the expected faces, a
      cancellation at each of the three points produces no faces, and the three progress fractions are reported in
      order.

## 6. The measured provider tuning

- [x] 6.1 Replace `DetectionVariant::profile`'s `EpProfile::default()` with New York's measured settings —
      `cuda_prefer_nhwc` and sequential execution at **both** precisions, CoreML left on its defaults — carrying the
      reference's prose: the -17.5%/-35.1% graph figures, the -11.6%/-23.4% end-to-end figures, why the NHWC answer
      here is the opposite of Athens', why CoreML's defaults are a measurement rather than an oversight, and the two
      CUDA defaults that must not change. Verify with tests mirroring the reference's `newyork_test.go` that both
      precisions declare NHWC and sequential execution, and that both leave CoreML's compute units and specialization
      at their defaults.

## 7. Verification

- [x] 7.1 Test the execute driver against the fake backend for the properties `data-inference`'s requirements name,
      none of which need a runtime: progress never decreases and reaches 1.0; a model already on disk does not give
      up the head of its range; a stopped run produces no result; a failed run produces no result rather than an
      empty one; the provider report names what was asked for beside what was built, and says nothing was executed
      when no session was built; and the image cache is neither read nor written — verified by driving the run with a
      populated store and asserting no access, not merely by asserting the result.
- [x] 7.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` and verify
      all three are clean, then `cargo doc -p opai --no-deps` and verify no new rustdoc warning against `HEAD`.
      Verify the doctrine holds: no `use` in `models/detection/newyork/` reaches into another model's directory, and
      `inference/` gained no file any model or family owns. No frontend scripts — this change touches no frontend
      file.
