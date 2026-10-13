# CLI Reference

`opai` is the command-line version of **Open Photo AI**. It runs the same AI models as the GUI. Each command enhances one image and writes one file, so it fits well in scripts and batch jobs.

- [Synopsis](#synopsis)
- [Quick start](#quick-start)
- [Arguments and global options](#arguments-and-global-options)
- [Enhancements](#enhancements)
- [Processing order](#processing-order)
- [Supported formats](#supported-formats)
- [What happens during a run](#what-happens-during-a-run)
- [Terminal output](#terminal-output)
- [Stopping a run](#stopping-a-run)
- [Exit codes](#exit-codes)
- [Log file](#log-file)
- [Common errors](#common-errors)
- [Scripting and batch processing](#scripting-and-batch-processing)

## Synopsis

```bash
opai <INPUT> <ENHANCEMENT FLAGS...> [--quality hd|sd] --output <FILE>
```

| Command          | What it does                                            |
|------------------|---------------------------------------------------------|
| `opai --help`    | Full help, with every flag, model and default (`-h` for a summary). |
| `opai --version` | Prints the version, e.g. `opai 26.10.0`. (`-V` also works.) |

The rules in short:

- `INPUT` comes **first**, before any flag.
- You need **at least one** enhancement flag.
- `--output` / `-o` is **required**. Its extension sets the output format.
- The order of the enhancement flags doesn't matter. The enhancements always run in a [fixed order](#processing-order).

## Quick start

```bash
# Upscale with the default model (Tokyo), at the scale the photo needs
opai photo.jpg --up -o photo-large.png

# Restore faces with the default model (Athens), then upscale 2x with Kyoto, and save as WebP
opai portrait.jpg --fr --up kyoto --scale 2 -o portrait.webp

# Upscale with Osaka at SD quality
opai scan.png --up osaka -q sd -o scan.avif

# Denoise at twice the normal strength with Gothenburg, then adjust light with Paris
opai night.jpg --dn gothenburg --dn-str 200 --la --la-bias -20 -o night-clean.jpg

# Colorize an old black & white photo, restore its faces and upscale it
opai grandparents.tif --cl --fr --up -o grandparents-restored.png
```

## Arguments and global options

### `INPUT` (required, positional)

The image to enhance. See [Supported formats](#supported-formats).

The input must come **before** the enhancement flags. Enhancement flags can take an optional model name, so the CLI reads the word after a flag as that model. For example, `opai --up photo.jpg -o out.png` fails because `photo.jpg` is read as an upscale model:

```
error: invalid value 'photo.jpg' for '--upscale [<MODEL>]'
  [possible values: kyoto, osaka, tokyo, saitama]
```

### `-o, --output <FILE>` (required)

The file to write. Its extension (any case: `.png`, `.PNG`) sets the format. See [Output formats](#output-formats).

- **An existing file at that path is overwritten without asking.**
- The directory must already exist.
- If the run fails or is stopped, nothing is written.

### `-q, --quality <hd|sd>` (default: `hd`)

Picks which build of each model to run. It applies to **every** enhancement in the command.

| Quality      | Most models | Osaka (upscale) | Trade-off                                     |
|--------------|-------------|-----------------|-----------------------------------------------|
| `hd` (default) | FP32      | FP16            | Highest quality. Slower, uses more memory.    |
| `sd`         | FP16        | INT8            | Lighter and usually faster. Slightly lower quality. |

The value is case-insensitive (`-q SD` works).

## Enhancements

Each enhancement is turned on by its flag. A flag with no model name uses that enhancement's **default model**. Add a model name after the flag to pick a different one:

```bash
opai photo.jpg --up -o out.png         # upscale with Tokyo (the default)
opai photo.jpg --up kyoto -o out.png   # upscale with Kyoto
```

- Every flag has a long name and a short alias; they do the same thing (`--upscale` = `--up`).
- Model names are case-insensitive (`--up OSAKA` = `--up osaka`).
- Use plain ASCII for model names: `malmo` (not `malmö`), `petersburg` (not `st-petersburg`), `saopaulo` (not `sao-paulo` or `são-paulo`).
- You can combine any number of enhancements in one command, but each one only once.

### Summary

Default models are in **bold**.

| Enhancement      | Flag                 | Alias  | Models                                         | Extra options              |
|------------------|----------------------|--------|------------------------------------------------|----------------------------|
| Denoise          | `--denoise`          | `--dn` | **stockholm**, gothenburg, malmo               | `--dn-str` (1 to 300)      |
| Face Recovery    | `--face-recovery`    | `--fr` | **athens**, santorini                          | none                       |
| Colorization     | `--colorization`     | `--cl` | **delhi**, mumbai, jaipur                      | none                       |
| Light Adjustment | `--light-adjustment` | `--la` | **paris**, lyon                                | `--la-bias` (-100 to 100)  |
| Color Balance    | `--color-balance`    | `--cb` | **rio**, saopaulo                              | `--cb-bias` (-100 to 100)  |
| Sharpen          | `--sharpen`          | `--sh` | **moscow**, petersburg, novgorod               | `--sh-str` (1 to 300)      |
| Upscale          | `--upscale`          | `--up` | **tokyo**, kyoto, osaka, saitama               | `--scale` (1.0 to 8.0)     |

The sections below explain each enhancement briefly. For more detail on when to pick each model, see the [Enhancements section of the README](../README.md#-enhancements).

### Strength and bias

Four enhancements take an extra option that controls how strong the effect is. Each option only works together with its own enhancement flag. For example, `--dn-str` without `--dn` fails with:

```
error: the following required arguments were not provided:
  --denoise [<MODEL>]
```

There are two kinds:

**Strength** (`--dn-str`, `--sh-str`): how much of the model's effect to apply.

| Value          | Effect                                                          |
|----------------|-----------------------------------------------------------------|
| `1`–`99`       | Weaker: the result is blended back towards the original image.  |
| `100` (default) | The model's own result.                                        |
| `101`–`300`    | Stronger: the effect is pushed past what the model produced.    |

**Bias** (`--la-bias`, `--cb-bias`): which direction to adjust in, and how far.

| Value           | Effect                                                |
|-----------------|-------------------------------------------------------|
| `-100`–`-1`     | Adjusts in the **opposite** direction to the model.   |
| `0`             | No change. The image stays as it was.                 |
| `50` (default)  | Half of the model's correction.                       |
| `100`           | The model's full correction.                          |

Both must be **whole numbers**. Values like `150.5` or `0.5` are rejected. A strength of `0` is also rejected, because it would have no effect; to skip an enhancement, leave its flag out.

### Denoise: `--denoise [MODEL]`, `--dn [MODEL]`

Removes noise from the image.

| Model         | Use it for                                                                           |
|---------------|--------------------------------------------------------------------------------------|
| `stockholm` (default) | Fast, high-quality removal of real camera sensor noise.                      |
| `gothenburg`  | Best quality on real-world sensor noise (low light, high ISO).                       |
| `malmo`       | Removing rain streaks from outdoor photos.                                           |

Option: `--dn-str <STRENGTH>`, a whole number from 1 to 300 (default 100). See [Strength and bias](#strength-and-bias).

### Face Recovery: `--face-recovery [MODEL]`, `--fr [MODEL]`

Finds the faces in the image and restores them.

| Model        | Use it for                                                                                   |
|--------------|----------------------------------------------------------------------------------------------|
| `athens` (default) | Keeping the person's identity. Runs at maximum fidelity to the original face.          |
| `santorini`  | Sharper, more polished faces. Can change the person's look on very low-quality inputs.       |

How it works:

1. Faces are detected automatically. You can't choose which faces to restore.
2. Only faces up to the size of a 512 × 512 pixel square (measured by area) are restored. Larger faces already have more detail than the model can add, and restoring them would soften them.
3. If no faces are found, or face detection fails, the rest of the run continues normally and the faces are left unchanged. The [log file](#log-file) records how many faces were found and restored.

No extra options.

### Colorization: `--colorization [MODEL]`, `--cl [MODEL]`

Adds color to black & white photos.

| Model        | Use it for                                                                      |
|--------------|---------------------------------------------------------------------------------|
| `delhi` (default) | Most photos. Rich, confident, lifelike colors.                              |
| `mumbai`     | Softer, more vintage-looking colors.                                            |
| `jaipur`     | Difficult photos where consistency matters. More muted, but few odd color patches. |

No extra options.

### Light Adjustment: `--light-adjustment [MODEL]`, `--la [MODEL]`

Fixes exposure and lighting.

| Model     | Use it for                                                                                  |
|-----------|---------------------------------------------------------------------------------------------|
| `paris` (default) | Uneven light: night scenes, backlit subjects, faces in shadow. Very fast.           |
| `lyon`    | Photos that are too bright or too dark overall.                                             |

Option: `--la-bias <BIAS>`, a whole number from -100 to 100 (default 50). Negative values work as you'd expect, e.g. `--la-bias -20`. See [Strength and bias](#strength-and-bias).

### Color Balance: `--color-balance [MODEL]`, `--cb [MODEL]`

Corrects white balance and color casts.

| Model      | Use it for                                                                                     |
|------------|------------------------------------------------------------------------------------------------|
| `rio` (default) | A color cast over the whole photo (orange indoors, blue in shade). The faster of the two. |
| `saopaulo` | Mixed light in one photo, like a warm lamp and daylight from a window.                         |

Option: `--cb-bias <BIAS>`, a whole number from -100 to 100 (default 50). See [Strength and bias](#strength-and-bias).

### Sharpen: `--sharpen [MODEL]`, `--sh [MODEL]`

Removes blur.

| Model        | Use it for                                                                     |
|--------------|--------------------------------------------------------------------------------|
| `moscow` (default) | Out-of-focus blur (the lens focused on the wrong thing).                 |
| `petersburg` | Motion blur and camera shake, fast and light on resources.                     |
| `novgorod`   | Motion blur and camera shake, best quality but slower.                         |

Option: `--sh-str <STRENGTH>`, a whole number from 1 to 300 (default 100). See [Strength and bias](#strength-and-bias).

### Upscale: `--upscale [MODEL]`, `--up [MODEL]`

Increases the image's resolution.

| Model      | Use it for                                                                                        |
|------------|---------------------------------------------------------------------------------------------------|
| `tokyo` (default) | A natural upscale that stays faithful to the original and doesn't invent detail.           |
| `kyoto`    | Real-world photos with noise, blur or compression. Fast, robust results.                          |
| `saitama`  | Cartoons, drawings, line art and digital illustrations.                                           |
| `osaka`    | The best quality available. Slow; needs a powerful GPU. Its builds are FP16 (`hd`) and INT8 (`sd`). |

Option: `--scale <FACTOR>`, the upscale factor, from `1.0` to `8.0`. Decimals are allowed (e.g. `2.5`).

**Without `--scale`**, the factor is chosen from the input image's size:

| Input size (width × height)              | Factor     |
|------------------------------------------|------------|
| Up to 1,048,576 pixels (1 MP, e.g. 1024 × 1024) | 4×  |
| Up to 4,194,304 pixels (4 MP, e.g. 2048 × 2048) | 2×  |
| Above 4 MP                                | No upscale |

> [!IMPORTANT]
> If the image is larger than 4 MP and you don't pass `--scale`, the upscale step is **skipped**. The terminal doesn't warn about this; it's only recorded in the [log file](#log-file). Any other enhancements in the command still run, and the file is still written at the original resolution. To upscale a large image anyway, pass `--scale`.

## Processing order

The enhancements always run in this order, no matter what order you type the flags in:

```
Denoise → Face Recovery → Colorization → Light Adjustment → Color Balance → Sharpen → Upscale
```

So these two commands do exactly the same thing:

```bash
opai photo.jpg --up --dn -o out.png
opai photo.jpg --dn --up -o out.png
```

The order is designed so that every step works on the cleanest image possible. For example, noise is removed before faces are restored, and the image is upscaled last so the other models work at the original resolution.

## Supported formats

### Input formats

The input format is detected from the **file extension** (any case), so the file needs a correct one. A JPEG named `photo.dat` is rejected.

| Format       | Extensions        |
|--------------|-------------------|
| BMP          | `.bmp`            |
| GIF          | `.gif`            |
| JPEG         | `.jpg`, `.jpeg`   |
| PNG          | `.png`            |
| TIFF         | `.tiff`, `.tif`   |
| AVIF         | `.avif`           |
| HEIF / HEIC  | `.heif`, `.heic`  |
| WebP         | `.webp`           |
| Camera RAW   | `.3fr`, `.ari`, `.arw`, `.cr2`, `.cr3`, `.crm`, `.crw`, `.dcr`, `.dcs`, `.dng`, `.erf`, `.fff`, `.iiq`, `.kdc`, `.mef`, `.mos`, `.mrw`, `.nef`, `.nrw`, `.orf`, `.ori`, `.pef`, `.qtk`, `.raf`, `.raw`, `.rw2`, `.rwl`, `.srw`, `.x3f` |

Camera RAW files are developed to 16 bits per channel before processing. Some other RAW formats are recognised but can't be decoded yet (for example GoPro `.gpr`, older Sony `.srf`/`.sr2` and Kodak `.k25`); those are rejected with an error.

### Output formats

| Format      | Extensions        | Compression | Default quality | 16-bit output |
|-------------|-------------------|-------------|-----------------|---------------|
| PNG         | `.png`            | Lossless    | n/a             | Yes           |
| TIFF        | `.tiff`, `.tif`   | Lossless    | n/a             | Yes           |
| BMP         | `.bmp`            | Lossless    | n/a             | No            |
| GIF         | `.gif`            | Lossless (256 colors) | n/a   | No            |
| JPEG        | `.jpg`, `.jpeg`   | Lossy       | 90              | No            |
| WebP        | `.webp`           | Lossy       | 75              | No            |
| AVIF        | `.avif`           | Lossy       | 60              | Yes           |
| HEIF / HEIC | `.heif`, `.heic`  | Lossy       | 60              | Yes           |

The quality and compression settings are the same ones the GUI uses when exporting. They can't currently be changed from the CLI.

**Bit depth:** the output keeps the bit depth of the input. A 16-bit source (a 16-bit PNG/TIFF or any camera RAW) produces a 16-bit result, saved as 16-bit if the output format supports it. For RAW files, use PNG, TIFF, AVIF or HEIF to keep the extra depth.

**Transparency is not kept.** Transparent areas are filled with black during processing.

## What happens during a run

1. **The command is checked.** The arguments, the output extension and the input file are all checked before anything is downloaded or loaded. A typo fails right away with [exit code 2](#exit-codes).
2. **Dependencies are installed (first run only).** The first time `opai` runs, it downloads the components it needs: ONNX Runtime, plus CUDA and TensorRT on systems with a supported NVIDIA GPU. A `💾 Downloading dependency …` line shows the progress. These are shared with the GUI, so if the GUI has already downloaded them, nothing is downloaded.
3. **The image is loaded.**
4. **Models are downloaded (first use only).** Each model is downloaded the first time it's used. The progress bar shows `Downloading` while this happens.
5. **The enhancements run**, in the [fixed order](#processing-order), with a single progress bar for the whole run. When `--fr` is used, face detection is included in that bar.
6. **The file is written** and the success line is printed.

### Hardware acceleration

The CLI automatically uses the fastest hardware available, trying in this order: **TensorRT → CUDA → CoreML → WebGPU → CPU**. You can't choose the hardware from the command line. The [log file](#log-file) records which hardware each run used.

> [!NOTE]
> With TensorRT, the first run of each model can take several minutes while TensorRT optimises it. Later runs are much faster.

### Run cache

Every step's result is cached on disk for **24 hours**. If you run the same enhancements on the same image again within that time, the cached results are reused and the run finishes almost immediately. Changing anything (a model, strength, bias, scale or quality) creates a new cache entry from that step onwards.

### One instance at a time

Only one Open Photo AI process can run at a time, and that includes the GUI. If another one is running, `opai` exits with [code 1](#exit-codes) and the error `Open Photo AI is already running (…); close it and try again`, naming the other process when it can (for example `gui, pid 4821`). Close it, or wait for it to finish, and try again. To process several images, run them one after another (see [Scripting](#scripting-and-batch-processing)).

## Terminal output

Everything `opai` prints goes to **stderr**. Nothing is written to stdout.

When stderr is a terminal, you see live progress:

```
💾 Downloading dependency ONNX Runtime   34%          ← only when something is being downloaded
🖼️ Applying enhancements to the image photo.jpg

Face Recovery   [1/2]  ████████████░░░░░░░░░░░░░░░   38.4%

✨ Image enhanced successfully; file saved at out.png
```

- The progress bar shows the enhancement currently running and its position in the run (`[1/2]` means the first of two enhancements).
- If something goes wrong, the last line is `🧨` followed by a short description of the error.

When stderr is **not** a terminal (for example, when redirected to a file or run in CI), there's no progress bar. Only the header line and the final `✨`/`🧨` line are printed, without colors.

**Colors** are turned off when the [`NO_COLOR`](https://no-color.org) environment variable is set to any non-empty value:

```bash
NO_COLOR=1 opai photo.jpg --up -o out.png
```

The terminal shows only the basic progress and the result. More detail, such as a skipped upscale, how many faces were found, the hardware used and full error messages, goes to the [log file](#log-file).

## Stopping a run

Press **Ctrl-C** at any time to stop.

- **While enhancing:** the run stops at the next safe point, nothing is written, and the line `🧨 stopped: nothing was written` is printed. Exit code: `130`.
- **While downloading dependencies:** the download stops. Next time, it resumes from where it left off instead of starting over. Exit code: `130`.

Any file that already exists at the output path is left untouched.

## Exit codes

| Code  | Meaning                                                                                                     |
|-------|-------------------------------------------------------------------------------------------------------------|
| `0`   | Success. The output file was written.                                                                      |
| `1`   | The run failed: setup error (including another instance running), enhancement error, or the file couldn't be written. |
| `2`   | Usage error: bad arguments, an unreadable or unsupported input, or an output extension that can't be written. |
| `130` | Stopped with Ctrl-C. Nothing was written.                                                                  |

`--help` and `--version` exit with `0`. Whenever the exit code isn't `0`, no output file is written.

## Log file

Each run writes detailed records to a log file shared by the CLI and the GUI:

| OS      | Path                                                              |
|---------|-------------------------------------------------------------------|
| macOS   | `~/Library/Application Support/io.vinicius.opai/logs/opai.log`    |
| Linux   | `~/.config/io.vinicius.opai/logs/opai.log`                        |
| Windows | `%APPDATA%\io.vinicius.opai\logs\opai.log`                        |

What's in it that the terminal doesn't show:

- An upscale that was skipped because the image was already large enough.
- Face detection results, e.g. `found 3 faces, restoring 2`, or `no faces were found; none will be restored`.
- The output's size and format, and the hardware it ran on, e.g. `ran on CoreML, CPU` or `served from cache`.
- The full error behind any `🧨` line.

**Please attach this file when [reporting a bug](https://github.com/vegidio/open-photo-ai/issues).**

## Common errors

| What you see                                                                 | Cause and fix |
|-------------------------------------------------------------------------------|---------------|
| `error: at least one enhancement is required: --denoise (--dn), …`             | No enhancement flag was given. Add at least one. |
| `error: the following required arguments were not provided: --output <FILE>`  | `-o`/`--output` is missing. |
| `error: invalid value 'foo' for '--upscale [<MODEL>]'` followed by `[possible values: …]` | Unknown model name. Pick one from the list shown. |
| `error: invalid value 'photo.jpg' for '--upscale [<MODEL>]'`                   | The input was placed after a flag. Put `INPUT` first. |
| `error: invalid value '9' for '--scale <FACTOR>': the scale must be from 1.0 to 8.0` | `--scale` out of range or not a number. |
| `error: invalid value '0.5' for '--la-bias <BIAS>': the bias must be a whole number from -100 to 100` | Strength/bias must be a whole number within range. |
| `error: the following required arguments were not provided: --denoise [<MODEL>]` | A strength/bias/scale option was used without its enhancement flag (e.g. `--dn-str` without `--dn`, or `--scale` without `--up`). |
| `🧨 cannot write out.xyz: .xyz is not a format opai can write; use one of: …` | The output extension isn't a supported format. Use one from the list. |
| `🧨 cannot write out: it has no extension to choose a format by; …`            | The output path has no extension. |
| `🧨 failed to read photo.jpg: No such file or directory …`                     | The input doesn't exist, can't be read, or its extension isn't a supported format. |
| `🧨 Open Photo AI is already running …; close it and try again`                | Another `opai` process or the GUI is running. Close it and try again. |
| `🧨 stopped: nothing was written`                                              | You pressed Ctrl-C. |

## Scripting and batch processing

Because `opai` handles one image per command, process folders with a loop. Each image runs after the previous one finishes, which also follows the [one-instance rule](#one-instance-at-a-time).

**macOS / Linux (bash or zsh):**

```bash
mkdir -p enhanced
for f in photos/*.jpg; do
  name=$(basename "${f%.*}")
  if ! opai "$f" --dn --up -o "enhanced/$name.png"; then
    echo "failed: $f" >&2
  fi
done
```

**Windows (PowerShell):**

```powershell
New-Item -ItemType Directory -Force enhanced | Out-Null
Get-ChildItem photos\*.jpg | ForEach-Object {
  opai $_.FullName --dn --up -o "enhanced\$($_.BaseName).png"
  if ($LASTEXITCODE -ne 0) { Write-Warning "failed: $($_.Name)" }
}
```

Tips:

- Check the [exit code](#exit-codes) to tell usage errors (`2`), which won't succeed on retry, apart from run failures (`1`).
- Use exit code `130` to stop the whole loop when the user presses Ctrl-C, e.g. `[ $? -eq 130 ] && break` in bash.
