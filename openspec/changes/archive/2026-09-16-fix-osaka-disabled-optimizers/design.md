## Context

See proposal.md — Why. What matters here is how narrow the defect is and how wide its blast radius was.

The whole thing is one character. `add-inference-2-providers` ported the reference's
`DisableOptimizers []string` faithfully as a `Vec<String>`, and joined it in `sessions/build.rs` with
`","` where the reference joins with `";"`. `ort`'s `with_disabled_optimizers` passes the string
through verbatim into `optimization.disable_specified_optimizers`, so what reached the runtime was one
name containing a comma. An unrecognised name is ignored rather than reported, so nothing disabled
anything and nothing said so.

That lay dormant until Osaka, because Osaka is the first model here to declare the field at all. Slice
1 then swept the setting — correctly refusing to take the reference on trust — but the separator was
hardcoded below the level it was sweeping, so the sweep varied order and spacing and never the thing
that was wrong. Every result was consistent with a comma-joined value being one unrecognised name;
slice 1 read it as the runtime accepting only one name.

The cost is not Osaka, which loads either way on `SimplifiedLayerNormFusion` alone. The cost is that
the wrong conclusion was written into three doc comments as measured fact and into
`execution-providers` as a requirement binding every model measured after it.

## Goals / Non-Goals

**Goals**

- Establish the separator by measurement, before any code is written against it.
- Restore `EpProfile`'s list and Osaka's two names, so its profile is the reference's field for field.
- Withdraw the requirement and the three comments that assert the limitation as fact.

**Non-Goals**

- Re-measuring anything else in Osaka's profile. The other six fields were transcribed correctly and
  are not in question — this change verified them against `profileFor` and `ditProfile` before
  proposing anything.
- Deciding whether `ReshapeFusion` is load-bearing. See D3.
- Any change to `add-upscale-osaka-2-pipeline`. The two are independent; see D5.
- Widening the field to the reference's other unused `EPProfile` members. Slice 1's D4 established
  that all seven are dead in the reference and none is added here either.

## Decisions

### D1. Measure the separator before writing anything against it

The precedent is slice 1's own tasks 1.1–1.2 and the reason it gave for them: the reference records
this setting as having once been *"briefly and wrongly declared fixed"* by checking it against a local
Python install rather than the bundled runtime. The same discipline applies to a claim now coming from
three directions that disagree — the reference says semicolon, `ort`'s doc comment says comma, and this
project's spec says neither is a list at all.

The instrument is the one slice 1 already found: Osaka's transformer fails to load on the CPU provider
unless `SimplifiedLayerNormFusion` is disabled, and an unrecognised name is silently ignored. So a pair
containing it either loads — in which case the separator parsed and both names were seen — or fails, in
which case it did not. That turns a silent setting into a binary observable, and it is why the pair is
measured rather than the single name:

```text
  ";"  →  "ReshapeFusion;SimplifiedLayerNormFusion"   expected LOAD
  ","  →  "ReshapeFusion,SimplifiedLayerNormFusion"   known    FAIL
```

**If the semicolon does not load**, the rest of this change does not happen: the finding stands, the
requirement stays, and the work reduces to correcting the *reasoning* in the comments — because even
then "the runtime takes a single name" would remain unproven, and what would actually be established is
only that neither of two separators works. That distinction is the whole lesson here and the comments
should carry it rather than a second confident claim.

Measured through this project's own binding against the pinned ONNX Runtime 1.26, as slice 1's task 1.2
insisted for the same reason: what a local install does is not evidence about what the application does.

**Measured.** Nine values through this crate's own session build, Osaka's FP16 transformer on the CPU
provider, ONNX Runtime 1.26 on an M2 Max. The table this change is authorised by, and the one slice 1's
sweep could not have produced:

```text
  (unset)                                                        FAIL
  "SimplifiedLayerNormFusion"                                    LOAD
  "ReshapeFusion"                                                FAIL
  "ReshapeFusion,SimplifiedLayerNormFusion"                      FAIL
  "ReshapeFusion;SimplifiedLayerNormFusion"                      LOAD
  "SimplifiedLayerNormFusion;ReshapeFusion"                      LOAD
  "SimplifiedLayerNormFusion;NotATransformerAtAll"               LOAD
  "ReshapeFusion;SimplifiedLayerNormFusion;NotATransformerAtAll" LOAD
  "ReshapeFusion; SimplifiedLayerNormFusion"                     FAIL
```

Three things follow, and the last is the one that is easy to miss. **The separator is a semicolon**, and
order does not matter. **An unrecognised entry is dropped on its own** rather than costing the whole
value — the runtime reads the list, then ignores the names in it that match nothing. And **the entries
are not trimmed**: a space after the semicolon makes an entry that matches no transformer, so the graph
fails exactly as though nothing had been disabled. That last row is the failure mode in one line — a
value that looks right, disables nothing, and says nothing about it.

So the semicolon loads and the conditional above resolves the way D2, D3 and D4 are written for. The
table is kept as a test rather than only as a record — `the_disabled_optimizer_setting_is_a_semicolon_separated_list`
in `inference::osaka::live`, `#[ignore]`d for the weights it needs — so that a runtime version bump that
changes any row of it fails rather than going quiet.

### D2. The field goes back to a list, and the join moves next to the name

