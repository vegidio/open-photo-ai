# CLI Reference

`opai` is the command-line version of **Open Photo AI**. It runs the same AI models as the GUI. One command can enhance a single image or a whole batch: any mix of files, wildcards and directories, written to one file or into an output directory.

- [Synopsis](#synopsis)
- [Quick start](#quick-start)
- [Arguments and global options](#arguments-and-global-options)
- [Where the results go](#where-the-results-go)
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
opai <INPUT>... <ENHANCEMENT FLAGS...> [OPTIONS] --output <OUTPUT>
```

| Command          | What it does                                            |
|------------------|---------------------------------------------------------|
| `opai --help`    | Full help, with every flag, model and default (`-h` for a summary). |
| `opai --version` | Prints the version, e.g. `opai 26.10.0`. (`-V` also works.) |

The rules in short:

- The `INPUT`s come **first**, before any flag. Each one is an image or a directory of images.
- You need **at least one** enhancement flag.
- `--output` / `-o` is **required**. It's a [file or a directory](#where-the-results-go).
- The order of the enhancement flags doesn't matter. The enhancements always run in a [fixed order](#processing-order).
- An existing file is **never overwritten** unless you pass `--overwrite`. The result is [numbered](#numbering-and---overwrite) instead.

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

# Denoise every JPEG in the current directory into the directory "enhanced"
opai *.jpg --dn -o enhanced

# Upscale a whole tree of photos, mirroring its folders, and save everything as WebP
opai photos -r --up --format webp -o photos-large

# Denoise a directory in place, replacing each photo with its result
opai photos --dn --overwrite -o photos
```

## Arguments and global options

### `INPUT...` (required, positional)

One or more images or directories, in any mix. See [Supported formats](#supported-formats).

- **A file** is enhanced as it is. Its extension must be one `opai` reads, so a JPEG named `photo.dat` is rejected.
- **A directory** contributes every image directly inside it, or its whole tree with [`--recursive`](#-r---recursive). Only files whose extension (any case) is one `opai` reads are picked up; other files are ignored silently. Links to directories found inside it are not followed.
- **Wildcards** such as `*.jpg` work in every shell, including `cmd` and PowerShell on Windows, where `opai` expands them itself. A wildcard that matches nothing is kept as written, and fails as a missing input.
- **Order:** the images are enhanced in the order the inputs were given, and in name order within each directory.
- **Duplicates:** an image reached twice (named twice, named and also found in a directory, or found in two overlapping directories) is enhanced once, where it was first reached.

The inputs must come **before** the enhancement flags. Enhancement flags can take an optional model name, so the CLI reads the word after a flag as that model. For example, `opai --up photo.jpg -o out.png` fails because `photo.jpg` is read as an upscale model:

```
error: invalid value 'photo.jpg' for '--upscale [<MODEL>]'
  [possible values: kyoto, osaka, tokyo, saitama]
```

### `-o, --output <OUTPUT>` (required)

Where the results go: one file, or a directory. See [Where the results go](#where-the-results-go).

### `-r, --recursive`

A directory input contributes its **whole tree**, not only the images directly inside it. The results [mirror](#mirroring-directory-inputs) the tree's folders.

### `--format <FORMAT>` (default: `preserve`)

The format every result is written in. Only accepted when the output is a **directory**; with an output file, its extension chooses the format.

| Value | Result |
|-------|--------|
| `preserve` (default) | Each image keeps its own format and extension, written in lowercase (`a.JPG` gives `a.jpg`). Camera RAW (and anything else `opai` can't write) is saved as TIFF. |
| `avif`, `bmp`, `gif`, `heic`, `jpeg`, `png`, `tiff`, `webp` | Every result in this format. JPEG is saved with the extension `.jpg`. |

The value is case-insensitive, and `jpg`, `tif` and `heif` are accepted for `jpeg`, `tiff` and `heic`.

### `--prefix <TEXT>`, `--suffix <TEXT>`

Text added before and after each image's name, before the extension: `--prefix new- --suffix -opai` turns `beach.jpg` into `new-beach-opai.jpg`. Only accepted when the output is a **directory**. Both are empty by default.

### `--overwrite`

Replaces a file that already exists at a destination instead of [numbering](#numbering-and---overwrite) the result.

### `-q, --quality <hd|sd>` (default: `hd`)

Picks which build of each model to run. It applies to **every** enhancement in the command.

| Quality      | Most models | Osaka (upscale) | Trade-off                                     |
|--------------|-------------|-----------------|-----------------------------------------------|
| `hd` (default) | FP32      | FP16            | Highest quality. Slower, uses more memory.    |
| `sd`         | FP16        | INT8            | Lighter and usually faster. Slightly lower quality. |

The value is case-insensitive (`-q SD` works).

## Where the results go

### A file or a directory

`-o` is read like this, in order:

1. **An existing directory** is a directory, whatever its name. `-o shots.png` writes into the directory `shots.png` if it exists.
2. **A path whose extension** (any case: `.png`, `.PNG`) is one `opai` [writes](#output-formats) is a **file**, saved in that format.
3. **Anything else** is a **directory**. It's created, with any missing parents, when the first result is written into it. `-o out` and `-o out.xyz` are both directories.

An output **file** takes exactly **one input file**: more than one input, or a directory input, fails with exit code `2`. So do `--format`, `--prefix` and `--suffix`, which need an output directory. The file's own directory must already exist.

An output **directory** fails with exit code `2` if a file (not a directory) already exists at that path.

To write into a new directory whose name ends in an image extension, create it first.

### Mirroring directory inputs

Under an output directory:

- **An image found in a directory input** is written at its path relative to that input. With `opai photos -r --up -o out`, `photos/a.jpg` becomes `out/a.jpg` and `photos/2024/summer/b.jpg` becomes `out/2024/summer/b.jpg`. Missing folders are created.
- **A file named as an input** goes directly into the output directory. `opai trips/2024/a.jpg home/b.jpg --up -o out` writes `out/a.jpg` and `out/b.jpg`.
- **The output directory is never taken as input.** `opai photos -r --up -o photos/enhanced` skips everything already in `photos/enhanced`, so a rerun doesn't enhance its own earlier results. (A file inside it that you name explicitly is still enhanced.)
- **In place:** with `-o` naming the input directory itself (`opai photos --dn -o photos`), each result lands beside its source.

The file name is the same one the GUI's export would use: the prefix, the source's name, the suffix, and the extension of the [`--format`](#--format-format-default-preserve) chosen.

### Numbering and `--overwrite`

> [!IMPORTANT]
> **Changed in this version:** `opai` used to overwrite an existing output file without asking. It now keeps it and numbers the new result instead. Pass `--overwrite` to get the old behaviour.

**Without `--overwrite`**, a file that already exists at a destination is kept, and the result is written to the first free name made by adding `_1`, `_2`, … up to `_999` before the extension, exactly as the GUI's export does. This applies to an output file too: if `out.png` exists, `opai photo.jpg --up -o out.png` writes `out_1.png`. Two inputs with the same name going to the same place (`opai one/a.jpg two/a.jpg --up -o out`) give `out/a.jpg` and `out/a_1.jpg`. In place, each result is numbered beside its source (`photos/a_1.jpg`).

**With `--overwrite`**, the existing file is replaced, without a prompt. The new file is written under a hidden name beside it first, and only renamed over it once it's complete, so a failed or stopped write leaves the original exactly as it was and nothing beside it. This makes in-place enhancement safe: `opai photos --dn --overwrite -o photos` replaces each photo with its denoised result, in its own format. This works when the result's name is the source's own name: the name follows the [export rule](#--format-format-default-preserve), so the extension is written in lowercase and camera RAW becomes TIFF. A camera RAW source (`b.nef`) keeps its file and gets `b.tiff` beside it. An uppercase extension (`a.JPG`) is written as `a.jpg`, which replaces the source on a filesystem that ignores case (the macOS and Windows defaults) but sits beside it on one that doesn't (most Linux filesystems). With two inputs writing to the same name, the second result replaces the first.

The success line always names the file actually written, including its number.

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

The input format is detected from the **file extension** (any case), so the file needs a correct one. A JPEG named `photo.dat` is rejected when named, and ignored inside a directory.

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

With an output **file**, the format is the one its extension names. With an output **directory**, it's chosen by [`--format`](#--format-format-default-preserve). The quality and compression settings are the same ones the GUI uses when exporting. They can't currently be changed from the CLI.

**Bit depth:** the output keeps the bit depth of the input. A 16-bit source (a 16-bit PNG/TIFF or any camera RAW) produces a 16-bit result, saved as 16-bit if the output format supports it. For RAW files, use PNG, TIFF, AVIF or HEIF to keep the extra depth.

**Transparency is not kept.** Transparent areas are filled with black during processing.

## What happens during a run

1. **The command is checked.** The arguments, the output, and every input are all checked before anything is downloaded or loaded. A typo fails right away with [exit code 2](#exit-codes), even if it's in the fortieth path. Specifically, the run is refused if:
   - an input you named (a file or a directory) doesn't exist or can't be read;
   - a file you named isn't an image `opai` can read;
   - a directory input's tree can't be listed (for example, a subfolder you don't have permission to read), naming that directory;
   - the output can't be used as asked (see [A file or a directory](#a-file-or-a-directory));
   - no image is left to enhance once the inputs are expanded, e.g. a directory with no images in it.

   A file **found inside a directory** that has an image extension but can't be read is not a refusal: it's skipped, recorded in the [log file](#log-file), and the rest of the batch goes on. Nothing is written or created when the run is refused.
2. **Dependencies are installed (first run only).** The first time `opai` runs, it downloads the components it needs: ONNX Runtime, plus CUDA and TensorRT on systems with a supported NVIDIA GPU. A `💾 Downloading dependency …` line shows the progress. These are shared with the GUI, so if the GUI has already downloaded them, nothing is downloaded. This happens once per command, however many images it has.
3. **Each image, one after another:**
   1. **The image is loaded.**
   2. **Models are downloaded (first use only).** Each model is downloaded the first time it's used. The progress bar shows `Downloading` while this happens.
   3. **The enhancements run**, in the [fixed order](#processing-order), with a single progress bar for the image. When `--fr` is used, face detection is included in that bar. The upscale factor, when `--scale` isn't given, is chosen from each image's own size.
   4. **The file is written** and the success line is printed, naming the file written.

**A batch continues past a failure.** An image that fails to decode, to be enhanced or to be written gets its own `🧨` line naming it, and the next image runs. At the end, the command exits with [code 1](#exit-codes) if any image failed.

### Hardware acceleration

The CLI automatically uses the fastest hardware available, trying in this order: **TensorRT → CUDA → CoreML → WebGPU → CPU**. You can't choose the hardware from the command line. The [log file](#log-file) records which hardware each image used.

> [!NOTE]
> With TensorRT, the first run of each model can take several minutes while TensorRT optimises it. Later runs are much faster.

### Run cache

Every step's result is cached on disk for **24 hours**. If you run the same enhancements on the same image again within that time, the cached results are reused and the run finishes almost immediately. Changing anything (a model, strength, bias, scale or quality) creates a new cache entry from that step onwards.

### One instance at a time

Only one Open Photo AI process can run at a time, and that includes the GUI. If another one is running, `opai` exits with [code 1](#exit-codes) and the error `Open Photo AI is already running (…); close it and try again`, naming the other process when it can (for example `gui, pid 4821`). Close it, or wait for it to finish, and try again. To process several images, give them all to one command (see [Scripting](#scripting-and-batch-processing)).

## Terminal output

Everything `opai` prints goes to **stderr**. Nothing is written to stdout.

When stderr is a terminal, you see live progress. Each image gets its own header, bar and result line:

```
💾 Downloading dependency ONNX Runtime   34%          ← only when something is being downloaded
🖼️ Applying enhancements to the image photos/a.jpg

Face Recovery   [1/2]  ████████████░░░░░░░░░░░░░░░   38.4%

✨ Image enhanced successfully; file saved at out/a.jpg
🖼️ Applying enhancements to the image photos/b.jpg

Upscale         [2/2]  ███████████████████████████  100.0%

🧨 could not enhance photos/b.jpg: …
🧨 1 of 2 images failed
```

- The progress bar shows the enhancement currently running and its position in the image's chain (`[1/2]` means the first of two enhancements).
- If an image fails, its result line is `🧨` followed by the image and a short description of the error. If any image failed, a last `🧨` line says how many.

When stderr is **not** a terminal (for example, when redirected to a file or run in CI), there's no progress bar. Only each image's header line and its `✨`/`🧨` line are printed, without colors.

**Colors** are turned off when the [`NO_COLOR`](https://no-color.org) environment variable is set to any non-empty value:

```bash
NO_COLOR=1 opai photo.jpg --up -o out.png
```

The terminal shows only the basic progress and the result. More detail, such as a skipped upscale, a skipped unreadable file, how many faces were found, the hardware used and full error messages, goes to the [log file](#log-file).

## Stopping a run

Press **Ctrl-C** at any time to stop.

- **While enhancing:** the run stops at the next safe point. Nothing is written for the image being enhanced, and no image after it is started. Files already written for earlier images are **kept**. The line `🧨 stopped: nothing more was written` is printed. Exit code: `130`.
- **While downloading dependencies:** the download stops. Next time, it resumes from where it left off instead of starting over. Nothing is written. Exit code: `130`.

A file that the stopped image would have replaced with `--overwrite`, including its own source in place, is left exactly as it was, with no hidden file beside it.

## Exit codes

| Code  | Meaning                                                                                                     |
|-------|-------------------------------------------------------------------------------------------------------------|
| `0`   | Success. Every image was written.                                                                          |
| `1`   | The run failed: a setup error (including another instance running), or **any** image that failed to decode, enhance or be written. The other images in the batch were still processed. |
| `2`   | Usage error: bad arguments, an unreadable or unsupported named input, an output that can't be used as asked, or no image to enhance. Nothing was written. |
| `130` | Stopped with Ctrl-C. Results already written for earlier images are kept.                                  |

`--help` and `--version` exit with `0`.

## Log file

Each run writes detailed records to a log file shared by the CLI and the GUI:

| OS      | Path                                                              |
|---------|-------------------------------------------------------------------|
| macOS   | `~/Library/Application Support/io.vinicius.opai/logs/opai.log`    |
| Linux   | `~/.config/io.vinicius.opai/logs/opai.log`                        |
| Windows | `%APPDATA%\io.vinicius.opai\logs\opai.log`                        |

What's in it that the terminal doesn't show:

- An upscale that was skipped because the image was already large enough.
- A file found in a directory that was skipped because it couldn't be read, and why.
- Face detection results, e.g. `found 3 faces, restoring 2`, or `no faces were found; none will be restored`.
- Each output's size and format, and the hardware it ran on, e.g. `ran on CoreML, CPU` or `served from cache`.
- The full error behind any `🧨` line.

**Please attach this file when [reporting a bug](https://github.com/vegidio/open-photo-ai/issues).**

## Common errors

| What you see                                                                 | Cause and fix |
|-------------------------------------------------------------------------------|---------------|
| `error: at least one enhancement is required: --denoise (--dn), …`             | No enhancement flag was given. Add at least one. |
| `error: the following required arguments were not provided: --output <OUTPUT>` | `-o`/`--output` is missing. |
| `error: invalid value 'foo' for '--upscale [<MODEL>]'` followed by `[possible values: …]` | Unknown model name. Pick one from the list shown. |
| `error: invalid value 'photo.jpg' for '--upscale [<MODEL>]'`                   | An input was placed after a flag. Put the `INPUT`s first. |
| `error: invalid value '9' for '--scale <FACTOR>': the scale must be from 1.0 to 8.0` | `--scale` out of range or not a number. |
| `error: invalid value '0.5' for '--la-bias <BIAS>': the bias must be a whole number from -100 to 100` | Strength/bias must be a whole number within range. |
| `error: the following required arguments were not provided: --denoise [<MODEL>]` | A strength/bias/scale option was used without its enhancement flag (e.g. `--dn-str` without `--dn`, or `--scale` without `--up`). |
| `error: invalid value 'raw' for '--format <FORMAT>': "raw" is not a format; use preserve, …` | Unknown `--format`. Pick one from the list shown. |
| `🧨 cannot read missing.jpg: No such file or directory …`                      | A named input doesn't exist or can't be read. If it's a wildcard, it matched nothing. |
| `🧨 the codec refused notes.txt: could not determine an image format …`        | A named file isn't an image `opai` can read, or its extension isn't a supported format. |
| `🧨 cannot list photos: …`                                                     | Part of a directory input's tree can't be read. Fix its permissions, or leave it out. |
| `🧨 no image to enhance in photos`                                             | The inputs hold no image `opai` can read. Add `-r` if the images are in subfolders. |
| `🧨 an output file takes exactly one input file; …`                            | `-o` names a file, but there are several inputs or a directory input. Give a directory instead. |
| `🧨 --suffix needs an output directory; …`                                     | `--format`, `--prefix` and `--suffix` only work when `-o` is a directory. |
| `🧨 cannot write into out: it is a file, not a directory, …`                   | `-o` would be a directory, but a file already exists there. |
| `🧨 cannot write out/a.png: …`                                                 | The result couldn't be written there: the directory is missing or not writable, or every numbered name up to `_999` is taken. |
| `🧨 Open Photo AI is already running …; close it and try again`                | Another `opai` process or the GUI is running. Close it and try again. |
| `🧨 could not enhance photos/b.jpg: …`                                         | That image failed; the rest of the batch went on. The log file has the full error. |
| `🧨 failed to decode photos/b.jpg: …`                                          | That image's header was readable, but the image itself wasn't; the rest of the batch went on. |
| `🧨 2 of 5 images failed`                                                      | The summary after a batch with failures. Exit code `1`. |
| `🧨 stopped: nothing more was written`                                         | You pressed Ctrl-C. |

## Scripting and batch processing

Give a whole batch to **one** command instead of looping: dependencies are checked once, the models stay loaded between images, and every input is checked before anything starts.

```bash
# Every JPEG in a folder, as PNG, into "enhanced"
opai photos/*.jpg --dn --up --format png -o enhanced

# A whole tree, mirrored into a new directory
opai photos -r --dn --up -o enhanced
```

The same commands work in PowerShell and `cmd` on Windows, where `opai` expands `*.jpg` itself.

Things to keep in mind in scripts:

- **Reruns don't overwrite.** Running the same command twice writes `a_1.jpg` next to `a.jpg`, and so on. Pass `--overwrite` if a rerun should replace the earlier results. Scripts written for older versions, which overwrote `-o` silently, now accumulate numbered files.
- **Exit codes:** `2` means the command itself is wrong and won't succeed on retry; nothing was written. `1` means at least one image failed, while the others were written; the `🧨` lines and the log file say which. `130` means the user pressed Ctrl-C.
- **One instance at a time:** don't start several `opai` commands in parallel; give them all the images instead.

If you still need a loop (for example, a different output name per image), check the exit code of each run:

```bash
mkdir -p enhanced
for f in photos/*.jpg; do
  name=$(basename "${f%.*}")
  opai "$f" --dn --up -o "enhanced/$name.png"
  case $? in
    0) ;;
    130) break ;;
    *) echo "failed: $f" >&2 ;;
  esac
done
```
