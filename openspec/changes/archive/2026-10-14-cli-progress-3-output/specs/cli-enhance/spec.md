## MODIFIED Requirements

### Requirement: Progress is shown while the command works

Everything the command shows SHALL be written to standard error. Standard output SHALL stay empty.

When standard error is a terminal, the command SHALL show, in this order:

1. **A dependency line**, only if initialization downloads or expands at least one dependency. It SHALL read
   `💾 Downloading dependency <name>... <mark> <percent>% Complete`. The name SHALL be the dependency's name as the GUI
   shows it (for example `ONNX Runtime`, `NVIDIA CUDA`, `NVIDIA TensorRT`). The percentage SHALL be a whole number
   covering that dependency's whole install, download and expansion together, and SHALL NOT show `100` before that
   install has finished. The mark SHALL be a braille spinner that keeps turning while the line is drawn, whether or
   not the percentage changes, and SHALL be `✓` while the percentage reads `100`. The line SHALL be rewritten in place
   as the install progresses, and when the next dependency starts, its name, spinner and percentage SHALL replace the
   previous ones. When initialization ends, the line SHALL remain as it was last drawn, which after a finished install
   is the last dependency's `✓ 100% Complete`. A dependency that was already installed SHALL NOT be shown. Where every
   dependency was already installed, there SHALL be no dependency line.
2. **The processor line**, once initialization has finished and the processor was accepted:
   `🧠 Running inference using the processor <processor>; quality <quality>`. The processor SHALL be the one asked for
   with `--processor`, spelled `Auto`, `CPU`, `CoreML`, `CUDA`, `TensorRT` or `WebGPU`; under `auto` it SHALL read
   `Auto`, not the provider the run resolves to. The quality SHALL be `HD` or `SD`.
3. A blank line.
4. **A live view of the batch**, drawn in place for the whole run, whatever the number of images, one included. It
   SHALL hold:
   1. **The batch's progress bar.** It SHALL be labelled `Enhancing` for the whole run. After the label, the line SHALL
      show a counter `[n/N]`, where `N` is the number of images in the run and `n` the number of images that have
      ended, written or failed, with `n` zero-padded to the width of `N`. The bar SHALL fill as
      `(n + p) / N`, where `p` is the progress of the image being worked on as its row shows it (see below), so that
      it moves forward within each image and never moves backwards, including when one image ends and the next
      starts. The line SHALL end with that progress as a percentage with one decimal place, which SHALL NOT show
      `100.0%` before the last image has ended. After the percentage, the line SHALL show an estimate of the time
      the batch has left, `ETA <time>`, at the rate the batch has run so far since its view started, kept current
      while the view is drawn even when no progress is reported. It SHALL read `ETA --` until at least 1% of the
      batch has run.
   2. A blank row.
   3. **The list.** One row per image started so far, in the order the images run, showing only the five most recent.
      Images not yet started SHALL NOT be listed. The view SHALL take two rows more than the list can hold: five list
      rows for a run of five or more images, otherwise one per image. Each row SHALL show the image's own progress as
      a whole percentage, right-aligned in three columns, as in `[  0%]`, `[ 42%]` and `[100%]`. The percentage SHALL
      cover the image's face detection, when there is one, and every operation of its chain, SHALL never decrease,
      and SHALL be held at `99%` until the image's result is written. A row SHALL read as follows, where `<path>` is
      the image's path as it was given or found:
      - `🖼️ [<p>%] Preparing to enhance <path>` before the image's run has reported anything, and for the whole time
        an image with no enhancement left to run is processed, because the only one requested was an upscale skipped
        as unnecessary
      - `🖼️ [<p>%] Downloading <family> for <path>` while an operation's model is being fetched
      - `🖼️ [<p>%] Applying <family> to <path>` while an operation runs or is served from the run cache, and after
        the last one until the result is written
      - `🖼️ [100%] Enhancements applied to <path>` once its result is written, whatever the number of enhancements,
        none included
      - `🧨 [<p>%] Failed to enhance <path>` once it has failed, keeping the percentage it had reached
      - `🧨 [<p>%] Stopped enhancing <path>` when the run was stopped while working on it, keeping the percentage it
        had reached

      The words between the percentage and the path SHALL be padded with spaces to the width of the longest wording,
      `Downloading Face Recovery for`, so that every row's path starts in the same column, and at least one space
      SHALL separate them from the path. The plain lines SHALL be padded the same way.

      `<family>` SHALL be the enhancement's name in the GUI's words: `Denoise`, `Face Recovery`, `Colorize`,
      `Light Adjust`, `Color Balance`, `Sharpen` or `Upscale`. The face detection SHALL read as `Face Recovery`.
