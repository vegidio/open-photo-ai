## MODIFIED Requirements

### Requirement: Progress is shown while the command works

Everything the command shows SHALL be written to standard error. Standard output SHALL stay empty.

When standard error is a terminal, the command SHALL show, in this order:

1. **A dependency line**, only if initialization downloads or expands at least one dependency. It SHALL read
   `💾 Downloading dependency <name>   <percent>%`, with three spaces between the name and the percentage. The name
   SHALL be the dependency's name as the GUI shows it (for example `ONNX Runtime`, `NVIDIA CUDA`, `NVIDIA TensorRT`).
   The percentage SHALL be a whole number covering that dependency's whole install, download and expansion together,
   and SHALL NOT show `100` before that install has finished. The line SHALL be rewritten in place as the install
   progresses, and when the next dependency starts, its name and percentage SHALL replace the previous ones. When
   initialization ends, the line SHALL remain as it was last drawn, at the last dependency's `100%`. A dependency that
   was already installed SHALL NOT be shown. Where every dependency was already installed, there SHALL be no dependency
   line.
2. A blank line.
3. **A live view of the batch**, drawn in place for the whole run, whatever the number of images, one included. It
   SHALL hold:
   1. **The current image's progress bar.** It SHALL cover that image's face detection, when there is one, and every
      operation of its chain, and it SHALL move forward once from the start to the end. It SHALL NOT refill between
      the detection and the chain or between operations. When the next image starts, the bar SHALL start again from
      empty, without first moving backwards. The bar SHALL be labelled with what the image's run is doing, in the
      GUI's words:
      - `Enhancing` before the image's run has reported anything
      - `Downloading` while an operation's model is being fetched, without a percentage of its own
      - otherwise the enhancement's name (`Face Recovery`, `Upscale`); the face detection reads as `Face Recovery`

      The label SHALL be padded so that the bar does not move sideways when the label changes. After the label, the
      line SHALL show a step counter `[n/N]`. `N` is the number of enhancements the image's run will apply. `n` is `0`
      before the run has reported anything, and afterwards the step being worked on, counted from one in the order the
      enhancements are applied. The face detection counts as the face recovery's step. The line SHALL end with the
      progress of the image's run as a percentage with one decimal place, which SHALL NOT show `100.0%` before that
      run has finished. The line SHALL NOT show an estimated time remaining. While an image with no enhancement left to
      run is processed, because the only one requested was an upscale skipped as unnecessary, the bar's row SHALL be
      blank.
   2. A blank row.
   3. **The list.** One row per image started so far, in the order the images run, showing only the five most recent.
      Images not yet started SHALL NOT be listed. The view SHALL take two rows more than the list can hold: five list
      rows for a run of five or more images, otherwise one per image. Each row SHALL name the image's path as it was
      given or found, and its position `[n/N]`, where `n` is the image's place in the run, counted from one, and `N`
      the number of images in the run. `n` SHALL be padded to the width of `N`. A row SHALL read:
      - `🖼️ [n/N] Enhancing image <path>` while the image is loaded, enhanced and written
      - `🖼️ [n/N] Enhanced image <path>` once its result is written
      - `🧨 [n/N] Failed image <path>` once it has failed
      - `🧨 [n/N] Stopped image <path>` when the run was stopped while working on it
4. A blank line.
5. **The summary**, below the view, which SHALL remain as it was last drawn:
   1. When at least one image was written, a success line. If exactly one image was written, it SHALL read
      `✨ Image enhanced successfully; file saved at <file>`, naming the file actually written. If more than one was
      written, it SHALL read `✨ <count> images enhanced successfully; files saved under <output>`, naming the output
      directory as it was given.
   2. One failure line, `🧨 <reason>`, for each image that failed, in the order they ran. Each reason SHALL name the
      image.
   3. The closing failure line, when the run did not end in success (see "Exit status says how the run ended").

