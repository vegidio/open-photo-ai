## 1. The vocabulary, and the rule that keeps it honest

- [x] 1.1 Create `crates/opai/src/autopilot.rs` with `Suggestion` — `FaceRecovery` and `Upscale { scale: Scale }` —
      its `family()` accessor, and `Suggestions { suggested, incomplete }`, per design.md's first two decisions.
      Declare `mod autopilot;` in `lib.rs` beside `mod cache;` and re-export both types. Write the doc comments to
      say why an arm carries a parameter only where the *image* determines one, and why `incomplete` is a field
      rather than an error — the `ProviderReport`-on-`Enhanced` reasoning, that an outcome invisible in the result is
      one nobody can ask about. Verify `cargo build -p opai --all-targets` is clean.
- [x] 1.2 Hold the spec's **"Nothing is suggested that the application cannot run"** with a test rather than with
      discipline: for every `Suggestion` arm, build that family's operation and assert
      `Operation::pipeline::<NoBackend>()` resolves it. Write the test's comment to say what it is for — an arm added
      for a family whose pipeline is still `FamilyNotImplemented` fails here, which is what keeps the rule true as
      families land. Verify it passes today and fails if a `Denoise` arm is added temporarily.

## 2. The two signals

- [x] 2.1 Write the geometry signal and the scale ladder — at most 1,048,576 px suggests 4x, at most 4,194,304 px
      suggests 2x, larger suggests nothing — with the thresholds as named constants carrying the reference's split
      across `shouldUpscale` and `defaultUpscaleScale`, and why it is one ladder here. Verify with tests at **all
      four** boundaries the spec pins: exactly 1,048,576 gives 4x and not 2x, 1,048,577 gives 2x, exactly 4,194,304
      gives 2x, and 4,194,305 gives nothing.
- [x] 2.2 Write the face signal over `inference::execute::execute` with
      `Detection::newyork(FloatPrecision::Fp32)`, passing the caller's options through. Document the FP32 choice
      with the reference's own reasoning, transcribed rather than summarised: a suggestion is accepted at its model's
      HD tier, so any other precision installs a second detection graph to re-answer an answered question. Classify
      its failure per design.md — `Cancelled` and `Shutdown` propagate, everything else becomes `incomplete` — in
      **one** match, so the two dispositions cannot drift apart. Verify against `NoBackend` that a run finding faces
      suggests face recovery, one finding none does not and is not a failure, and one whose model cannot be opened
      returns no face-recovery suggestion with `incomplete` set and **no** error.

## 3. The analysis

- [x] 3.1 Write `suggest<B: Backend>(backend, cache, source, options)` as the body, generic in the backend for the
      reason `inference::execute::execute` is: everything it decides is then checked against a fake session on a
      runner with no runtime. Evaluate cheapest first — area, geometry, then the face signal — so the no-area case
      returns before anything is installed. Verify against `NoBackend` that an image with no area produces an empty
      `Suggestions` with `incomplete` unset and acquires **nothing**, asserted against the fake's acquisition log
      rather than against the result, and that a run cancelled while the face signal is in flight returns
      `Err(InferenceError::Cancelled)` carrying no suggestions at all — not the geometry one it had already
      determined.
- [x] 3.2 Emit suggestions in `Family::ALL` order, and pin it with a test that sorts the produced families by their
      position in `Family::ALL` and compares against what came back, rather than asserting the literal pair. Write
      the test's comment to say why: the ordering is a property of the family, so a signal added later is placed by
      the one list rather than by where its push was written. Verify an image that is both small and has faces comes
      back face recovery first, then upscale.
- [x] 3.3 Say what `Family::ALL` is and is not, since the ordering rule rests on it: it is the only canonical family
      order the crate has, **not** "the order the enhancement path applies families" — the chain applies operations
      in caller order and names no family order, and `Family::ALL`'s own doc says it is not a presentation order.
      Pin the divergence from the reference's one present-and-apply list (`['dn','fr','cl','la','cb','sh','up']`) as
      a **checked fact** rather than a comment: assert it is colorization's position alone, and that the families a
      suggestion can name today agree exactly. Verify the test fails when `Family::ALL` is reordered to match.
- [x] 3.4 Cover the two store scenarios end to end rather than by construction, by giving this module's fake backend
      a graph that answers. Promote `newyork`'s named-output fixture to a `pub(crate)` `answer_graph` beside the
      model — not into `inference::test_support`, which holds only what no model owns — and have both suites call
      it, so the two cannot disagree about that graph's shape. Verify against an **initially empty** store that a
      second analysis of the same photograph acquires nothing, and that an `execute` of the same detection after an
      analysis acquires nothing and returns the face it found. Verify each fails under mutation, and that the
      `newyork` suite passes unchanged.
- [x] 3.5 Add `Opai::suggest(&self, source, options) -> Result<Suggestions, InferenceError>` in `lib.rs` beside
      `process` and `execute`, forwarding the store handle as `execute` does, and write the module header for
      `autopilot.rs`: what an analysis is, why it stops at the family, why it owns no session — `execute` acquires
      and drops within the call, so `release_sessions` has nothing outstanding from an analysis — and which signals
      are deliberately absent until their families have pipelines. Verify `cargo doc -p opai --no-deps` produces no
      new rustdoc warning against `HEAD`, and that the rustdoc example compiles.

## 4. The record

- [x] 4.1 Record what an analysis concluded at the default verbosity, naming the families suggested, and record a
      signal that could not be read with the signal's name and its reason — the spec's **"An analysis says what it
      suggested"**. Use the field style `inference::execute` already writes so one log reads the same for both paths.
      Verify with a capturing subscriber that a completed analysis writes the conclusion at `info`, that an
      incomplete one writes both records, and that a run producing no suggestions still writes one — an analysis that
      concluded nothing and an analysis that never ran must not look alike in the file.

## 5. Verification

- [x] 5.1 Pin the new public surface in `crates/opai/tests/public_api.rs` the way `Opai::execute`, `ExecuteOptions`
      and `Executed` are pinned there: `Suggestion`'s arms named in a construction, `Suggestions`' two fields read
      back, and `suggest` named at its signature. Verify `cargo test -p opai --test public_api` passes with the
      existing entries unchanged.
- [x] 5.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` and verify
      all three are clean. Verify `inference::execute`'s own tests pass with **no edit to any of them**, which is
      what makes the face signal a caller of that path rather than a change to it. No frontend scripts — this change
      touches no frontend file, and the GUI wiring is its own change.