5. A blank line.
6. **The summary**, below the view, which SHALL remain as it was last drawn:
   1. When at least one image was written, a success line. If exactly one image was written, it SHALL read
      `✨ Image enhanced successfully; file saved at <file>`, naming the file actually written. If more than one was
      written, it SHALL read `✨ <count> images enhanced successfully; files saved under <output>`, naming the output
      directory as it was given.
   2. One failure line, `🧨 <reason>`, for each image that failed, in the order they ran. Each reason SHALL name the
      image.
   3. The closing failure line, when the run did not end in success (see "Exit status says how the run ended").

On a colour terminal, the dependency's name and its percentage SHALL be bold white, the spinner magenta and the `✓`
green. In the processor line, the processor SHALL be green and the quality yellow. The bar SHALL be drawn in a colour
gradient. In a `🖼️` row, the percentage, its `%` sign included, SHALL be blue, the enhancement's name, or the word
`Enhancements`, magenta, and the path cyan and underlined. A `🧨` row SHALL be red throughout. The success line SHALL be bold yellow throughout.
When the `NO_COLOR` environment variable is set to a non-empty value, the same lines SHALL be drawn without colour.

When standard error is not a terminal, the command SHALL print, without colour, without a dependency line, view or
progress bar, and without any cursor-movement or redraw sequences:

1. The processor line, followed by a blank line.
2. One line for each image, when that image ends, with the words its row ends with in the view, its percentage
   included. A failed image's line SHALL add `: <reason>` after the path.
3. The summary's success line, when at least one image was written.
4. The closing failure line, when the run did not end in success.

The failure lines of the view's summary SHALL NOT be repeated in this mode, since each image's own line already
carries its reason.

When standard error is a terminal that cannot draw the live view, the command SHALL print the lines it would print
when standard error is not a terminal, coloured as on a colour terminal.

A processor refused by "The processor is chosen on the command line" SHALL end the command before the processor line
is printed.

The command SHALL run on the execution provider chosen with `--processor`, as "The processor is chosen on the
command line" describes. It SHALL record in its log file, for each image, the path written, the size and format of
the image written, and the provider or providers the run used. If every step was served from the run cache, it SHALL
record that instead. If no enhancement was left to run, it SHALL record that no enhancement ran, rather than that the
run was served from the cache. None of this SHALL be printed to the terminal; the processor line names only what was
asked for.

#### Scenario: First run on a new machine

- **WHEN** the command runs where the runtime is not yet installed, on a terminal
- **THEN** the first line shows `💾 Downloading dependency ONNX Runtime...` with a turning spinner and a rising
  `<n>% Complete`
- **AND** when initialization ends, that line remains as `💾 Downloading dependency ONNX Runtime... ✓ 100% Complete`,
  and the processor line follows directly under it

#### Scenario: Several dependencies downloaded

- **WHEN** initialization downloads the runtime and then a GPU library, on a terminal
- **THEN** the single dependency line names the runtime with its percentage, and is then rewritten to name the GPU
  library with its own percentage and a turning spinner

#### Scenario: Everything already installed

- **WHEN** the command runs where every dependency is already installed, on a terminal
- **THEN** no dependency line is shown, and the processor line is the first thing printed, followed by a blank line
  and the view

#### Scenario: The processor line names what was asked for

- **WHEN** the user runs `opai photo.jpg --up -q sd -o out.png` on a terminal
- **THEN** the processor line reads `🧠 Running inference using the processor Auto; quality SD`
- **AND WHEN** the user runs `opai photo.jpg --up -p cpu -o out.png`
- **THEN** it reads `🧠 Running inference using the processor CPU; quality HD`

#### Scenario: A refused processor prints no processor line

- **WHEN** the user asks for a processor this machine does not offer
- **THEN** the command ends with its failure line, and no processor line is printed

#### Scenario: Face recovery and upscale

- **WHEN** the user runs `opai photo.jpg --fr --up -o out.png` on a terminal, and `out.png` does not exist
- **THEN** the view is three rows tall: the bar, a blank row, and the image's row
- **AND** the bar is labelled `Enhancing` with the counter `[0/1]`, and the row reads
  `🖼️ [  0%] Preparing to enhance photo.jpg`
- **AND** the row reads `Applying Face Recovery to photo.jpg` while the faces are found and restored, then
  `Applying Upscale to photo.jpg`, its percentage never decreasing and not passing `99%`
