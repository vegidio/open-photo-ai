# cli-enhance Specification

## Purpose

The terminal front end's enhance command: one photograph in, enhanced by any of the seven enhancement families, one
file out. It covers the arguments and their defaults, what is checked before anything is installed, what the run
reports while it works, and how it ends.

## Requirements

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
upscale. The same enhancements and parameters SHALL apply to every image in the run.

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
`--light-adjustment` or `--color-balance` SHALL fail with a usage error and SHALL write nothing. The error SHALL list
every enhancement flag with its alias.

#### Scenario: No enhancement flag

- **WHEN** the user runs `opai photo.jpg -o out.png`
- **THEN** the command fails with a usage error saying at least one enhancement is required, and listing all seven
  enhancement flags

#### Scenario: A new family alone is enough

- **WHEN** the user runs `opai photo.jpg --cl -o out.png`
- **THEN** the photograph is colorized and written

### Requirement: Quality selects the precision each model runs at

`hd` SHALL run every requested model at its higher-fidelity published build, and `sd` at its lighter one:

| Model | `hd` | `sd` |
|---|---|---|
| Kyoto, Tokyo, Saitama, Athens, Santorini | FP32 | FP16 |
| Stockholm, Gothenburg, Malmö | FP32 | FP16 |
| Moscow, Petersburg, Novgorod | FP32 | FP16 |
| Delhi, Mumbai, Jaipur | FP32 | FP16 |
| Paris, Lyon | FP32 | FP16 |
| Rio, São Paulo | FP32 | FP16 |
| Osaka | FP16 | INT8 |

The face detection that feeds a face recovery SHALL run at the precision the library prescribes for it, whatever the
quality.

A face recovery SHALL run at maximum fidelity to the original face.

#### Scenario: Default quality

- **WHEN** the user runs `opai photo.jpg --up -o out.png`
- **THEN** Tokyo runs at FP32

#### Scenario: Standard quality on Osaka

- **WHEN** the user runs `opai photo.jpg --up osaka -q sd -o out.png`
- **THEN** Osaka runs its INT8 build

#### Scenario: Standard quality on a new family

- **WHEN** the user runs `opai photo.jpg --dn gothenburg -q sd -o out.png`
- **THEN** Gothenburg runs its FP16 build

### Requirement: The processor is chosen on the command line

`--processor`, alias `-p`, SHALL choose the execution provider every model of the run is asked to run on, face
detection included. Its values SHALL be:

| Value | Provider | Accepted in builds for |
|---|---|---|
| `auto` (default) | the best provider this machine supports, trying TensorRT, CUDA, CoreML, WebGPU and then the CPU | every OS |
| `cpu` | the CPU | every OS |
| `coreml` | CoreML | macOS only |
| `cuda` | CUDA | Windows and Linux only |
| `tensorrt` | TensorRT | Windows and Linux only |
| `webgpu` | WebGPU | every OS |

A value that is not accepted by the build for the running OS SHALL be refused by the argument parser as an unknown
value: clap's own error, with no failure line, and the usage status. `--help` and the parser's list of possible
values SHALL name only the values the running build accepts.

An accepted value SHALL then be checked against the providers this machine offers once initialization has
finished. This is the same report the GUI builds its processor list from. `auto` and `cpu` SHALL always pass. A
value this machine does not offer SHALL be refused before any image is loaded. The refusal SHALL be one failure line
naming the processor asked for and the processors this machine offers, and the command SHALL exit with the usage
status. Nothing SHALL be written.

A processor that passes the check but fails to run a model SHALL fall back to the CPU for that model, as the library
does, and the run SHALL go on. The log file SHALL record the fallback. The terminal SHALL NOT report it.

The processor SHALL NOT change what initialization installs. The installed dependencies SHALL be what this machine
needs, whichever processor is chosen.

#### Scenario: No processor given

- **WHEN** the user runs `opai photo.jpg --up -o out.png`
- **THEN** the run uses the best provider this machine supports, as it did before this flag existed

#### Scenario: Forcing the CPU

- **WHEN** the user runs `opai photo.jpg --up -p cpu -o out.png` on a machine with an accelerator
- **THEN** every model of the run, face detection included, is asked to run on the CPU, and the log file records that
  the image ran on CPU

#### Scenario: Case does not matter

- **WHEN** the user runs `opai photo.jpg --up --processor CoreML -o out.png` on macOS
- **THEN** the run uses CoreML

#### Scenario: A processor this OS can never offer

- **WHEN** the user runs `opai photo.jpg --up -p cuda -o out.png` with a macOS build
- **THEN** the argument parser's own error is printed, listing the possible values `auto`, `cpu`, `coreml` and
  `webgpu`, with no failure line, and the exit status is 2
