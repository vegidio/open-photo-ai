# cli-enhance Specification

## Purpose

The terminal front end's enhance command: one photograph in, upscaled and/or face-restored, one file out. It covers the
arguments and their defaults, what is checked before anything is installed, what the run reports while it works, and
how it ends.

## Requirements

### Requirement: The command takes one input, its enhancements, and an output

The terminal application SHALL be invoked as `opai <input> [options] --output <file>`. The input path SHALL be the
first argument. The options SHALL be:

- `--upscale`, alias `--up`, with an optional model: `kyoto`, `osaka`, `tokyo` or `saitama`. With no model it SHALL
  mean `tokyo`.
- `--scale <factor>`: the upscale factor, from 1.0 to 8.0 inclusive. It SHALL be accepted only alongside `--upscale`.
- `--face-recovery`, alias `--fr`, with an optional model: `athens` or `santorini`. With no model it SHALL mean
  `athens`.
- `--quality`, alias `-q`: `hd` or `sd`, defaulting to `hd`.
- `--output`, alias `-o`: the file to write. It is required.

The order in which `--upscale` and `--face-recovery` are given SHALL NOT change the order they are applied in. That
order is the library's apply order, in which a face recovery runs before an upscale.

Model names SHALL be matched without regard to case.

#### Scenario: Bare flags take their default models

- **WHEN** the user runs `opai photo.jpg --up --fr -o out.png`
- **THEN** the photograph is face-restored with Athens and upscaled with Tokyo, and written to `out.png`

#### Scenario: A named model

- **WHEN** the user runs `opai photo.jpg --up osaka -o out.png`
- **THEN** the photograph is upscaled with Osaka

#### Scenario: Flag order does not change the result

- **WHEN** the user runs the same photograph once with `--up --fr` and once with `--fr --up`
- **THEN** both runs apply face recovery first and then the upscale, and produce the same image

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

### Requirement: At least one enhancement is requested

A run that gives neither `--upscale` nor `--face-recovery` SHALL fail with a usage error and SHALL write nothing.

#### Scenario: No enhancement flag

- **WHEN** the user runs `opai photo.jpg -o out.png`
- **THEN** the command fails with a usage error saying at least one enhancement is required

### Requirement: Quality selects the precision each model runs at

`hd` SHALL run every requested model at its higher-fidelity published build, and `sd` at its lighter one:

| Model | `hd` | `sd` |
|---|---|---|
| Kyoto, Tokyo, Saitama, Athens, Santorini | FP32 | FP16 |
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

### Requirement: The upscale factor defaults to what the photograph needs

With `--scale` given, the upscale SHALL run at that factor.

Without `--scale`, the factor SHALL be the library's suggested scale for the input's dimensions: 4x up to 1 MP, 2x up
to 4 MP. Above 4 MP the upscale SHALL be skipped, and the command SHALL print a note saying it was skipped because the
photograph is already large enough. The rest of the run SHALL continue. The output SHALL be written even when the
skipped upscale was the only enhancement requested.

#### Scenario: A small photograph

- **WHEN** a 1000x800 photograph is run with `--up` and no `--scale`
- **THEN** it is upscaled 4x

#### Scenario: A large photograph with only an upscale requested

- **WHEN** a 4000x3000 photograph is run with `--up` and no `--scale`
- **THEN** a note says the upscale was skipped because the photograph is already large enough
- **AND** the output is written with the photograph's own dimensions
- **AND** the completion line says no enhancement ran

#### Scenario: An explicit scale is always honoured

- **WHEN** a 4000x3000 photograph is run with `--up --scale 2`
- **THEN** it is upscaled 2x

### Requirement: Arguments and files are checked before anything is installed

Before initializing, and so before anything is downloaded, the command SHALL refuse:

- an input that does not exist or is not an image it can read
- an output whose extension names no format it can write, or has no extension

The output format SHALL be the one the output's extension names. A refusal SHALL name the offending path and SHALL
exit with the usage status.

#### Scenario: Missing input

- **WHEN** the user runs `opai missing.jpg --up -o out.png` and `missing.jpg` does not exist
- **THEN** the command fails naming `missing.jpg`, and nothing is installed

#### Scenario: Unknown output extension

- **WHEN** the user runs `opai photo.jpg --up -o out.xyz`
- **THEN** the command fails saying `.xyz` is not a format it can write, and lists the formats it can

#### Scenario: Format follows the extension

- **WHEN** the user runs `opai photo.jpg --up -o out.webp`
- **THEN** the file written is WebP

### Requirement: The output is written like a GUI export

The output SHALL be written at the source's own bit depth (16 bits for a source carrying more than 8, otherwise 8).
For a lossy format it SHALL use the library's recommended encoder settings for that format, the same ones a GUI export
uses when the user has not changed the quality.

