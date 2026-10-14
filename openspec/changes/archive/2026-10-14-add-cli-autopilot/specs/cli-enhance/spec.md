## ADDED Requirements

### Requirement: Autopilot chooses each image's enhancements

With `--autopilot`, alias `--ap`, the command SHALL analyse each image after loading it, the same analysis the GUI's
Autopilot runs, and SHALL apply to that image the enhancements the analysis suggests for it, in the library's apply
order. Different images in one run MAY receive different enhancements.

`--autopilot` SHALL NOT be combined with any flag that chooses an enhancement or its parameters: `--upscale`,
`--face-recovery`, `--denoise`, `--sharpen`, `--colorization`, `--light-adjustment`, `--color-balance`, `--scale`,
`--dn-str`, `--sh-str`, `--la-bias` or `--cb-bias`. Giving any of them with `--autopilot` SHALL fail with a usage
error that names both flags and says they cannot be used together, and nothing SHALL be installed or written. Every
other option SHALL work with `--autopilot` as it does without it, and `--quality` SHALL select the precision of the
models autopilot runs as it does for the enhancement flags.

Each suggested enhancement SHALL run with its family's model as follows:

| Family | Environment variable | Default model |
|---|---|---|
| Denoise | `OPAI_AP_DENOISE` | Stockholm |
| Face recovery | `OPAI_AP_FACE` | Athens |
| Colorization | `OPAI_AP_COLORIZATION` | Delhi |
| Light adjustment | `OPAI_AP_LIGHT` | Paris |
| Colour balance | `OPAI_AP_COLOR` | Rio |
| Sharpen | `OPAI_AP_SHARPEN` | Moscow |
| Upscale | `OPAI_AP_UPSCALE` | Tokyo |

The default model SHALL be the one the family's enhancement flag runs when given with no model. Where the family's
variable is set to one of that family's model names, spelled as the enhancement flag accepts it and matched without
regard to case, that model SHALL run instead. Where it is set to `off`, matched without regard to case, that family
SHALL NOT be suggested for any image, and its signal SHALL NOT be read. A variable that is unset or set to the empty
string SHALL leave the default in place.

A denoise or sharpen SHALL run at strength 100, a light adjustment or colour balance at bias 50, and an upscale at the
scale the analysis suggests for the image.

These variables SHALL be read only by the command line application and only when `--autopilot` is given. Without
`--autopilot`, they SHALL have no effect and SHALL NOT be checked. The GUI SHALL NOT read them.

An image for which the analysis suggests nothing SHALL still be written, with its content unchanged, as an image whose
only requested enhancement was a skipped upscale is.

When a signal of the analysis cannot be read, such as the face detector failing to install or open, the image SHALL
NOT fail: the enhancements suggested by the signals that were read SHALL run, and the command SHALL record the failure
in its log file as a warning. A Ctrl-C during the analysis SHALL stop the run as described in "A run can be stopped".

The command SHALL record in its log file, for each image, which enhancements the analysis suggested. This SHALL NOT be
printed to the terminal beyond what the image's row already shows.

#### Scenario: Different images get different enhancements

- **WHEN** the user runs `opai photos --ap -o out`, and `photos` holds a small grainy photograph and a large sharp one
  with a colour cast
- **THEN** the first is denoised and upscaled, the second is colour-balanced, and both are written under `out`

#### Scenario: Default models

- **WHEN** the user runs `opai small.jpg --ap -o out.png` with no `OPAI_AP_*` variable set, and the analysis suggests
  an upscale
- **THEN** the upscale runs with Tokyo at FP32, at the suggested scale

#### Scenario: A model chosen through the environment

- **WHEN** the user runs `OPAI_AP_UPSCALE=Osaka opai small.jpg --ap -q sd -o out.png`, and the analysis suggests an
  upscale
- **THEN** the upscale runs with Osaka's INT8 build

#### Scenario: A family switched off

- **WHEN** the user runs `OPAI_AP_SHARPEN=off opai blurred.jpg --ap -o out.png`
- **THEN** no sharpen is applied, whatever the photograph's blur, and the other families are still suggested

#### Scenario: Combined with an enhancement flag

- **WHEN** the user runs `opai photo.jpg --ap --up -o out.png`
- **THEN** the command fails with a usage error saying `--autopilot` cannot be used with `--upscale`, and nothing is
  installed or written

#### Scenario: Combined with a parameter flag

- **WHEN** the user runs `opai photo.jpg --ap --dn-str 150 -o out.png`
- **THEN** the command fails with a usage error saying `--autopilot` cannot be used with `--dn-str`