- **AND** nothing is installed

#### Scenario: A processor this machine does not offer

- **WHEN** the user runs `opai photo.jpg --up -p tensorrt -o out.png` on a Linux machine with no NVIDIA GPU, whose
  machine offers the CPU only
- **THEN** initialization finishes, and the command ends with one failure line naming `tensorrt` and the processors
  this machine offers, `auto` and `cpu`
- **AND** no image is loaded, nothing is written, and the exit status is 2

#### Scenario: Help lists only this build's processors

- **WHEN** the user runs `opai --help` with a Windows build
- **THEN** `--processor` lists `auto`, `cpu`, `cuda`, `tensorrt` and `webgpu`, and does not list `coreml`

### Requirement: The upscale factor defaults to what the photograph needs

With `--scale` given, the upscale SHALL run at that factor.

Without `--scale`, the factor SHALL be the library's suggested scale for the input's dimensions: 4x up to 1 MP, 2x up
to 4 MP. Above 4 MP the upscale SHALL be skipped, and the command SHALL record in its log file that the upscale was
skipped because the photograph is already large enough. That note SHALL NOT be printed to the terminal. The rest of
the run SHALL continue. The output SHALL be written even when the skipped upscale was the only enhancement requested.

#### Scenario: A small photograph

- **WHEN** a 1000x800 photograph is run with `--up` and no `--scale`
- **THEN** it is upscaled 4x

#### Scenario: A large photograph with only an upscale requested

- **WHEN** a 4000x3000 photograph is run with `--up` and no `--scale`
- **THEN** the log file records that the upscale was skipped because the photograph is already large enough, and the
  terminal shows no note about it
- **AND** the output is written with the photograph's own dimensions
- **AND** the command ends with its success line

#### Scenario: An explicit scale is always honoured

- **WHEN** a 4000x3000 photograph is run with `--up --scale 2`
- **THEN** it is upscaled 2x

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

A refusal SHALL name the offending path or flag and SHALL exit with the usage status. Nothing SHALL be written or
created when the run is refused.

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

### Requirement: The output is written like a GUI export

Each result SHALL be written at its source's own bit depth (16 bits for a source carrying more than 8, otherwise 8).
For a lossy format it SHALL use the library's recommended encoder settings for that format, the same ones a GUI export
uses when the user has not changed the quality.

Under an output directory, each result's format, extension and name SHALL follow the library's export naming rule
that the GUI's export uses, with the run's format choice, prefix and suffix.

Without `--overwrite`, an existing file at a destination SHALL be kept. The result SHALL be written to the first free
name made by adding `_1`, `_2` and so on, up to `_999`, before the extension, as the GUI's export does. With
`--overwrite`, the destination SHALL be replaced, and only once the new file is complete. A write that does not finish
SHALL leave the destination as it was and nothing beside it.

#### Scenario: A 16-bit source written as PNG

- **WHEN** a 16-bit TIFF is run with `--up -o out.png`
- **THEN** `out.png` is 16 bits per channel

#### Scenario: JPEG at the recommended quality

- **WHEN** a photograph is run with `-o out.jpg`
- **THEN** it is written at the library's recommended JPEG quality

#### Scenario: Preserve under a directory

- **WHEN** `a.jpg` and `b.nef` are run with `--up -o out`
- **THEN** `out/a.jpg` is written as JPEG and `out/b.tiff` as TIFF

#### Scenario: Prefix, suffix and format

- **WHEN** `beach.jpg` is run with `--up --prefix new- --suffix -opai --format webp -o out`
- **THEN** the result is written to `out/new-beach-opai.webp`

#### Scenario: An existing file is kept

- **WHEN** `out.png` already exists and the user runs `opai photo.jpg --up -o out.png`
- **THEN** `out.png` is unchanged, and the result is written to `out_1.png`

#### Scenario: Overwriting

- **WHEN** `out.png` already exists and the user runs `opai photo.jpg --up --overwrite -o out.png`
- **THEN** `out.png` is replaced by the new result, without a prompt

#### Scenario: In place

- **WHEN** `photos` holds `a.jpg` and `b.png`, and the user runs `opai photos --dn --overwrite -o photos`
- **THEN** `a.jpg` and `b.png` are replaced by their denoised results, in their own formats, and no other file is
  left in `photos`

#### Scenario: In place where the name changes

- **WHEN** `photos` holds `a.JPG` and `b.nef`, and the user runs `opai photos --dn --overwrite -o photos`
- **THEN** the results are written to `a.jpg` and `b.tiff`, the names the export rule gives them, and `b.nef` is kept
- **AND** `a.JPG` is replaced only where the filesystem does not distinguish case; elsewhere it is kept beside
  `a.jpg`

