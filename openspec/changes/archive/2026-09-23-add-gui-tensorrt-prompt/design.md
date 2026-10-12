## Context

Everything the dialog reads already exists (see proposal.md - Why):

- `useSetupStore` holds `status` (`idle | running | ready | failed`) and, once `initialize` resolves, `providers` - the
  `SupportedProviders` report, with `tensorrt` set only when an RTX-branded NVIDIA adapter was found *and* TensorRT was
  installed for this platform.
- `useSettingsStore` is persisted under `settings-storage`, the Wails application's key. Its data is `SettingsData`;
  `apply` is documented as the only writer and is called by the Settings dialog's Save. `partialize` persists exactly
  `settingsData(state)`, and `merge` repairs each field on rehydrate.
- `dialogs.tensorRT.*` (`title`, `logoAlt`, `intro`, `optimization` with `<b>`, `question`, `footer` with `<b>`/`<u>`)
  and `common.yes`/`common.no` are already in all thirteen locales.
- The Wails application persisted `isFirstTensorRT: boolean` in the same `settings-storage` blob, `false` once answered.

## Goals / Non-Goals

**Goals:**

- Screen 03 as the design draws it, shown by the rule in the `gui-setup` delta.
- The answer and the "asked" flag written in one store update, so no state exists in which one is recorded without
  the other.

**Non-Goals:**

- Mapping the Wails application's processor spellings (`"CUDA"`, `"TensorRT"`) on rehydrate. The existing repair turns
  them into `auto`; changing that is a `gui-settings` repair question, not this dialog's.
- Any Rust change. Detection is `opai`'s, already reported by `initialize`.
- Analytics. The reference tracks `TensorRtPromptAnswered`; nothing in this application sends analytics yet.

## Decisions

### The flag lives in the settings store, outside `SettingsData`

`tensorrtAsked: boolean` is a field of the settings store's state but not of `SettingsData`. `partialize` persists
`{ ...settingsData(state), tensorrtAsked }`; `merge` reads it as `stored.tensorrtAsked === true ||
stored.isFirstTensorRT === false`, defaulting to `false`.

- *Why the settings store:* it is the blob the Wails application wrote its flag into, so the upgrade path is one
  expression in `merge`, and the flag is written in the same `set` as the processor it accompanies.
- *Why not in `SettingsData`:* `SettingsData` is what the Settings dialog drafts, restores and resets. The flag in
  there would be drafted by a dialog that never shows it, and `settingsDefaults()` - which Reset to defaults reads -
  would put it back to `false`.
- *Alternative - its own `localStorage` key:* cleanest separation, but it cannot see the Wails application's flag
  without reading another store's blob, and the answer and the flag would be two writes to two keys.

`settingsData()` is unchanged, so Save, Cancel and Reset never touch the flag: Save's `apply(draft)` spreads over the
existing state and the flag is not in the draft.

### One action writes the answer: `answerTensorRT(enable: boolean)`

It sets `processor` to `"auto"` or `"cuda"` and `tensorrtAsked` to `true` in one `set`. It is the store's second
writer, and the store's documentation of `apply` as the only one is updated to name it. Calling `apply({ processor })`
followed by a flag setter was rejected: two writes, two persists, and a window where the processor is saved and the
flag is not.

### Whether the dialog is up is derived, not held

`App` renders `<TensorRTDialog />` when `status === "ready" && providers?.tensorrt && !tensorrtAsked`. There is no
`useState` for "open": answering flips `tensorrtAsked`, and the condition takes the dialog down. This also gives the
setup ordering for free - `ready` is only reached once the setup dialog is gone - and a successful retry reaches
`ready` the same way a first attempt does.

`processor: "cuda"` needs no guard for a machine offering TensorRT without CUDA: TensorRT is installed as a layer on
the CUDA libraries, so a report with `tensorrt` has `cuda` too. If one ever did not, the existing Performance-page rule
already reads an unoffered processor as the automatic choice.

### The dialog cannot be dismissed without an answer

Built on the existing shadcn `dialog`, as `SetupDialog` is: `onEscapeKeyDown`, `onPointerDownOutside` and
`onInteractOutside` all prevented - the three ways Radix closes a dialog, each turned off explicitly as every other
dialog here does - and no close control in the title bar. `DialogTitleBar` draws its close control unconditionally, so
it gains a `showCloseButton` prop (default `true`), named as `DialogContent` names the same switch. The reference behaves the same way - it passes no `onClose`, so neither Escape nor
the backdrop closes it.

### Layout follows screen 03

512px wide; a 48px title bar; a 24px-padded column with a 24px gap holding the logo at 96px tall, three centred 13px
paragraphs 12px apart (the emphasis in the second is `font-bold text-white` through `<Trans>`'s `components`, as in
the reference), the two buttons at `min-w-36` with 12px between them - No secondary, Yes primary - and the footnote as
a 12px muted caption. The logo is copied from the reference's `src/assets/logo_tensorrt.avif` into this frontend and
imported, which Vite already handles.

### The fake is one widened report, in one place

For the look-over, `succeeded` is fed `{ ...providers, cuda: true, tensorrt: true }` and the dialog condition drops
`!tensorrtAsked`, both marked `// FAKE(tensorrt-prompt)` so removal is one grep. Widening the report rather than
forcing the dialog open means the Performance page also offers TensorRT and CUDA, so the result of each answer can be
seen there. The answer still writes the real store; removing the fake leaves whatever was last answered, which the
user resets from Settings.

## Division across crates

`gui` only, frontend only. `opai` already owns detection and reports it; `cli` has no first launch and takes its
provider from a flag. **rust-sak:** not used - there is no Rust in this change. **New crates or npm packages:** none.

## Risks / Trade-offs

- [The fake left in by mistake would show every user the dialog on every launch, and point macOS users at CUDA] →
  it is the last task, marked `FAKE(tensorrt-prompt)`, and `grep -rn "FAKE(tensorrt-prompt)" crates/gui/frontend`
  returning nothing is a precondition for archiving.
- [While the fake is in, answering No on a Mac sends `cuda` to every inference command] → the library falls back to
  the CPU for an unsupported provider; the user resets the processor to Auto after looking.
- [An upgrading user who answered No in the Wails application silently becomes Auto] → accepted; see Non-Goals.

## Validation

Validated in the Vite/Vitest environment and on macOS (Apple Silicon) with the fake in place. **Untested on real
TensorRT hardware** - Windows or Linux with an RTX card - where the report's `tensorrt` is genuine; the condition reads
only that boolean, so the fake exercises the same path.