- **AND** once `out.png` is written, the row reads `🖼️ [100%] Enhancements applied to photo.jpg`, the
  bar reads `[1/1]` at `100.0%`
- **AND** the run ends with `✨ Image enhanced successfully; file saved at out.png`

#### Scenario: A numbered result is named

- **WHEN** `out.png` already exists and the user runs `opai photo.jpg --up -o out.png` on a terminal
- **THEN** the run ends with `✨ Image enhanced successfully; file saved at out_1.png`

#### Scenario: The list scrolls

- **WHEN** a batch of eight images is run on a terminal, and the seventh is being enhanced
- **THEN** the list shows the rows of images 3 to 7, the first four of them ended and the last with its own
  percentage, and no row for image 8
- **AND** the bar's counter reads `[6/8]`

#### Scenario: The bar fills across the batch

- **WHEN** a batch of four images is run on a terminal, and the third image's row reads `[ 50%]`
- **THEN** the bar's counter reads `[2/4]` and its percentage is `62.5%`
- **AND** when the third image ends and the fourth starts, the bar does not move backwards

#### Scenario: The bar starts again for each image

- **WHEN** the first of two images finishes and the second starts, on a terminal
- **THEN** the bar does not start again: it reads `Enhancing` with `[1/2]` at `50.0%`, without having moved backwards
- **AND** the second image's row starts again from `🖼️ [  0%] Preparing to enhance <path>`

#### Scenario: An image with nothing to run

- **WHEN** the user runs `opai large.jpg --up -o out.png` on a terminal, and the upscale is skipped as
  unnecessary
- **THEN** the row reads `🖼️ [  0%] Preparing to enhance large.jpg` until the file is written
- **AND** then `🖼️ [100%] Enhancements applied to large.jpg`

#### Scenario: A batch that succeeds

- **WHEN** the user runs `opai photos --up -o out`, and `photos` holds five images, all written
- **THEN** the bar ends at `[5/5]` and `100.0%`, and the run ends with
  `✨ 5 images enhanced successfully; files saved under out`

#### Scenario: A failure in a batch

- **WHEN** a batch of three images is run on a terminal, and the second fails while being written
- **THEN** its row reads `🧨 [ 99%] Failed to enhance <path>` in red, the bar's counter counts it as
  ended, and the third image still runs
- **AND** below the view, the run prints `✨ 2 images enhanced successfully; files saved under <output>`, then
  `🧨 <reason>` naming the second image, then `🧨 1 of 3 images failed`

#### Scenario: A failure that scrolled out of the list

- **WHEN** the first of eight images fails, on a terminal
- **THEN** its row is no longer shown once the sixth image starts
- **AND** its failure line is still printed below the view when the run ends

#### Scenario: Model downloaded during the run

- **WHEN** a requested model is not yet on disk
- **THEN** the image's row reads `🖼️ [<p>%] Downloading <family> for <path>` while the model is fetched, and then
  `Applying <family> to <path>` when it runs

#### Scenario: Paths line up in the list

- **WHEN** the list shows a written image, a failed one and one being upscaled, on a terminal
- **THEN** the rows read, with every path starting in the same column:
  ```
  🖼️ [100%] Enhancements applied to       photos/c.jpg
  🧨 [ 42%] Failed to enhance             photos/e.jpg
  🖼️ [ 42%] Applying Upscale to           photos/g.jpg
  ```

#### Scenario: Nothing printed outside the layout

- **WHEN** a `--fr --up` run finds faces and writes its output on a terminal
- **THEN** the terminal shows no face count, no provider the run resolved to, no image size and no format
- **AND** the log file records them, with the path written

#### Scenario: Output piped to a file

- **WHEN** standard error is redirected to a file, and a batch of three images runs on `auto` at `hd` where the second
  fails at 42%
- **THEN** the file holds `🧠 Running inference using the processor Auto; quality HD`, a blank line,
  `🖼️ [100%] Enhancements applied to <path>`, `🧨 [ 42%] Failed to enhance <path>: <reason>`,
  `🖼️ [100%] Enhancements applied to <path>`, the success line, and `🧨 1 of 3 images failed`, in that
  order
- **AND** it holds no colour, no dependency line, no progress bar and no redraw sequences

#### Scenario: Colours turned off

- **WHEN** the command runs on a terminal with `NO_COLOR=1`
- **THEN** the dependency line, the processor line, the view and the summary are drawn as usual, without colour, and
  the spinner still turns
