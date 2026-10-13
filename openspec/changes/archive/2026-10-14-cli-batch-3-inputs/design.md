## Context

See proposal.md - Why. These facts from the code shape the design:

- **The CLI is one image, end to end.** `args::Args` has `input: PathBuf` and `output: PathBuf`. `run::run` resolves
  the output format (`output::resolve`), probes the input's header (`output::check_input`), plans the chain for that
  size (`chain::plan(&args, w, h)`), initializes `Opai`, loads, enhances and calls `image::save` straight to
  `args.output`. That last call is why an existing file is silently overwritten today.
- **`opai::export` (slice 2) is everything naming needs.** `FormatChoice` parses `preserve`/`jpeg`/… (exact,
  lowercase only). `destination_for(source, naming, folder)` composes the name and folder without touching the disk.
  `claim(requested, overwrite)` is blocking and returns a `Claim` whose `path()` is written and whose `commit()` keeps
  it. A dropped `Claim` gives its name back. `claim` refuses a missing directory (`NotFound`) rather than creating one.
- **`rust_sak::fs::list_path(dir, &ListOptions)`** walks one level or recursively, filters by extension
  case-insensitively, sorts by name within each directory, does not follow links, fails on a non-directory, and
  **fails the whole call** on an entry it cannot read. It has no way to prune a subtree. `opai` already enables the
  `fs` feature at the pinned tag.
- **`opai::image::input_extensions()`** is the list of extensions the application opens, RAW included. The GUI's
  picker and drop check already use it.
- **`progress::View`'s bar slot starts at most once** (`Slot::attempted`), so a second image's bar would not draw.
- **Nothing else in the CLI assumes one image.** The `CancellationToken`, the Ctrl-C handler and the `Opai` handle
  can all be shared across images as they are.

## Slices

