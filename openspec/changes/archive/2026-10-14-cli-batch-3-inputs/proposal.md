## Why

`opai` takes one photograph and writes one file, and it overwrites whatever is already at `-o`. The batch series lets
it take many files, wildcards and directories in one run. Slice 1 gave `rust-sak` a live session taller than one row.
Slice 2 moved the export naming rule (format choice, Preserve, prefix and suffix, numbering, overwriting through a
hidden file) into `opai::export`. This slice is where the CLI starts using them: it accepts many inputs, works out
where each result goes, and processes them all under one `Opai`. The list view stays in slice 4.

## What Changes

Task estimate: **12**. This change is slice 3 of the four-slice `cli-batch` series (see design.md - Slices).

**In `cli`:**

- **Many inputs.** `opai <INPUT>... [options] -o <OUTPUT>`: any mix of files and directories, at least one. A
  directory contributes the images directly inside it, or its whole tree with `--recursive` (`-r`). Only files whose
  extension the library opens (`opai::image::input_extensions`) are picked up from a directory. Other files in it are
  ignored.
- **Wildcards on Windows.** The arguments are read through `wild`, so `opai *.jpg …` expands in `cmd` and PowerShell
  as it already does in Unix shells. On other platforms `wild` passes the arguments through unchanged.
- **Duplicates removed.** A file given twice, or given by name and also found in a directory, is processed once, at
  its first occurrence.
- **The output directory is skipped by the walk.** `opai photos -r -o photos/enhanced` does not pick up its own
  earlier results.
- **`-o` is a file or a directory.** A path with an extension the library writes is a file. It is accepted only for a
  single named file input, and its format is the one the extension names, as today. Any other path is a directory.
  It is created if missing, and refused if a file already exists there.
- **The output tree mirrors each directory input.** A file found at `photos/2024/a.jpg` under the input `photos` is
  written to `<out>/2024/a.<ext>`. Files named on the command line go directly into `<out>`. With `-o` naming the
  input directory itself, each result lands beside its source ("in place").
- **`--format`, `--prefix`, `--suffix`**, accepted only when `-o` is a directory. `--format` takes `preserve` (the
  default) or one of the eight writable formats, with the common extension spellings (`jpg`, `tif`, `heif`) as
  aliases. Names are composed by `opai::export::destination_for`.
- **BREAKING - numbering by default.** An existing file at the destination is no longer overwritten. The result is
  written to the first free `_1` … `_999` name beside it, using `opai::export::claim`. This includes a named `-o`
  file. `--overwrite` replaces the destination instead, through a hidden file renamed over it once the write is
  complete, so a source enhanced in place is never left half-written.
- **A pre-flight check over every input**, before anything is installed. A named file or directory that does not
  exist or cannot be read refuses the whole run (exit 2). A file found in a directory whose header cannot be read is
  skipped and recorded in the log. A run left with no image refuses with a usage error.
- **One `Opai` for the whole batch.** Initialization, the dependency line and the single-instance claim happen once.
  The images are then enhanced one after another, each with the chain planned for its own size.
- **A batch continues past a failure.** An image that fails to decode, enhance or write gets its own failure line,
  and the next image runs. The run exits `1` if any image failed. Ctrl-C stops the whole batch, keeps the files
  already written and exits `130`.
- **Interim terminal output.** Until slice 4's list view, each image prints the existing header, bar and result line
  in turn. The success line names the path actually written, including its number.
- **Docs.** `docs/CLI_REFERENCE.md` and the binary's module docs describe the new synopsis, flags and output rules.

**Not in `opai` or `gui`:** neither changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-enhance`: the command takes many inputs and `--recursive`. `-o` becomes a file or a directory, with mirroring,
  `--format`, `--prefix` and `--suffix`. Numbering replaces the silent overwrite, and `--overwrite` brings it back
  safely. The pre-flight check distinguishes named from discovered inputs. A batch continues past a failure, and the
  exit status and Ctrl-C rules are restated for many images. The progress requirement is left to slice 4.

## Impact

**Crates touched:** `cli` only. Where this logic lives:

- **Naming, formats, numbering and overwriting** are `opai::export`'s (slice 2). The CLI only calls it, so the CLI
  and the GUI cannot drift.
- **Expanding inputs** (directories, recursion, deduplication, skipping the output directory) and **mirroring a
  directory input under `-o`** stay in `cli`. The GUI has no equivalent: it never walks a directory, and its export
  dialog writes every file flat into one folder or beside its source. These rules are about the command line's
  arguments. If the GUI ever accepts dropped folders, the walk moves to `opai` in that change, before the GUI uses it.

**`rust-sak`:** `fs::list_path` already walks a directory, one level or recursively, filtered by extension
case-insensitively, sorted, without following symbolic links. The CLI turns on `rust-sak`'s `fs` feature, which
`opai` already enables at the same tag, so nothing extra is compiled. It cannot prune a subtree, so the output
directory is filtered out of its results instead (see design.md). Nothing is added to `rust-sak`.

**Third-party crates introduced:** `wild` (2.2.x), the Windows glob expansion for `std::env::args_os`. `rust-sak` has
nothing comparable. See design.md.

**Not in scope:**

- The five-row scrolling list, its colours and wording, the end summary, and the one-line-per-image plain mode
  (slice 4).
- Concurrency between images. They run one after another.
- Changing any naming rule: numbering, `_999`, Preserve's TIFF fallback (slice 2 owns them).
- Making `-o` optional.
