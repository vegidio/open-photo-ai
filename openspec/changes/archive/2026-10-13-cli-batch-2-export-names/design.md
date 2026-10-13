## Context

See proposal.md - Why. These facts from the code shape the design:

- **The rule is split across three files and two languages.** `crates/gui/src/export/format.rs` owns the format enum
  (spelled as the window spells it, `heic` for HEIF), its extension, the capability table and the encoder settings.
  `crates/gui/src/export/destination.rs` owns `claim`/`Claim`. `crates/gui/frontend/lib/export.ts` composes the name
  and folder (`formatFor`, `extensionFor`, `exportNameFor`, `destinationFor`) by reading `preserves`/`fallback` off
  `export_formats`' answer.
- **Most of it already rests on `opai`.** `format.rs` delegates to `opai::image::{extensions_of, default_quality,
  recommended_encode_options, MIN_QUALITY, MAX_QUALITY}`. `destination.rs` uses only `std` and `tracing`, and has no
  GUI dependency at all.
- **The window composes the name in two places.** `batch.ts` sends `destinationFor(...)` and `formatFor(...)` to the
  `export` command. `ExportRow.tsx` shows `exportNameFor(...)` and `extensionFor(...)` live while the batch is idle,
  updating as the user types a prefix or suffix.
- **The `export` command addresses its source by identity.** `Opened::resolve(identity)` gives the source's path,
  which is all the naming rule needs. Today the window sends that same path's derived destination back.
- **A `write`/`unwritable` error already carries the destination asked for** (`ExportError::{Write, Unwritable}.path`),
  so the window does not need its own copy to describe a failure.
- **CI runs the Rust tests on Linux only.** macOS and Windows are built, not tested.

## Slices

CLI batch input is delivered as four changes. **This change is change 2.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-batch-1-live-rows` | In `rust-sak`: a `LiveLine` height, so a `Live` session draws more than one row; restoring below the whole viewport; a release (the `26.10.6` tag moved). Here: every `rust-sak` pin moved to it | - | **DONE** (archived 2026-10-13) |
| 2 | `cli-batch-2-export-names` | In `opai`: a public `export` module with the format choice (named, or Preserve with camera RAW falling back to TIFF), each format's extension and encoder settings, composing the name (prefix, stem, suffix, extension) and the destination (chosen folder or the source's own), and claiming a free name (`_1` to `_999`) or overwriting through a hidden file renamed over the target. Moved from `gui`'s `export/format.rs`, `export/destination.rs` and the frontend's `destinationFor` family. `gui` adopts it with no change in behaviour: `export` takes the naming instead of a destination, and a new `export_names` command feeds the rows | - | **CURRENT** |
| 3 | `cli-batch-3-inputs` | In `cli`: inputs that are any mix of files and directories, with `--recursive`; wildcards expanded on Windows (`wild`); duplicate inputs removed; the output directory skipped by the walk; `-o` read as a file (writable image extension, one input only) or a directory (created); the output tree mirroring each directory input, with named files flat; `--format`, `--prefix` and `--suffix` (only when `-o` is a directory); numbering by default and `--overwrite`, including in place; the pre-flight check (a named file that cannot be read refuses the run, a discovered one is skipped); a batch continuing past a failure with a non-zero exit; one `Opai` for the whole batch; the `cli-enhance` spec delta for inputs and outputs | 2 | PENDING |
| 4 | `cli-batch-4-list-view` | In `cli`: the chain bar with a five-row scrolling list under it, in UMD's colours (`🖼️ [n/N] Enhancing / Enhanced image <path>`, `🧨 … Failed image` in red), used for every run including `[1/1]`; the end summary; one plain line per image when stderr is not a terminal; the `cli-enhance` spec delta for progress | 1, 3 | PENDING |

Changes 1 and 2 are independent. Slice 1 landed first.

## Goals / Non-Goals

**Goals:**

- One implementation of the naming rule, in `opai`, that the GUI uses now and the CLI uses in slice 3.
- What a row shows is what the export asks for, because both come from the same function, called with the same
  naming and the same source path.
- No observable change in the GUI: the same names, folders, numbering, overwriting, errors and row text.

**Non-Goals:**

- Mirroring a directory tree, or checking that `-o` is a file or a directory. Those are the CLI's (slice 3). This
  slice names one source into one folder.
- Accepting other spellings (`jpg`, `heif`) as a format choice. The choice is spelled as the window spells it today.
  Whether the CLI's `--format` accepts aliases is decided in slice 3, on top of this parser.
- Slimming `export_formats`' answer (see D7).

## How the work divides

| Crate | Owns after this change |
|---|---|
| `opai` | Everything that decides a file: the format enum and the format choice with their one spelling, extensions, the capability table, encoder settings, name and destination composition, the claim and its commit |
| `gui` (Rust) | Tauri commands and wire shapes: `export` (now taking the naming), `export_formats`, the new `export_names`, running the claim off the runtime's threads, the `Written` record |
| `gui` (frontend) | Display only: asks `export_names` for what to show and sends the export settings' naming as they are stored |
| `cli` | Untouched. It uses `opai::export` in slice 3 |

## `rust-sak` coverage

| Need | Covered by | Decision |
|---|---|---|
| Image formats, extension aliases, encoder options | `rust_sak::image` (through `opai::image`) | Used, as today |
| Claiming a free numbered name; write-through-and-rename | nothing. `fs::mk_temp_file_in` creates files readable by their owner only | Local, in `opai`. See D4 |

No third-party crate is introduced.

## Decisions

### D1. One public module, `opai::export`, in three files

```
crates/opai/src/export/
  mod.rs     docs + re-exports
  format.rs  ExportFormat, FormatChoice, FormatCapability/QualityRange/ExportFormats, capabilities(), encode_options()
  name.rs    Naming, Named, name_for(), destination_for()
  claim.rs   Claim, claim()