### Requirement: A directory input contributes the images inside it

A directory input SHALL contribute every file directly inside it whose extension, in any case, is one the
application opens, camera RAW included. With `--recursive` it SHALL contribute such files from its whole tree. Other
files SHALL be ignored without being reported. A link to a directory found inside a directory input SHALL NOT be
descended into.

The images SHALL be enhanced in the order their inputs were given. The images a directory contributes SHALL follow
each other in name order within each directory.

#### Scenario: Only the top level by default

- **WHEN** `photos` holds `a.jpg` and `2024/b.jpg`, and the user runs `opai photos --up -o out`
- **THEN** only `a.jpg` is enhanced

#### Scenario: The whole tree with --recursive

- **WHEN** `photos` holds `a.jpg` and `2024/b.jpg`, and the user runs `opai photos -r --up -o out`
- **THEN** both are enhanced

#### Scenario: Other files are ignored

- **WHEN** `photos` holds `a.JPG`, `shot.nef` and `notes.txt`
- **THEN** `a.JPG` and `shot.nef` are enhanced, and `notes.txt` is neither enhanced nor reported

### Requirement: Wildcards are expanded on every platform

An input containing a wildcard (`*`, `?`, `[…]`) that the shell did not expand SHALL be expanded by the command on
Windows, so `opai *.jpg --up -o out` selects the same files there as in a Unix shell. A wildcard matching nothing
SHALL be kept as written, and refused as a missing input.

#### Scenario: A wildcard in a Windows shell

- **WHEN** the user runs `opai *.jpg --up -o out` in PowerShell, in a directory holding `a.jpg` and `b.jpg`
- **THEN** both are enhanced

### Requirement: Each image is enhanced once

An image reached more than once, because it was named twice, or both named and found in a directory input, or found
in two overlapping directory inputs, SHALL be enhanced once, where it was first reached. Two paths are the same image
when they lead to the same file.

#### Scenario: Named and inside a directory

- **WHEN** the user runs `opai photos/a.jpg photos --up -o out`
- **THEN** `a.jpg` is enhanced once and written to `out/a.jpg`

### Requirement: An output directory's contents are not taken as input

When the output is a directory inside a directory input, nothing inside the output directory SHALL be taken as an
input.

#### Scenario: Results kept inside the input

- **WHEN** `photos/enhanced` holds results of an earlier run, and the user runs `opai photos -r --up -o photos/enhanced`
- **THEN** only the images outside `photos/enhanced` are enhanced

### Requirement: The output is a file or a directory

An output path that already exists as a directory SHALL be a directory. Otherwise, an output path whose extension, in
any case, names a format the command can write SHALL be a file, written in that format. Any other output path SHALL be
a directory, created with any missing parents when the first result is written into it.

#### Scenario: A file

- **WHEN** the user runs `opai photo.jpg --up -o out.PNG`
- **THEN** the result is written as PNG to `out.PNG`

#### Scenario: A directory to create

- **WHEN** `out` does not exist and the user runs `opai photo.jpg --up -o out`
- **THEN** `out` is created and the result is written to `out/photo.jpg`

#### Scenario: An existing directory whatever its name

- **WHEN** a directory named `shots.png` exists and the user runs `opai photo.jpg --up -o shots.png`
- **THEN** the result is written to `shots.png/photo.jpg`

### Requirement: Results under an output directory mirror each directory input

Under an output directory, the result of an image found in a directory input SHALL be written at that image's path
relative to the directory input, inside the output directory, with any missing directories created. The result of a
file named as an input SHALL be written directly inside the output directory.

#### Scenario: A tree mirrored

- **WHEN** the user runs `opai photos -r --up -o out`, and `photos` holds `a.jpg` and `2024/summer/b.jpg`
- **THEN** the results are `out/a.jpg` and `out/2024/summer/b.jpg`

#### Scenario: Named files go flat

- **WHEN** the user runs `opai trips/2024/a.jpg home/b.jpg --up -o out`
- **THEN** the results are `out/a.jpg` and `out/b.jpg`

#### Scenario: Two inputs with the same name

- **WHEN** the user runs `opai one/a.jpg two/a.jpg --up -o out`, and `out` is empty
- **THEN** the results are `out/a.jpg` and `out/a_1.jpg`

### Requirement: A batch continues past a failed image

An image that fails to decode, to be enhanced or to be written SHALL end with its own failure line, naming the image,
and SHALL NOT stop the run. The next image SHALL be enhanced. The run SHALL be initialized once, and every image SHALL
share that initialization, so a dependency is downloaded at most once per run.