`EpProfile::disabled_optimizer: Option<String>` becomes a collection again, restoring what
`add-inference-2-providers` had before slice 1 narrowed it.

The join stays where it is, in `sessions/build.rs`, but the separator becomes a named constant beside
the field it serves rather than a literal inside a `.join()` call. That is the actual lesson of this
defect: the separator was invisible at every level that mattered. A caller reading `EpProfile` could
not see it, slice 1's sweep could not vary it, and the one place it appeared was a one-character string
in a method chain. Naming it, and stating in its doc comment that it is the runtime's contract and how
it was established, makes the next sweep able to find it.

**Alternative considered — keeping `Option<String>` and letting a model write its own separator.**
Rejected. It moves the problem into every model's declaration instead of solving it once, and makes a
wrongly joined value something a model is allowed to write rather than something the type prevents.
The whole reason this was silent is that a malformed value looks exactly like a well-formed one.

### D3. Both names are declared, and `ReshapeFusion` is not re-litigated

The reference declares both, and its comment describes both as miscompiling this graph. Slice 1
measured that `SimplifiedLayerNormFusion` alone is sufficient to load it, and that measurement is not
in doubt — it was made with a single name, which is the one case its hardcoded comma could express
correctly.

Sufficiency is not the test being applied, though. Matching the reference's configuration is, and
declaring a transformer that turns out to be unnecessary costs one fusion this graph was demonstrably
not relying on. Dropping it would mean this project's profile diverges from the reference's on the
strength of a measurement of a *different* question, which is how the current situation arose.

### D4. The requirement is removed, not softened

*A model disables at most one graph optimizer* is not a true statement made too strongly; it describes
a runtime that does not exist. Editing it in place would leave its scenarios and its reasoning as a
record of something that was never the case, and `MODIFIED` is for behaviour that changed rather than
for a finding that was wrong.

So it is `REMOVED`, with the reason and the migration stating what the evidence actually showed, and a
new requirement takes its place saying what is true: a model may disable several, and the separator is
the thing that has to be right. That new requirement deliberately carries the *silence* of the failure
rather than only the mechanism, because the silence is what let this run for three changes.

The two other requirements that slice 1 edited — *A model's measured settings include how the graph
itself is run* and *Osaka's measured settings* — are genuinely `MODIFIED`: they describe behaviour that
is changing, and both are carried at their full current text with the singular taken back out.

### D5. This lands before slice 2, but does not block it

`add-upscale-osaka-2-pipeline` is proposed and unapplied. It does not touch `EpProfile`, `variant.rs`
or `build.rs`, so there is no conflict in either direction and the two could archive in either order.

Taking this one first is still worth doing, for one reason: slice 2's deliverable includes an
end-to-end run on the real FP16 weights, and that run should exercise the configuration the application
ships rather than one already known to differ from the reference. If the order slips, nothing breaks —
Osaka loads under both declarations.

## Risks / Trade-offs

**[The semicolon does not load either, and the real answer is something else again]** → This is what
D1's measurement is for, and it is the first task rather than an assumption folded into the third. The
fallback is stated there: the change reduces to correcting the reasoning in the comments, and the
requirement stays until something is actually established. No code is written against the separator
before it is observed.

**[Restoring the pair breaks a model that loads today]** → The measurement is exactly this check, run
before the declaration changes, and slice 1's `#[ignore]`d live region test on the real weights is the
standing guard afterwards. The failure mode is loud — session creation fails — rather than a wrong
image, which is the one kind of failure this setting cannot cause.

**[`ReshapeFusion` turns out to cost something on another provider]** → Unmeasured here and accepted,
as it is in the reference. It is one fusion, this graph does not need it to load, and the whole profile
is transcribed rather than re-derived — the same standing caveat slice 1 recorded for every figure in
it. A future sweep that finds a cost has a named constant and a spec requirement to change rather than
a literal buried in a method chain.

**[Other models measured while the requirement stood were shaped by it]** → Checked: none. Osaka is the
only model in the project that declares this field at all, so the requirement constrained nothing else
in practice. It is being withdrawn before it does.

## Migration Plan

None. `EpProfile` is `pub(crate)` and no consumer configures a session, so no public signature changes;
nothing here is persisted, serialized or cached; and no model's *output* changes, since a disabled
fusion changes how a graph is compiled rather than what it computes. The observable difference is
confined to which transformers the runtime skips while building the session.

## How the work divides

**`opai`** — all of it. Session configuration is library-internal.

**`gui` and `cli`** — nothing. No user-visible behaviour changes, so no component, no i18n key and no
Tauri command is touched. `perf` is unaffected.

## Crates and rust-sak

**No crate is added, and `rust-sak` has no bearing on this.** The defect is in this project's own
porting of the reference's session configuration, not in infrastructure: there is no download, hash,
extraction, cache, codec or hardware probe anywhere near it, so no `rust-sak` module applies and its
enabled feature set is unchanged.

## Validated against

**Platforms:** the defect and the fix are platform-independent — a session config entry reaching the
runtime the same way everywhere. The measurement runs on macOS arm64, which is the machine this change
has.

**Execution providers:** the CPU provider, which is where the failure this setting exists for actually
occurs — the node the rewrite invents belongs to that provider's FP16 cast insertion, and slice 1
recorded that CoreML opens the graph either way. **CUDA and TensorRT are untested by this change**, and
nothing in it is provider-specific: this is a session-level setting that reaches every provider.