An existing file at the output path SHALL be overwritten without a prompt.

#### Scenario: A 16-bit source written as PNG

- **WHEN** a 16-bit TIFF is run with `--up -o out.png`
- **THEN** `out.png` is 16 bits per channel

#### Scenario: JPEG at the recommended quality

- **WHEN** a photograph is run with `-o out.jpg`
- **THEN** it is written at the library's recommended JPEG quality

#### Scenario: Overwriting

- **WHEN** `out.png` already exists and the user runs `opai photo.jpg --up -o out.png`
- **THEN** `out.png` is replaced by the new result, without a prompt

### Requirement: Faces are found for a face recovery, and a failed detection does not stop the run

When a face recovery is requested, the command SHALL find the faces with the library's shared step and its default
selection. That selection is the same set the GUI restores before a user toggles any face. The command SHALL report how
many faces were found and how many will be restored.

If no face is found, the command SHALL say so, and the rest of the chain SHALL still run.

If the detection fails, the command SHALL print a warning carrying the failure, and SHALL run the rest of the chain with
no face restored.

#### Scenario: Faces found

- **WHEN** a photograph with three faces, two of them restorable, is run with `--fr`
- **THEN** the command reports three faces found and two to restore, and restores those two

#### Scenario: No faces

- **WHEN** a landscape with no faces is run with `--fr --up`
- **THEN** the command says no faces were found, and the photograph is still upscaled and written

#### Scenario: Detection fails

- **WHEN** the face detection fails during a `--fr --up` run
- **THEN** a warning carrying the failure is printed, and the photograph is still upscaled and written

### Requirement: Progress is shown while the command works

While initializing, the command SHALL show each dependency it installs with its own progress. A dependency already
installed SHALL be shown as already installed, not re-downloaded.

While enhancing, the command SHALL show the operation in progress (face detection, each enhancement), its stage
(downloading its model, running, or served from the cache), and progress through the whole run.

When standard error is not a terminal, progress bars SHALL NOT be drawn. The command's messages, notes and errors SHALL
still be written.

The command SHALL run on the best execution provider this machine supports. On completion it SHALL report the path
written and the provider or providers the run used. If every step was served from the run cache, it SHALL say that
instead. If no enhancement was left to run, because the only one requested was an upscale skipped as unnecessary, it
SHALL say that no enhancement ran rather than that the run was served from the cache.

#### Scenario: First run on a new machine

- **WHEN** the command runs where the runtime is not yet installed
- **THEN** the runtime's download and expansion are shown with progress before enhancing starts

#### Scenario: Model downloaded during the run

- **WHEN** a requested model is not yet on disk
- **THEN** its download is shown as that operation's stage before it runs

#### Scenario: Output piped to a file

- **WHEN** standard error is redirected to a file
- **THEN** the file holds the command's messages, without progress-bar redraw sequences

### Requirement: The command refuses to start while another instance holds the claim

If the GUI or another terminal run is already initialized for this user, the command SHALL fail before installing
anything. The message SHALL be the library's refusal, which names the holder and its process id where those can be read
and tells the user to close it and try again. Nothing SHALL be written.

#### Scenario: GUI open

- **WHEN** the GUI is open and the user runs `opai photo.jpg --up -o out.png`
- **THEN** the command fails saying Open Photo AI is already running, naming the GUI, and `out.png` is not written

### Requirement: A run can be stopped

Pressing Ctrl-C while the photograph is being enhanced SHALL stop the run at the next point the library checks. The
command SHALL write no output file and SHALL exit with the interrupted status. A model download it was waiting on SHALL
keep the bytes already transferred, so the next run resumes from them.

#### Scenario: Interrupted upscale

- **WHEN** the user presses Ctrl-C during an upscale
- **THEN** the command says it was stopped, writes nothing, and exits with the interrupted status

### Requirement: Exit status says how the run ended

The command SHALL exit with:

- `0` when the output was written
- `2` for a usage error: bad arguments, an unreadable input, or an unwritable output format
- `130` when stopped by Ctrl-C
- `1` for any other failure: initialization, inference, or writing the file

Every failure SHALL print a one-line reason to standard error.

#### Scenario: Success

- **WHEN** a run writes its output
- **THEN** the exit status is 0

#### Scenario: Write fails

- **WHEN** the output's directory does not exist
- **THEN** the command prints a reason naming the output path and exits with status 1

### Requirement: The command reports its version

`opai --version` SHALL print the version the core library reports, the same value the GUI's navbar shows, and SHALL
initialize nothing.

#### Scenario: Version

- **WHEN** the user runs `opai --version`
- **THEN** the running build's version is printed and the command exits 0 without installing anything
