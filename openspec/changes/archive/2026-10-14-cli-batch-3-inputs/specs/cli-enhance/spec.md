## RENAMED Requirements

- FROM: `### Requirement: The command takes one input, its enhancements, and an output`
- TO: `### Requirement: The command takes its inputs, its enhancements, and an output`

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
- `--quality`, alias `-q`: `hd` or `sd`, defaulting to `hd`.
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

Model names and format names SHALL be matched without regard to case.

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
- `2` for a usage error: bad arguments, an unreadable named input, an output that cannot be used as asked, or no
  image to enhance
- `130` when stopped by Ctrl-C
- `1` for any other failure: initialization, or any image that failed to decode, enhance or be written

Every failure SHALL end with one failure line on standard error, `🧨 <reason>`, where the reason is a short message
saying what went wrong. On a colour terminal the failure line SHALL be bold red throughout. This applies to every
failure the command reports itself, including refusals of the inputs or output, another instance holding the claim,
a stop by Ctrl-C, and each image that fails in a batch. Argument errors detected while parsing the command line, and
`--help` and `--version`, SHALL be printed as the argument parser prints them, with no failure line.

#### Scenario: Success

- **WHEN** a run writes every output
- **THEN** the exit status is 0

#### Scenario: Write fails

- **WHEN** the output file's directory does not exist
- **THEN** the command ends with a failure line `🧨 <reason>` naming the output path, and exits with status 1

#### Scenario: Unreadable input

- **WHEN** the user runs `opai missing.jpg --up -o out.png` and `missing.jpg` does not exist
- **THEN** the command prints a single failure line `🧨 <reason>` naming `missing.jpg`, and exits with status 2

#### Scenario: One image of several fails

- **WHEN** a batch of three images is run and the second fails to be written
- **THEN** the second ends with its own failure line, the third is still enhanced and written, and the command exits
  with status 1

#### Scenario: Unknown flag

- **WHEN** the user runs `opai photo.jpg --up --vignette -o out.png`
- **THEN** the argument parser's own error is printed, with no failure line, and the exit status is 2

## ADDED Requirements

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