#### Scenario: Variables ignored without autopilot

- **WHEN** the user runs `OPAI_AP_UPSCALE=paris opai photo.jpg --up -o out.png`
- **THEN** the variable is not checked, and the photograph is upscaled with Tokyo

#### Scenario: Nothing to do

- **WHEN** the user runs `opai large.jpg --ap -o out.png`, and the analysis suggests nothing
- **THEN** `out.png` is written with the photograph's content unchanged
- **AND** the command ends with its success line

#### Scenario: The face detector cannot be obtained

- **WHEN** the user runs `opai small-portrait.jpg --ap -o out.png`, and the face detection model cannot be installed
- **THEN** the image is still upscaled and written, without face recovery
- **AND** the log file records the failed detection as a warning

## MODIFIED Requirements

### Requirement: The command takes its inputs, its enhancements, and an output

The terminal application SHALL be invoked as `opai <input>... [options] --output <output>`. The inputs SHALL come
first: one or more paths, each a file or a directory, in any mix. The options SHALL be:

- `--upscale`, alias `--up`, with an optional model: `kyoto`, `osaka`, `tokyo` or `saitama`. With no model it SHALL
  mean `tokyo`.
- `--scale <factor>`: the upscale factor, from 1.0 to 8.0 inclusive. It SHALL be accepted only alongside `--upscale`.
- `--face-recovery`, alias `--fr`, with an optional model: `athens` or `santorini`. With no model it SHALL mean
  `athens`.
- `--denoise`, alias `--dn`, with an optional model: `stockholm`, `gothenburg` or `malmo`. With no model it SHALL mean
  `stockholm`.
- `--dn-str <strength>`: the denoise strength, a whole number from 1 to 300 inclusive, defaulting to 100. It SHALL be
  accepted only alongside `--denoise`.
- `--sharpen`, alias `--sh`, with an optional model: `moscow`, `petersburg` or `novgorod`. With no model it SHALL mean
  `moscow`.
- `--sh-str <strength>`: the sharpen strength, a whole number from 1 to 300 inclusive, defaulting to 100. It SHALL be
  accepted only alongside `--sharpen`.
- `--colorization`, alias `--cl`, with an optional model: `delhi`, `mumbai` or `jaipur`. With no model it SHALL mean
  `delhi`.
- `--light-adjustment`, alias `--la`, with an optional model: `paris` or `lyon`. With no model it SHALL mean `paris`.
- `--la-bias <bias>`: the light adjustment bias, a whole number from -100 to 100 inclusive, defaulting to 50. It SHALL
  be accepted only alongside `--light-adjustment`.
- `--color-balance`, alias `--cb`, with an optional model: `rio` or `saopaulo`. With no model it SHALL mean `rio`.
- `--cb-bias <bias>`: the colour balance bias, a whole number from -100 to 100 inclusive, defaulting to 50. It SHALL be
  accepted only alongside `--color-balance`.
- `--autopilot`, alias `--ap`: each image's enhancements are chosen by analysing it, as described in "Autopilot
  chooses each image's enhancements". It SHALL NOT be combined with any of the options above.
- `--quality`, alias `-q`: `hd` or `sd`, defaulting to `hd`.
- `--processor`, alias `-p`: the execution provider to run on, defaulting to `auto`. Its values are described in
  "The processor is chosen on the command line".
- `--recursive`, alias `-r`: a directory input contributes its whole tree rather than only the files directly in it.
- `--format <format>`: `preserve`, or one of `avif`, `bmp`, `gif`, `heic`, `jpeg`, `png`, `tiff` and `webp`, with
  `jpg`, `tif` and `heif` accepted as the same formats as `jpeg`, `tiff` and `heic`. It SHALL default to `preserve`.
  It SHALL be accepted only when the output is a directory.
- `--prefix <text>` and `--suffix <text>`: written before and after each source's name. Both SHALL default to empty,
  and SHALL be accepted only when the output is a directory.
- `--overwrite`: an existing file at a destination is replaced rather than kept.
- `--output`, alias `-o`: the file or directory to write to. It is required.

A strength or bias SHALL be applied as that percentage of the model's own effect: a strength of 100 or a bias of 100
applies the model's output as it is, a strength of 200 applies twice that, and a negative bias shifts the image in the
opposite direction. A negative bias SHALL be accepted written as a plain number, as in `--la-bias -20`.

The order in which the enhancement flags are given SHALL NOT change the order they are applied in. That order is the
library's apply order: denoise, face recovery, colorization, light adjustment, colour balance, sharpen, and then
upscale. Without `--autopilot`, the same enhancements and parameters SHALL apply to every image in the run.