On a colour terminal, the dependency's name SHALL be yellow and its percentage bold white. The bar SHALL be drawn in a
colour gradient. In a `🖼️` row, the `n/N` SHALL be blue, the word `image` magenta, and the path cyan and underlined. A
`🧨` row SHALL be red throughout. The success line SHALL be bold yellow throughout. When the `NO_COLOR` environment
variable is set to a non-empty value, the same lines SHALL be drawn without colour.

When standard error is not a terminal, the command SHALL print, without colour, without a dependency line, view or
progress bar, and without any cursor-movement or redraw sequences:

1. One line for each image, when that image ends, with the words its row ends with in the view. A failed image's line
   SHALL add `: <reason>` after the path.
2. The summary's success line, when at least one image was written.
3. The closing failure line, when the run did not end in success.

The failure lines of the view's summary SHALL NOT be repeated in this mode, since each image's own line already
carries its reason.

When standard error is a terminal that cannot draw the live view, the command SHALL print the lines it would print
when standard error is not a terminal, coloured as on a colour terminal.

The command SHALL run on the best execution provider this machine supports. It SHALL record in its log file, for each
image, the path written, the size and format of the image written, and the provider or providers the run used. If
every step was served from the run cache, it SHALL record that instead. If no enhancement was left to run, it SHALL
record that no enhancement ran, rather than that the run was served from the cache. None of this SHALL be printed to
the terminal.

#### Scenario: First run on a new machine

- **WHEN** the command runs where the runtime is not yet installed, on a terminal
- **THEN** the first line shows `💾 Downloading dependency ONNX Runtime` with a rising percentage
- **AND** when initialization ends, that line remains at `100%`, and the view follows it after a blank line

#### Scenario: Several dependencies downloaded

- **WHEN** initialization downloads the runtime and then a GPU library, on a terminal
- **THEN** the single dependency line names the runtime with its percentage, and is then rewritten to name the GPU
  library with its own percentage

#### Scenario: Everything already installed

- **WHEN** the command runs where every dependency is already installed, on a terminal
- **THEN** no dependency line is shown, and the view is the first thing drawn

#### Scenario: Face recovery and upscale

- **WHEN** the user runs `opai photo.jpg --fr --up -o out.png` on a terminal, and `out.png` does not exist
- **THEN** the view is three rows tall: the bar, a blank row, and `🖼️ [1/1] Enhancing image photo.jpg`
- **AND** the bar is labelled `Face Recovery` with the counter `[1/2]` while the faces are found and restored, then
  `Upscale` with `[2/2]`, its progress never decreases, and it ends at `100.0%`
- **AND** the row then reads `🖼️ [1/1] Enhanced image photo.jpg`
- **AND** the run ends with `✨ Image enhanced successfully; file saved at out.png`

#### Scenario: A numbered result is named

- **WHEN** `out.png` already exists and the user runs `opai photo.jpg --up -o out.png` on a terminal
- **THEN** the run ends with `✨ Image enhanced successfully; file saved at out_1.png`

#### Scenario: The list scrolls

- **WHEN** a batch of eight images is run on a terminal, and the seventh is being enhanced
- **THEN** the list shows the rows of images 3 to 7, `[3/8]` to `[7/8]`, the first four of them `Enhanced` and the
  last `Enhancing`, and no row for image 8

#### Scenario: The bar starts again for each image

- **WHEN** the first of two images finishes and the second starts, on a terminal
- **THEN** the bar reads `Enhancing` with `[0/N]` at `0.0%` for the second image, without having moved backwards
  from `100.0%`

#### Scenario: A batch that succeeds

- **WHEN** the user runs `opai photos --up -o out`, and `photos` holds five images, all written
- **THEN** the run ends with `✨ 5 images enhanced successfully; files saved under out`

#### Scenario: A failure in a batch

