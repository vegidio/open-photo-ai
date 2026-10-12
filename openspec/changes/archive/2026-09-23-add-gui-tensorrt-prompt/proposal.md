## Why

Screen 03 of the design, "First run - TensorRT detected", is the last first-run dialog the Rust GUI does not draw. On a
machine that supports TensorRT the automatic processor picks it first, and its first run of each model spends minutes
building an engine with nothing to say why. The reference asks the user once, up front, whether that trade is wanted;
without the question a first enhancement on an RTX card reads as a hang.

Now because everything it needs has landed: setup already reports which providers this machine offers, the processor
preference is stored and read by every inference command, and the dialog's copy is already translated in every locale.

## What Changes

- **A one-time TensorRT dialog.** Once setup has succeeded, on a machine whose report says it offers TensorRT, and
  only if the user has never answered, the application shows screen 03: the TensorRT logo, three paragraphs on what
  TensorRT is and what its first run costs, **No** and **Yes**, and a footnote pointing at Settings → AI processor.
- **Yes saves the automatic processor; No saves CUDA.** Either answer is a committed preference, remembered across
  restarts, exactly as if it had been saved from the Settings dialog.
- **The answer is remembered, and the question is not asked again.** A remembered flag beside the settings records
  that the question was answered. Resetting settings to their defaults does not clear it.
- **A user upgrading from the Wails application is not asked twice.** The flag that application stored under the same
  key is read as the answer.
- **The logo asset** is brought over from the reference.
- **A temporary fake**, for looking at the dialog on a machine without TensorRT: the provider report is widened with
  TensorRT and CUDA, and the remembered answer is ignored so the dialog appears on every launch. It is the last task,
  and it is removed before this change is archived.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-setup`: a requirement is added - a machine that supports TensorRT is asked once, after setup succeeds, whether
  to use it.
- `gui-settings`: a requirement is added - the TensorRT question's answer sets the processor preference, and the fact
  that it was answered is remembered apart from the preferences the settings surface holds, so resetting them does not
  ask again.

## Impact

**Crates: `gui` only**, and within it the frontend. No Tauri command is added or widened, no capability is granted, and
no `Cargo.toml` is touched. `opai` and `cli` are untouched: the detection the dialog reads is already `opai`'s answer,
reported through the existing `initialize` command. Whether to ask a user a question on first launch is a front end's
concern - the CLI takes its provider from a flag and has no first launch to ask on.

**rust-sak: nothing used and nothing missing.** There is no Rust and no infrastructure in this change.

**New npm dependencies: none.** The dialog is built from the existing `dialog` and `button` components.

**Task estimate: eight**, so one change rather than a series.

**Parity: two deliberate divergences from the reference's behaviour.**

- **Yes writes the automatic processor rather than leaving the preference alone.** On a first launch the two are the
  same thing - the reference's default is Auto. Writing it states the answer instead of depending on nothing having
  changed the preference first.
- **An upgrading user who answered No in the Wails application reads as Auto, not CUDA.** That application stored the
  processor as `"CUDA"`, which the existing rehydrate repair already replaces with the automatic choice, and the
  remembered answer means they are not asked again. Mapping the reference's spellings is a separate question about
  the processor repair as a whole, not this dialog's, so it is not taken on here.
