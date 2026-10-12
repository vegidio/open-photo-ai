## MODIFIED Requirements

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
   `💾 Downloading dependency <name>   <percent>%`, with three spaces between the name and the percentage. The name
   SHALL be the dependency's name as the GUI shows it (for example `ONNX Runtime`, `NVIDIA CUDA`, `NVIDIA TensorRT`).
   The percentage SHALL be a whole number covering that dependency's whole install, download and expansion together,
   and SHALL NOT show `100` before that install has finished. The line SHALL be rewritten in place as the install
   progresses, and when the next dependency starts, its name and percentage SHALL replace the previous ones. When
   initialization ends, the line SHALL remain as it was last drawn, at the last dependency's `100%`. A dependency that
   was already installed SHALL NOT be shown. Where every dependency was already installed, there SHALL be no dependency
   line.
2. **A header line**, `🖼️ Applying enhancements to the image <input>`, naming the input path as it was given.
3. A blank line.
4. **One progress bar for the whole run.** It SHALL cover the face detection, when there is one, and every operation
   of the chain, and it SHALL move forward once from the start to the end. It SHALL NOT refill between the detection
   and the chain or between operations. The bar SHALL be labelled with what the run is doing, in the GUI's words:
   - `Enhancing` before the run has reported anything
   - `Downloading` while an operation's model is being fetched, without a percentage of its own
   - otherwise the enhancement's name (`Face Recovery`, `Upscale`); the face detection reads as `Face Recovery`

   The label SHALL be padded so that the bar does not move sideways when the label changes. After the label, the line
   SHALL show a step counter `[n/N]`. `N` is the number of enhancements the run will apply. `n` is `0` before the run
   has reported anything, and afterwards the step being worked on, counted from one in the order the enhancements are
   applied. The face detection counts as the face recovery's step. The line SHALL end with the progress of the whole
   run as a percentage with one decimal place, which SHALL NOT show `100.0%` before the run has finished. The line
   SHALL NOT show an estimated time remaining. Where no enhancement is left to run, because the only one requested was
   an upscale skipped as unnecessary, no progress bar SHALL be drawn.
5. A blank line.
6. **A result line.** On success it SHALL read `✨ Image enhanced successfully; file saved at <output>`. On failure it
   SHALL be the failure line (see "Exit status says how the run ended").

On a colour terminal, the dependency's name SHALL be yellow and its percentage bold white. The header's input path
SHALL be green. The bar SHALL be drawn in a colour gradient. The success line SHALL be bold yellow throughout. When the
`NO_COLOR` environment variable is set to a non-empty value, the same lines SHALL be drawn without colour.

When standard error is not a terminal, the command SHALL print only the header line and the result line, without
colour, without a dependency line or progress bar, and without any cursor-movement or redraw sequences.

The command SHALL run on the best execution provider this machine supports. It SHALL record in its log file the size
and format of the image written and the provider or providers the run used. If every step was served from the run
cache, it SHALL record that instead. If no enhancement was left to run, it SHALL record that no enhancement ran,
rather than that the run was served from the cache. None of this SHALL be printed to the terminal.

#### Scenario: First run on a new machine

- **WHEN** the command runs where the runtime is not yet installed, on a terminal
- **THEN** the first line shows `💾 Downloading dependency ONNX Runtime` with a rising percentage
- **AND** when initialization ends, that line remains at `100%` and the header follows it

#### Scenario: Several dependencies downloaded

- **WHEN** initialization downloads the runtime and then a GPU library, on a terminal
- **THEN** the single dependency line names the runtime with its percentage, and is then rewritten to name the GPU
  library with its own percentage

#### Scenario: Everything already installed

- **WHEN** the command runs where every dependency is already installed, on a terminal
- **THEN** the first line is the header, and no dependency line is shown

#### Scenario: Face recovery and upscale

- **WHEN** the user runs `opai photo.jpg --fr --up -o out.png` on a terminal, and the faces are not already known
- **THEN** the bar is labelled `Face Recovery` with the counter `[1/2]` while the faces are found and restored, then
  `Upscale` with `[2/2]`
- **AND** the bar's progress never decreases and ends at `100.0%`
- **AND** the run ends with `✨ Image enhanced successfully; file saved at out.png`

#### Scenario: Model downloaded during the run

- **WHEN** a requested model is not yet on disk
- **THEN** the bar is labelled `Downloading` while the model is fetched, keeping that operation's step counter, and
  then takes the operation's own name when it runs

#### Scenario: Nothing printed outside the layout

- **WHEN** a `--fr --up` run finds faces and writes its output on a terminal
- **THEN** the terminal shows no face count, no provider, no image size and no format
- **AND** the log file records them

#### Scenario: Output piped to a file

- **WHEN** standard error is redirected to a file
- **THEN** the file holds the header line and the result line, with no colour, no dependency line, no progress bar and
  no redraw sequences

#### Scenario: Colours turned off

- **WHEN** the command runs on a terminal with `NO_COLOR=1`
- **THEN** the dependency line, the bar and the result line are drawn as usual, without colour

### Requirement: A run can be stopped

Pressing Ctrl-C while the photograph is being enhanced SHALL stop the run at the next point the library checks. The
command SHALL write no output file and SHALL exit with the interrupted status. A model download it was waiting on SHALL
keep the bytes already transferred, so the next run resumes from them.

Pressing Ctrl-C while the dependency line is shown SHALL stop the command without finishing initialization. It SHALL
write nothing and exit with the interrupted status. A dependency download it interrupts SHALL keep the bytes already
transferred, so the next run resumes from them.

However the command is stopped, the terminal SHALL be left as it was found: echo and line editing work, and the
shell's prompt starts on a line of its own.

#### Scenario: Interrupted upscale

- **WHEN** the user presses Ctrl-C during an upscale
- **THEN** the progress bar stays in place, the command ends with its failure line saying it was stopped, writes
  nothing, and exits with the interrupted status

#### Scenario: Interrupted dependency download

- **WHEN** the user presses Ctrl-C while the dependency line is shown
- **THEN** the command ends with its failure line saying it was stopped, writes nothing, exits with the interrupted
  status, and leaves the terminal usable
- **AND** the next run resumes the download from the bytes already transferred

### Requirement: Exit status says how the run ended

The command SHALL exit with:

- `0` when the output was written
- `2` for a usage error: bad arguments, an unreadable input, or an unwritable output format
- `130` when stopped by Ctrl-C
- `1` for any other failure: initialization, inference, or writing the file

Every failure SHALL end with one failure line on standard error, `🧨 <reason>`, where the reason is a short message
saying what went wrong. On a colour terminal the failure line SHALL be bold red throughout. This applies to every
failure the command reports itself, including refusals of the input or output, another instance holding the claim, and
a stop by Ctrl-C. Argument errors detected while parsing the command line, and `--help` and `--version`, SHALL be
printed as the argument parser prints them, with no failure line.

#### Scenario: Success

- **WHEN** a run writes its output
- **THEN** the exit status is 0

#### Scenario: Write fails

- **WHEN** the output's directory does not exist
- **THEN** the command ends with a failure line `🧨 <reason>` naming the output path, and exits with status 1

#### Scenario: Unreadable input

- **WHEN** the user runs `opai missing.jpg --up -o out.png` and `missing.jpg` does not exist
- **THEN** the command prints a single failure line `🧨 <reason>` naming `missing.jpg`, and exits with status 2

#### Scenario: Unknown flag

- **WHEN** the user runs `opai photo.jpg --up --sharpen -o out.png`
- **THEN** the argument parser's own error is printed, with no failure line, and the exit status is 2