Model names, format names and processor names SHALL be matched without regard to case.

#### Scenario: Bare flags take their default models

- **WHEN** the user runs `opai photo.jpg --up --fr -o out.png`
- **THEN** the photograph is face-restored with Athens and upscaled with Tokyo, and written to `out.png`

#### Scenario: Bare new flags take their default models and parameters

- **WHEN** the user runs `opai photo.jpg --dn --sh --cl --la --cb -o out.png`
- **THEN** the photograph is denoised with Stockholm at strength 100, colorized with Delhi, light-adjusted with Paris
  at bias 50, colour-balanced with Rio at bias 50, and sharpened with Moscow at strength 100

#### Scenario: A named model

- **WHEN** the user runs `opai photo.jpg --up osaka -o out.png`
- **THEN** the photograph is upscaled with Osaka

#### Scenario: A named model for a new family

- **WHEN** the user runs `opai photo.jpg --cb SaoPaulo -o out.png`
- **THEN** the photograph is colour-balanced with São Paulo

#### Scenario: Each parameter applies to its own enhancement

- **WHEN** the user runs `opai photo.jpg --dn --dn-str 200 --sh --sh-str 80 -o out.png`
- **THEN** the photograph is denoised at strength 200 and sharpened at strength 80

#### Scenario: A negative bias

- **WHEN** the user runs `opai photo.jpg --la --la-bias -20 -o out.png`
- **THEN** the photograph is light-adjusted at bias -20

#### Scenario: Flag order does not change the result

- **WHEN** the user runs the same photograph once with `--up --fr` and once with `--fr --up`
- **THEN** both runs apply face recovery first and then the upscale, and produce the same image

#### Scenario: Flag order does not change the apply order of the new families

- **WHEN** the user runs `opai photo.jpg --up --sh --dn -o out.png`
- **THEN** the photograph is denoised first, then sharpened, then upscaled

#### Scenario: Several inputs

- **WHEN** the user runs `opai a.jpg b.png scans --dn -o out`
- **THEN** `a.jpg`, `b.png` and the images directly inside `scans` are each denoised and written under `out`

#### Scenario: A format alias

- **WHEN** the user runs `opai photos --up --format JPG -o out`
- **THEN** every image is written as JPEG with the extension `jpg`

#### Scenario: An unknown format

- **WHEN** the user runs `opai photos --up --format raw -o out`
- **THEN** the command fails with a usage error listing the accepted formats, and nothing is installed or written

#### Scenario: An unknown model

- **WHEN** the user runs `opai photo.jpg --up paris -o out.png`
- **THEN** the command fails with a usage error that lists the accepted upscale models, and nothing is installed or
  written

#### Scenario: A scale without an upscale

- **WHEN** the user runs `opai photo.jpg --fr --scale 2 -o out.png`
- **THEN** the command fails with a usage error saying `--scale` requires `--upscale`

#### Scenario: A scale out of range

- **WHEN** the user runs `opai photo.jpg --up --scale 9 -o out.png`
- **THEN** the command fails with a usage error naming the accepted range, 1.0 to 8.0

#### Scenario: A parameter without its enhancement

- **WHEN** the user runs `opai photo.jpg --sh --dn-str 150 -o out.png`
- **THEN** the command fails with a usage error saying `--dn-str` requires `--denoise`

#### Scenario: A strength out of range

- **WHEN** the user runs `opai photo.jpg --dn --dn-str 0 -o out.png` or `opai photo.jpg --dn --dn-str 301 -o out.png`
- **THEN** the command fails with a usage error naming the accepted range, 1 to 300

#### Scenario: A bias out of range

- **WHEN** the user runs `opai photo.jpg --cb --cb-bias 101 -o out.png`
- **THEN** the command fails with a usage error naming the accepted range, -100 to 100

#### Scenario: A fractional parameter

- **WHEN** the user runs `opai photo.jpg --dn --dn-str 150.5 -o out.png`
- **THEN** the command fails with a usage error saying the strength must be a whole number from 1 to 300

### Requirement: At least one enhancement is requested

A run that gives none of `--upscale`, `--face-recovery`, `--denoise`, `--sharpen`, `--colorization`,
`--light-adjustment`, `--color-balance` or `--autopilot` SHALL fail with a usage error and SHALL write nothing. The
error SHALL list every enhancement flag with its alias, and `--autopilot` with its alias.

#### Scenario: No enhancement flag