```

`format.rs` and `claim.rs` are moved from `gui` with their tests, and only their visibility changes (`pub(crate)` →
`pub`). `name.rs` is new Rust, ported from `lib/export.ts`, and its tests are ported one-for-one from
`lib/export.test.ts` (`formatFor and extensionFor`, `destinationFor`). That way the move is checked against the
behaviour the TypeScript pinned, not against a fresh reading of it.

It goes beside `opai::image`, not inside it. `image` reads and writes one file at a path the caller names. Choosing
that path is an export policy that sits on top of it.

**Alternative rejected: `opai::image::naming`.** It would make `image`'s documented scope ("writes an image to a
destination the caller names") false, and the claim is not image I/O.

### D2. The format choice has one spelling, shared by the wire and a future flag

```rust
#[derive(Serialize, Deserialize)] #[serde(rename_all = "lowercase")]
pub enum ExportFormat { Bmp, Gif, Jpeg, Png, Tiff, Avif, Heic, Webp }

pub enum FormatChoice { Preserve, Format(ExportFormat) }
// Serialized as one lowercase string: "preserve" | "jpeg" | ...; FromStr/Display use the same spelling.
```

`FormatChoice` has hand-written `Serialize`/`Deserialize` over that string, implemented through `FromStr`/`Display`.
That is exactly what the window's `FormatChoice` type sends today, so the stored export settings cross unchanged.
Slice 3's `--format` parses with the same `FromStr`.

**Alternative rejected: a flat nine-variant enum.** It reads well on the wire, but every caller that needs a real
format would have to handle a `Preserve` arm that cannot occur once the format is resolved.

### D3. Naming is pure and works on `OsStr`

```rust
pub struct Naming { pub prefix: String, pub suffix: String, pub format: FormatChoice }
pub struct Named { pub name: OsString, pub format: ExportFormat, pub extension: String }

