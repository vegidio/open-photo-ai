## MODIFIED Requirements

### Requirement: The command takes one input, its enhancements, and an output

The terminal application SHALL be invoked as `opai <input> [options] --output <file>`. The input path SHALL be the
first argument. The options SHALL be:

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
- `--output`, alias `-o`: the file to write. It is required.

A strength or bias SHALL be applied as that percentage of the model's own effect: a strength of 100 or a bias of 100
applies the model's output as it is, a strength of 200 applies twice that, and a negative bias shifts the image in the
opposite direction. A negative bias SHALL be accepted written as a plain number, as in `--la-bias -20`.

The order in which the enhancement flags are given SHALL NOT change the order they are applied in. That order is the
library's apply order: denoise, face recovery, colorization, light adjustment, colour balance, sharpen, and then
upscale.

Model names SHALL be matched without regard to case.

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

- **WHEN** the user runs `opai photo.jpg --up --vignette -o out.png`
- **THEN** the argument parser's own error is printed, with no failure line, and the exit status is 2