CLI batch input is delivered as four changes. **This change is change 3.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-batch-1-live-rows` | In `rust-sak`: a `LiveLine` height, so a `Live` session draws more than one row; restoring below the whole viewport; a release (the `26.10.6` tag moved). Here: every `rust-sak` pin moved to it | - | **DONE** (archived 2026-10-13) |
| 2 | `cli-batch-2-export-names` | In `opai`: a public `export` module with the format choice (named, or Preserve with camera RAW falling back to TIFF), each format's extension and encoder settings, composing the name and the destination, and claiming a free name (`_1` to `_999`) or overwriting through a hidden file renamed over the target. `gui` adopts it with no change in behaviour | - | **DONE** (archived 2026-10-13) |
| 3 | `cli-batch-3-inputs` | In `cli`: inputs that are any mix of files and directories, with `--recursive`; wildcards expanded on Windows (`wild`); duplicate inputs removed; the output directory skipped by the walk; `-o` read as a file (writable image extension, one input only) or a directory (created); the output tree mirroring each directory input, with named files flat; `--format`, `--prefix` and `--suffix` (only when `-o` is a directory); numbering by default and `--overwrite`, including in place; the pre-flight check (a named file that cannot be read refuses the run, a discovered one is skipped); a batch continuing past a failure with a non-zero exit; one `Opai` for the whole batch; the `cli-enhance` spec delta for inputs and outputs | 2 | **CURRENT** |
| 4 | `cli-batch-4-list-view` | In `cli`: the chain bar with a five-row scrolling list under it, in UMD's colours (`🖼️ [n/N] Enhancing / Enhanced image <path>`, `🧨 … Failed image` in red), used for every run including `[1/1]`; the end summary; one plain line per image when stderr is not a terminal; the `cli-enhance` spec delta for progress | 1, 3 | PENDING |

## Goals / Non-Goals

**Goals:**

- Every refusal that can be made without the library is made before `Opai::initialize`, so a typo in the fortieth
  path never costs a download.
- The CLI writes through `opai::export` and nothing else, so a name the CLI writes is the name the GUI would write.
- The project is in a working, testable state after this slice: batches run end to end with the interim output, and
  slice 4 replaces only the display.

**Non-Goals:**

- Any change to the progress requirement or to how a single image looks while it runs. Slice 4 does that.
- Running images concurrently. One `Opai` serves one image at a time, and the model registry's leases already keep
  the sessions warm between images.
- Following links into directories during a walk.

## How the work divides

| Crate | Owns after this change |
|---|---|
| `opai` | Unchanged. It already owns everything that decides a file: format, name, destination, claim and commit (slice 2), the extensions it opens, probing, loading, enhancing and saving |
| `cli` | The arguments; expanding inputs (walk, recursion, deduplication, skipping the output directory); reading `-o` as a file or a directory; mirroring a directory input under `-o`; the batch loop and its exit status |
| `gui` | Unchanged |

Expanding and mirroring stay in `cli` because they describe the command line's arguments, not an export rule. The GUI
neither walks directories nor mirrors trees: its queue is the files the user opened, written flat into one folder or
beside each source. If the GUI ever accepts a dropped folder, the walk moves to `opai` in that change, ahead of the GUI
using it, as the project's promotion rule requires.

## `rust-sak` coverage

| Need | Covered by | Decision |
|---|---|---|
| Walking a directory, recursive or not, filtered by extension, deterministic order, no link following | `fs::list_path` + `ListOptions` | Used. The CLI adds `fs` to its `rust-sak` features. Same tag, already compiled for `opai` |
| Skipping the output directory during a walk | nothing: `list_path` cannot prune | Filtered after the walk (D4). Not worth a `rust-sak` release now: the cost is walking a directory that is usually small. Revisit (a `ListOptions::skip` predicate) if a large output tree inside the input proves slow |
| Tolerating an unreadable subdirectory | nothing: `list_path` fails the whole call | Refuse the run naming it (D3). Changing `list_path`'s all-or-nothing contract is a `rust-sak` design question with no second consumer asking for it |
| Windows glob expansion of `argv` | nothing | `wild` (below) |
| Name, format, claim, overwrite | `opai::export` (slice 2) | Used |

**Third-party crate: `wild` 2.2.x.** On Windows it expands `*`, `?` and `[…]` in `std::env::args_os()` the way a Unix
shell would. Elsewhere it returns the arguments unchanged. It is small, has no dependencies on non-Windows targets, and
is the usual way Rust CLIs get Unix-style wildcards on Windows. **Alternative rejected: the `glob` crate on each input
in our own code.** We would have to decide which arguments are patterns (a file may contain `[`), handle quoting
ourselves, and do it on Unix too, where the shell has already expanded and a second expansion would be wrong. `wild`
does the expansion where `cmd`/PowerShell leave it undone, and only there.

## Decisions

### D1. The arguments

```rust
pub struct Args {
    #[arg(value_name = "INPUT", required = true, num_args = 1..)]
    pub inputs: Vec<PathBuf>,
    // … enhancement flags unchanged …
    #[arg(long, short = 'r')]                       pub recursive: bool,
    #[arg(long, value_name = "FORMAT", value_parser = parse_format)] pub format: Option<FormatChoice>,
    #[arg(long, value_name = "TEXT")]               pub prefix: Option<String>,
    #[arg(long, value_name = "TEXT")]               pub suffix: Option<String>,
    #[arg(long)]                                    pub overwrite: bool,
    #[arg(long, short = 'o', value_name = "OUTPUT")] pub output: PathBuf,
}
```

- **The inputs stay first and positional.** A bare enhancement flag still takes the next word as its model, so
  `opai --up a.jpg` keeps failing as it does today, and the existing test for it stays.
- **`--format`, `--prefix` and `--suffix` are `Option`s**, so "given with an output file" can be refused (D5) rather
  than silently ignored. Their defaults (Preserve, empty) are applied when the naming is built.
- **`parse_format`** lowercases the word, maps `jpg`→`jpeg`, `tif`→`tiff`, `heif`→`heic`, then calls `opai`'s
  `FormatChoice::from_str`. Its error lists `preserve` and the eight names. The aliases are the CLI's: slice 2 kept
  `FormatChoice` to the one spelling the GUI stores, and widening `opai`'s parser would make the GUI's wire accept
  words its window never sends.
- **`args::parse` reads `wild::args_os()`** instead of `std::env::args_os()`. `parse_from` is unchanged, so the tests
  keep passing word lists.

### D2. Expanding the inputs: `inputs.rs`

```rust
pub struct Source { pub path: PathBuf, pub root: Option<PathBuf>, pub width: u32, pub height: u32 }
// root: the directory input it was found under (mirroring); None for a named file.

pub fn expand(inputs: &[PathBuf], recursive: bool, skip: Option<&Path>) -> Result<Vec<Source>, CliError>;
```

For each input in order:

1. `std::fs::metadata` (following a link the user named). Missing or unreadable → `Usage` naming it.
2. **A file** is probed with the existing `check_input` (moved here from `output.rs`). A failed probe → `Usage`.
3. **A directory** is listed with `list_path(dir, ListOptions::new().recursive(r).extensions(input_extensions()))`.
   A listing error → `Usage` naming the directory. Each file found is probed. A failed probe is **skipped** with a
   `tracing::warn!` naming it and the reason.
4. **Deduplication** keeps a `HashSet` of `std::fs::canonicalize(path)`, falling back to the path as given if
   canonicalizing fails, and drops any later occurrence. Canonical paths make `./a.jpg`, `a.jpg` and a path through a
   linked parent the same image. Hard links to one file are not detected: they are distinct paths the user may want
   as distinct outputs.
5. An empty result → `Usage("no image to enhance in …")`.

The probe happens here, once per image, and gives `chain::plan` its dimensions, as `run` does today. Probing reads only
headers, which is the cost of guaranteeing that a doomed path is caught before the download.

**Alternative rejected: probe lazily, as each image comes up.** It would let a bad named file through to after
initialization, which the spec forbids.

### D3. An unreadable subdirectory refuses the run

`list_path` is all-or-nothing. Rather than reimplementing the walk to skip what it cannot read, the run is refused
naming the directory. The user named the tree, and an incomplete listing of it is a surprise better reported up front
than discovered in the output. The spec records this as "a directory input whose tree cannot be listed".

### D4. The output directory is filtered out of each listing

When `-o` is a directory, `expand` gets `skip = Some(canonicalize(out))` when `out` exists. A directory that does not
exist yet cannot hold earlier results, so there is nothing to skip. Every file found under a directory input whose
canonical path starts with `skip` is dropped before probing. Named files are not filtered: a user who names a file
inside `out` asked for it.

The skip applies only to a directory input that strictly contains `out`. When `out` is the input itself (in place),
or contains it, every image of that input lies under `skip`, and filtering would leave nothing to enhance. Such an
input is listed unfiltered.

### D5. Reading `-o`: `output.rs`

```rust
pub enum Target {
    File { path: PathBuf, format: ImageFormat },  // today's `Output` minus `options`
    Directory { path: PathBuf, naming: Naming },
}
pub fn target(args: &Args) -> Result<Target, CliError>;
```

In order: an existing directory → `Directory`. An extension `ImageFormat::from_path` knows → `File`. Otherwise →
`Directory`, and if something other than a directory exists there, `Usage`.

Then the combinations: `File` with more than one input, or with an input that is a directory → `Usage` ("an output
file takes exactly one input file"). `File` with any of `--format`/`--prefix`/`--suffix` → `Usage` naming the flag.
Both checks run before `expand`, so the cheap refusals do not wait for a walk.

Nothing is created here. The directory is created when the first result is written into it (D6), so a refused or
interrupted-at-init run leaves no empty directory behind.

### D6. Where each image goes, and how it is written

```rust
fn destination(target: &Target, source: &Source) -> (PathBuf, ImageFormat)
```

- **`File`**: the path as given, in the extension's format.
- **`Directory`**: `folder = out.join(source.path.strip_prefix(root).parent())` for a found image, `out` for a named
  one. Then `opai::export::destination_for(&source.path, naming, Some(&folder))`, and the format is
  `ImageFormat::from(named.format)`.

Writing replaces today's direct `image::save`:

```
create_dir_all(parent)        // Directory targets only; a File target's directory must exist, as today
claim = export::claim(dest, args.overwrite)
image::save(pixels, claim.path(), format, recommended_encode_options(format, None))
written = claim.commit()       // the path the success line names
```

- **The encoder settings stay `recommended_encode_options(format, None)`**, which is what
  `opai::export::encode_options(f, None)` gives. The spec says "the GUI's default quality", so no `--quality` for the
  encoder is added. `-q` stays the model precision.
- **In place is free.** With `-o` the input directory itself, the mirrored folder is the source's own directory.
  Without `--overwrite` the claim numbers past the source. With it, the claim writes a hidden file and renames it over
  the source only on commit. The source was fully loaded before enhancing, so replacing it is safe.
- **The claim is taken after enhancing, right before saving.** Taking it earlier would hold a placeholder on disk for
  the whole inference, visible to the user as an empty file.
- **`claim` is blocking and is called directly**, as slice 2's D4 planned for the CLI. It creates one file, and the
  runtime has nothing else to serve at that moment.
- **A write error names the path asked for.** Claim and commit failures are `io::Error`s without a path, so they are
  wrapped in a new `CliError::Claim { path, source }` whose message names `dest`.

### D7. The batch loop: `run.rs`

```
target(args) → expand(...) → initialize once → stop_on_ctrl_c
for (n, source) in sources:
    if cancel.is_cancelled(): break
    header(source.path); plan(args, w, h); load; enhance; write
    Ok  → success(written)
    Err(Cancelled) → stop the loop
    Err(e) → failure line naming source.path; failed += 1; continue
drop(opai)
```

- **One `Opai`, one `CancellationToken`, one Ctrl-C handler** for the whole batch. A Ctrl-C during an image's bar
  cancels the token: that image's `process` returns `Cancelled`, its `Claim` (if any) is dropped and gives its name
  back, and the loop stops. A Ctrl-C between images (no live line, so a SIGINT) is caught by the `is_cancelled` check
  at the top of the next iteration.
- **Per-image failures** (`load`, `enhance`, `write`) print `look::failure` with the image's path in the reason and
  are logged in full. `Init` and `Cancelled` end the run as today.
- **Exit status.** `run` returns `Ok(())` when nothing failed, `Err(CliError::Cancelled)` on a stop, and a new
  `Err(CliError::Batch { failed, total })` when one or more images failed. `Batch` exits `1`. Its line
  (`🧨 2 of 5 images failed`) is printed by `main` like every other error, after the per-image lines. A run of one
  image that fails prints its own line and then this one. That is redundant for one image, but it stays until slice
  4 replaces both with the summary.
- **A decode failure after a clean probe** is a per-image failure (exit 1), not a usage error (exit 2) as it is today
  for the single image. The probe is what decides "readable input", and a batch cannot exit 2 for its fifth image
  after writing four.

### D8. Interim display

Each image prints what a single-image run prints today: the header naming its path, the blank line, the bar, the
blank line, and its result line. The dependency line still appears at most once, before the first header: `finish_install` closes its slot for good,
so a report arriving after it starts nothing. To draw a
second bar, `View`'s bar slot is reset by `finish_bar`. A slot whose start the terminal **refused** stays refused, so
a terminal that cannot draw is not asked again for every image. Slice 4 removes this per-image layout, so no spec
requirement describes it beyond the existing progress requirement, which every image still meets.

## Risks / Trade-offs

- **[BREAKING: an existing `-o` file is no longer overwritten.]** Scripts that rerun `opai a.jpg --up -o out.png`
  now accumulate `out_1.png`, `out_2.png`, and so on. → The proposal and the spec delta mark it. `CLI_REFERENCE.md`
  states it in the `-o` and `--overwrite` sections and in the scripting section.
- **[A file whose extension names a format but that is meant as a directory.]** `-o backup.png` for a new directory
  is read as a file. → An existing directory always wins (D5). For a new one the user creates it first. The rule is
  documented.
- **[Probing a large tree before starting.]** Thousands of RAW headers add a noticeable pause before the first
  image. → Accepted for the up-front guarantee. Slice 4's list view does not change it. If it hurts, probe in
  parallel with `spawn_blocking`, with no change to behaviour.
- **[`canonicalize` on Windows returns `\\?\` paths.]** They are used only as deduplication keys and for the output
  directory skip, never shown or written, so the prefix is harmless as long as both sides of a comparison are
  canonical. They are, by construction.
- **[Wildcards are untested on Windows.]** CI runs the Rust tests on Linux only. `wild` is a no-op there, so the tests
  pass word lists to `parse_from` and never reach it. → `wild`'s expansion is checked by hand on Windows if a machine is
  available. Otherwise it is recorded as **untested** at archive. `wild` keeping a pattern that matches nothing as
  written, which the spec relies on, is checked against its docs and source when the dependency is added.
- **[Two inputs mirror onto the same path.]** `opai one two -o out` where both hold `a.jpg`. → Numbering gives
  `out/a.jpg` and `out/a_1.jpg`. With `--overwrite` the second replaces the first, which is what the user asked for.
  Documented in `CLI_REFERENCE.md`.

## Validation

To be validated on **macOS** (Apple Silicon): `cargo test -p cli`, the workspace's `fmt`/`clippy`/`test`, and a manual
pass with real models on the CoreML provider: a mixed file+directory batch, `-r` mirroring, in place with and without
`--overwrite`, a failing image mid-batch, Ctrl-C mid-batch, and piping stderr to a file. **Linux** is covered by CI's
Rust test job. **Windows** is built by CI, and its wildcard expansion and `\\?\` canonical paths are **untested** unless
checked by hand. Execution providers do not matter here: each image runs exactly the chain a single-image run does,
and only the CPU and CoreML paths are exercised by the manual pass.

## Migration Plan

There is no data migration. The one behaviour change users notice is numbering instead of overwriting (see Risks).
To roll back, revert the commit. `opai` and `gui` are untouched, so nothing else moves with it.