pub fn name_for(source: &Path, naming: &Naming) -> Named;
pub fn destination_for(source: &Path, naming: &Naming, folder: Option<&Path>) -> (PathBuf, Named);
```

- **No filesystem access.** It never stats the source or the folder, so it is cheap to call on every keystroke, and
  it cannot fail.
- **Stem** is `Path::file_stem`, so only the last extension is removed (`2026.09.24 beach.jpg` → `2026.09.24 beach`).
  The TypeScript cut the lowercase record extension's length off the name. For any name with an extension both give
  the same result, and the ported tests pin that.
- **Preserve** lowercases the source's extension and keeps it when `opai::image::extensions_of` lists it for some
  format. Otherwise it gives TIFF/`tiff`, as `export_formats` publishes. This is the rule `preserves`/`fallback`
  encoded, now applied where it is defined.
- **Folder:** `folder.unwrap_or(source.parent())` joined with `PathBuf::join`. A source that is a bare relative name
  (`beach.jpg`, the CLI's case) has an empty parent and gives a bare name.
- **OsString, not String**, so a non-UTF-8 source names its export correctly on disk. Only the GUI's wire converts it
  lossily for display, as `ImageRecord.path` already does.

### D4. The claim moves to `opai` unchanged, and not to `rust-sak`

`claim(requested, overwrite) -> io::Result<Claim>` and `Claim::{path, commit}` move with every test, including the
race, the panic, permissions and the `_999` refusal. They stay **blocking**. The GUI keeps calling it inside
`crate::task::spawn_blocking`. The CLI has no runtime to protect, so it calls it directly.

**Why not `rust-sak`:** "create the first free name, exclusively, and write through a hidden name before renaming"
is general, but the shape here is this application's convention: `_n` up to 999, `.name.opai-n`, and permissions
copied from the replaced file. `rust-sak`'s `fs` has no comparable primitive to extend. Promoting it would mean
designing a general API with no second consumer yet, and a `rust-sak` release in the middle of this series. Revisit
when UMD or MediaSim need the same thing.

### D5. `export` takes the naming, and works out the destination from the source it resolves

The command's `destination: PathBuf, format: ExportFormat` become one `naming: ExportNaming`:

```rust
#[derive(Deserialize)] #[serde(rename_all = "camelCase")]
struct ExportNaming { prefix: String, suffix: String, location: Option<PathBuf>, format: FormatChoice }
```

`Request` carries it. `export_with` already resolves the source path when it judges the chain (step 2). Step 8 runs
`opai::export::destination_for(source, naming, location)` and claims the result. That gives the format to encode and
the `path` that `ExportError::{Write, Unwritable}` names, so both keep their meaning ("the destination asked for").
The step order stays the same, and no step moves before the chain runs.

This is a change to the IPC wire only. The window is the only caller and changes in the same slice.

**Alternative rejected: keep `destination` + `format` on the wire and only move the code.** The window would still
need its own composer to fill them in, which is the copy this slice removes.

### D6. `export_names` gives the rows what they show

```rust
#[tauri::command]
fn export_names(paths: Vec<String>, naming: ExportNaming) -> Vec<ExportName>
// ExportName { path, name, destination, format, extension }, in the order asked
```

Synchronous (`traced_sync`), and it cannot fail. It takes **paths**, not identities, because it is pure: it opens
nothing and reveals nothing, so there is nothing to vet, and the queue's rows are already keyed by path. `export`
resolves the same path from the identity and calls the same `destination_for`, so a row's name is the name asked for.

**The frontend side:**

- `ipc/export.ts` gains `exportNames(paths, naming)`. `ExportRequest` loses `destination`/`format` and gains the
  naming.
- `features/export` gains a `useExportNames(queue, settings)` hook. It asks again whenever the queue, the prefix,
  suffix, location or format changes, and **discards an answer that is not the latest one asked for** (a sequence
  counter), so fast typing cannot leave a row showing an older name. Until the first answer arrives, rows render
  nothing, as they already do while waiting for `export_formats`. After that, a row keeps its current name until the
  newer one arrives, so typing does not make rows flicker.
- **No debounce.** One call covers the whole queue: one IPC round trip, plus string work that takes microseconds per
  path in Rust. Dropping stale answers handles ordering, and a debounce would make the names lag behind the typing.
- `batch.ts` asks `export_names` once at Save, with the settings the batch started with, and reads each file's
  `format` for the quality (`qualityFor`/`queueQualityFor` take a format instead of resolving one). It sends the
  naming to `export`. `describeExportError` takes a write's file and folder from the error's own `path`, not from a
  destination the window computed.
- `lib/export.ts` loses `formatFor`, `extensionFor`, `exportNameFor`, `destinationFor`, `stemOf`, `join` and their
  tests. `FORMAT_CHOICES`, `groupByChain` and `describeExportError` stay.

### D7. `export_formats`' answer is left as it is

After this change the window no longer reads `preserves` or `fallback`. They stay on the wire, with
`the_capabilities_cross_the_boundary_as_the_window_reads_them` still pinning them. They are true facts that `opai`
now applies itself, and removing them is a wire change this slice does not need. The quality ranges are still read.

## Risks / Trade-offs

- **[A subtle behaviour difference in the port.]** → The TypeScript tests move into Rust one-for-one (D1), and the
  existing `run.rs` export tests keep asserting the written path. The row test (`ExportRow.test.tsx`) keeps its
  expected name strings and stubs `export_names` with them.
- **[Windows paths.]** The TypeScript kept a folder's own separator (`D:\Exports\` + name). Rust joins with the
  platform's separator, which on Windows is `\`, so it gives the same result for a Windows folder. → This is
  **untested**: CI runs the Rust suite on Linux only, and no Windows machine is checked by hand in this slice. A
  `#[cfg(windows)]` test is added so that it runs wherever the suite is run on Windows.
- **[Typing is now an IPC round trip per keystroke.]** → One call per change covers the whole queue, and stale
  answers are dropped. If a very long queue ever lags, a debounce can be added inside the hook without touching Rust.
- **[A non-UTF-8 path shows lossily in the row.]** → It is written correctly on disk (D3). The display matches how
  `ImageRecord.path` already shows it.
- **[Moving `pub(crate)` items into a public API makes them part of `opai`'s surface.]** → `opai` is not published
  anywhere, and its only consumers are this repository's crates, which change with it.

## Validation

To be validated on **macOS** (Apple Silicon): `cargo test` for `opai` and `gui`, the frontend's vitest suite, and a
manual pass of the export dialog (rows, Preserve over RAW, numbering, overwrite). **Linux** is covered by CI's Rust and
frontend test jobs. **Windows** is built by CI but its path joining is untested (see Risks). No execution provider is
involved: naming and claiming run no model.

## Migration Plan

There is no data migration. Stored export settings (`prefix`, `suffix`, `location`, `format`) are what the `export`
command now takes, so they are sent as stored. To roll back, revert the commit. The wire and the frontend change
together, so no mixed version can exist in one build.