- **WHEN** the user runs `opai photo.jpg -o out.png`
- **THEN** the command fails with a usage error saying at least one enhancement is required, and listing all seven
  enhancement flags and `--autopilot (--ap)`

#### Scenario: Autopilot alone is enough

- **WHEN** the user runs `opai photo.jpg --ap -o out.png`
- **THEN** the photograph is analysed, enhanced with what it calls for, and written

#### Scenario: A new family alone is enough

- **WHEN** the user runs `opai photo.jpg --cl -o out.png`
- **THEN** the photograph is colorized and written

### Requirement: Arguments and files are checked before anything is installed

Before initializing, and so before anything is downloaded, the command SHALL:

- refuse a named input, file or directory, that does not exist or cannot be read
- refuse a named input file that is not an image it can read
- refuse a directory input whose tree cannot be listed, naming the directory that could not be read
- skip a file found inside a directory input whose extension is one the application opens but whose image cannot be
  read, recording it in the log file, and go on with the rest
- refuse an output that is a file when more than one input is given, or when the one input is a directory
- refuse `--format`, `--prefix` or `--suffix` when the output is a file
- refuse an output that is a directory when a file that is not a directory already exists at that path
- refuse a run left with no image to enhance once the inputs are expanded
- with `--autopilot`, refuse an `OPAI_AP_*` environment variable whose value is neither empty, `off`, nor a model of
  its family, naming the variable and the value and listing the accepted models and `off`

A refusal SHALL name the offending path, flag or variable and SHALL exit with the usage status. Nothing SHALL be
written or created when the run is refused.

#### Scenario: Missing input

- **WHEN** the user runs `opai missing.jpg --up -o out.png` and `missing.jpg` does not exist
- **THEN** the command fails naming `missing.jpg`, and nothing is installed

#### Scenario: A missing input among several

- **WHEN** the user runs `opai a.jpg missing.jpg --up -o out` and `missing.jpg` does not exist
- **THEN** the command fails naming `missing.jpg`, and `a.jpg` is not enhanced

#### Scenario: A named file that is not an image

- **WHEN** the user runs `opai notes.txt --up -o out`
- **THEN** the command fails naming `notes.txt`, and nothing is installed

#### Scenario: A broken image inside a directory

- **WHEN** a directory input holds `a.jpg`, which is readable, and `b.jpg`, which is not a readable image
- **THEN** `b.jpg` is skipped and recorded in the log file, and `a.jpg` is enhanced

#### Scenario: A file output with several inputs

- **WHEN** the user runs `opai a.jpg b.jpg --up -o out.png`
- **THEN** the command fails saying an output file takes exactly one input file, and nothing is installed

#### Scenario: Affixes with a file output

- **WHEN** the user runs `opai photo.jpg --up --suffix -big -o out.png`
- **THEN** the command fails saying `--suffix` needs an output directory, and nothing is installed

#### Scenario: A directory with no images

- **WHEN** the user runs `opai empty --up -o out`, and `empty` holds no file the application opens
- **THEN** the command fails saying there is no image to enhance, and `out` is not created

#### Scenario: Format follows the extension

- **WHEN** the user runs `opai photo.jpg --up -o out.webp`
- **THEN** the file written is WebP

#### Scenario: Unknown output extension

- **WHEN** `out.xyz` does not exist and the user runs `opai photo.jpg --up -o out.xyz`
- **THEN** `.xyz` names no format the command writes, so `out.xyz` is read as a directory: it is created, and the
  result is written to `out.xyz/photo.jpg`

#### Scenario: An autopilot variable naming another family's model

- **WHEN** the user runs `OPAI_AP_UPSCALE=paris opai photo.jpg --ap -o out.png`
- **THEN** the command fails with a usage error naming `OPAI_AP_UPSCALE` and `paris`, and listing `kyoto`, `osaka`,
  `tokyo`, `saitama` and `off`
- **AND** nothing is installed or written

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
        as unnecessary or because autopilot suggested none
      - `🖼️ [<p>%] Choosing enhancements for <path>` with `--autopilot`, from when the image is loaded until its
        analysis has ended, at the percentage `0`
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

#### Scenario: An image being analysed

- **WHEN** the user runs `opai photo.jpg --ap -o out.png` on a terminal
- **THEN** once the image is loaded, its row reads `🖼️ [  0%] Choosing enhancements for photo.jpg`, its path in the
  same column as every other wording's
- **AND** once the analysis has ended, the row moves on to `Downloading <family> for` or `Applying <family> to` for each
  suggested enhancement, or stays at `Preparing to enhance` when none was suggested, until the file is written

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