#### Scenario: A failure in the middle

- **WHEN** the second of three images fails while being enhanced
- **THEN** the first and third are written, and the second ends with a failure line naming it

### Requirement: Faces are found for a face recovery, and a failed detection does not stop the run

When a face recovery is requested, the command SHALL find the faces with the library's shared step and its default
selection. That selection is the same set the GUI restores before a user toggles any face. The command SHALL record in
its log file how many faces were found and how many will be restored. It SHALL NOT print that report to the terminal.

If no face is found, the command SHALL record that in its log file, and the rest of the chain SHALL still run.

If the detection fails, the command SHALL record a warning carrying the failure in its log file, and SHALL run the rest
of the chain with no face restored. The run SHALL still end with its success line when the output is written.

#### Scenario: Faces found

- **WHEN** a photograph with three faces, two of them restorable, is run with `--fr`
- **THEN** the log file records three faces found and two to restore, and those two are restored
- **AND** the terminal shows no face count

#### Scenario: No faces

- **WHEN** a landscape with no faces is run with `--fr --up`
- **THEN** the log file records that no faces were found, and the photograph is still upscaled and written

#### Scenario: Detection fails

- **WHEN** the face detection fails during a `--fr --up` run
- **THEN** the log file records a warning carrying the failure, and the photograph is still upscaled and written

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

### Requirement: The command refuses to start while another instance holds the claim

If the GUI or another terminal run is already initialized for this user, the command SHALL fail before installing
anything. The message SHALL be the library's refusal, which names the holder and its process id where those can be read
and tells the user to close it and try again. Nothing SHALL be written.

#### Scenario: GUI open

- **WHEN** the GUI is open and the user runs `opai photo.jpg --up -o out.png`
- **THEN** the command fails saying Open Photo AI is already running, naming the GUI, and `out.png` is not written

### Requirement: A run can be stopped

Pressing Ctrl-C while an image is being enhanced SHALL stop the run at the next point the library checks. The image
being enhanced SHALL NOT be written, and no image after it SHALL be started. Files the run already wrote SHALL be
kept. The command SHALL exit with the interrupted status. A model download it was waiting on SHALL keep the bytes
already transferred, so the next run resumes from them.

Pressing Ctrl-C while the dependency line is shown SHALL stop the command without finishing initialization. It SHALL
write nothing and exit with the interrupted status. A dependency download it interrupts SHALL keep the bytes already
transferred, so the next run resumes from them.

However the command is stopped, the terminal SHALL be left as it was found: echo and line editing work, and the
shell's prompt starts on a line of its own. With `--overwrite`, a file the stopped image would have replaced SHALL be
left as it was.

#### Scenario: Interrupted upscale

- **WHEN** the user presses Ctrl-C during an upscale
- **THEN** the progress bar stays in place, the command ends with its failure line saying it was stopped, writes
  nothing for that image, and exits with the interrupted status

#### Scenario: Interrupted batch

- **WHEN** the user presses Ctrl-C while the third of five images is being enhanced
- **THEN** the first two results are kept, nothing is written for the third, the fourth and fifth are not started,
  and the command exits with the interrupted status

#### Scenario: Interrupted overwrite

- **WHEN** the user presses Ctrl-C while an image run with `--overwrite` in place is being enhanced
- **THEN** the source file is unchanged, and no hidden file is left beside it

#### Scenario: Interrupted dependency download

- **WHEN** the user presses Ctrl-C while the dependency line is shown
- **THEN** the command ends with its failure line saying it was stopped, writes nothing, exits with the interrupted
  status, and leaves the terminal usable
- **AND** the next run resumes the download from the bytes already transferred

### Requirement: Exit status says how the run ended

The command SHALL exit with:

- `0` when every image was written
- `2` for a usage error: bad arguments, an unreadable named input, an output that cannot be used as asked, no image
  to enhance, or a processor this machine does not offer
- `130` when stopped by Ctrl-C
- `1` for any other failure: initialization, or any image that failed to decode, enhance or be written

Every failure SHALL end with one failure line on standard error, `🧨 <reason>`, where the reason is a short message
saying what went wrong. On a colour terminal the failure line SHALL be bold red throughout. This applies to every
failure the command reports itself, including refusals of the inputs, the output or the processor, another instance holding the claim,
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

### Requirement: The command reports its version

`opai --version` SHALL print the version the core library reports, the same value the GUI's navbar shows, and SHALL
initialize nothing.

#### Scenario: Version

- **WHEN** the user runs `opai --version`
- **THEN** the running build's version is printed and the command exits 0 without installing anything