- **WHEN** a batch of three images is run on a terminal, and the second fails while being written
- **THEN** its row reads `🧨 [2/3] Failed image <path>` in red, and the third image still runs
- **AND** below the view, the run prints `✨ 2 images enhanced successfully; files saved under <output>`, then
  `🧨 <reason>` naming the second image, then `🧨 1 of 3 images failed`

#### Scenario: A failure that scrolled out of the list

- **WHEN** the first of eight images fails, on a terminal
- **THEN** its row is no longer shown once the sixth image starts
- **AND** its failure line is still printed below the view when the run ends

#### Scenario: Model downloaded during the run

- **WHEN** a requested model is not yet on disk
- **THEN** the bar is labelled `Downloading` while the model is fetched, keeping that operation's step counter, and
  then takes the operation's own name when it runs

#### Scenario: Nothing printed outside the layout

- **WHEN** a `--fr --up` run finds faces and writes its output on a terminal
- **THEN** the terminal shows no face count, no provider, no image size and no format
- **AND** the log file records them, with the path written

#### Scenario: Output piped to a file

- **WHEN** standard error is redirected to a file, and a batch of three images runs where the second fails
- **THEN** the file holds `🖼️ [1/3] Enhanced image <path>`, `🧨 [2/3] Failed image <path>: <reason>`,
  `🖼️ [3/3] Enhanced image <path>`, the success line, and `🧨 1 of 3 images failed`, in that order
- **AND** it holds no colour, no dependency line, no progress bar and no redraw sequences

#### Scenario: Colours turned off

- **WHEN** the command runs on a terminal with `NO_COLOR=1`
- **THEN** the dependency line, the view and the summary are drawn as usual, without colour

### Requirement: Exit status says how the run ended

The command SHALL exit with:

- `0` when every image was written
- `2` for a usage error: bad arguments, an unreadable named input, an output that cannot be used as asked, or no
  image to enhance
- `130` when stopped by Ctrl-C
- `1` for any other failure: initialization, or any image that failed to decode, enhance or be written

Every failure SHALL end with one failure line on standard error, `🧨 <reason>`, where the reason is a short message
saying what went wrong. On a colour terminal the failure line SHALL be bold red throughout. This applies to every
failure the command reports itself, including refusals of the inputs or output, another instance holding the claim,
and a stop by Ctrl-C. Argument errors detected while parsing the command line, and `--help` and `--version`, SHALL be
printed as the argument parser prints them, with no failure line.

When images failed and the run was not stopped, the closing failure line SHALL be
`🧨 <failed> of <total> images failed` for a run of more than one image. For a run of one image, it SHALL be that
image's own failure line, naming it, instead of a count, and the summary SHALL NOT list that failure a second time.
When the run was stopped, the closing failure line SHALL say it was stopped, whatever failed before.

#### Scenario: Success

- **WHEN** a run writes every output
- **THEN** the exit status is 0

#### Scenario: Write fails

- **WHEN** the output file's directory does not exist
- **THEN** the command ends with a failure line `🧨 <reason>` naming the output path, and exits with status 1
- **AND** no `1 of 1 images failed` line is printed

#### Scenario: Unreadable input

- **WHEN** the user runs `opai missing.jpg --up -o out.png` and `missing.jpg` does not exist
- **THEN** the command prints a single failure line `🧨 <reason>` naming `missing.jpg`, and exits with status 2

#### Scenario: One image of several fails

- **WHEN** a batch of three images is run and the second fails to be written
- **THEN** the second's failure is reported, the third is still enhanced and written, and the command ends with
  `🧨 1 of 3 images failed` and exits with status 1

#### Scenario: Stopped after a failure

- **WHEN** the first of five images fails, and the user presses Ctrl-C while the third is being enhanced
- **THEN** the first image's failure line is printed, the command ends with its failure line saying it was stopped,
  and it exits with status 130

#### Scenario: Unknown flag

- **WHEN** the user runs `opai photo.jpg --up --vignette -o out.png`
- **THEN** the argument parser's own error is printed, with no failure line, and the exit status is 2
