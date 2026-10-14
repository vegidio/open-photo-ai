# CLI Reference

`opai` is the command-line version of **Open Photo AI**. It runs the same AI models as the GUI. One command can enhance a single image or a whole batch: any mix of files, wildcards and directories, written to one file or into an output directory.

- [Synopsis](#synopsis)
- [Quick start](#quick-start)
- [Arguments and global options](#arguments-and-global-options)
- [Where the results go](#where-the-results-go)
- [Enhancements](#enhancements)
- [Autopilot](#autopilot)
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
- You need **at least one** enhancement flag, or [`--autopilot`](#autopilot) to let `opai` choose them per image.
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

# Upscale on the CPU only, e.g. to rule out a GPU problem
opai photo.jpg --up -p cpu -o photo-large.png

# Let autopilot choose each photo's enhancements, as the GUI's Autopilot does
opai photos --ap -o enhanced
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

### `--autopilot`, `--ap`

Analyses each image and applies the enhancements it calls for, instead of the ones you name. It can't be combined with the enhancement flags or their options. See [Autopilot](#autopilot).

### `-q, --quality <hd|sd>` (default: `hd`)

Picks which build of each model to run. It applies to **every** enhancement in the command.

| Quality      | Most models | Osaka (upscale) | Trade-off                                     |
|--------------|-------------|-----------------|-----------------------------------------------|
| `hd` (default) | FP32      | FP16            | Highest quality. Slower, uses more memory.    |
| `sd`         | FP16        | INT8            | Lighter and usually faster. Slightly lower quality. |

The value is case-insensitive (`-q SD` works).

### `-p, --processor <PROCESSOR>` (default: `auto`)

Picks the hardware every model of the run is asked to run on, face detection included. Which values exist depends on the operating system `opai` was built for:

| Processor        | Runs on                                                                              | macOS | Windows / Linux |
|------------------|--------------------------------------------------------------------------------------|:-----:|:---------------:|
| `auto` (default) | The best this machine supports. See [Hardware acceleration](#hardware-acceleration). | ✓     | ✓               |
| `cpu`            | The CPU only.                                                                        | ✓     | ✓               |
| `coreml`         | Apple's CoreML (macOS 12 or newer).                                                  | ✓     |                 |
| `cuda`           | NVIDIA CUDA.                                                                         |       | ✓               |
| `tensorrt`       | NVIDIA TensorRT.                                                                     |       | ✓               |
| `webgpu`         | WebGPU: the GPU through Metal, Direct3D 12 or Vulkan.                                | ✓     | ✓               |

The value is case-insensitive (`-p CPU` and `--processor WebGPU` work). `opai --help` lists only the values your build accepts, and any other value is refused straight away, before anything is installed.

A value your build accepts is then checked against what **this machine** offers, once [dependencies are installed](#what-happens-during-a-run). This is the same check the GUI uses to build its list of processors: CUDA needs a supported NVIDIA GPU and driver, TensorRT also needs a GPU that can run it, CoreML needs macOS 12 or newer, and WebGPU needs a GPU the runtime can reach. If the machine doesn't offer the processor, the run stops with one line naming the processors it does offer, for example `🧨 the processor webgpu is not available on this machine; use auto, cpu or coreml`, and exits with [code 2](#exit-codes). No image is loaded and nothing is written.

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

**Without `--overwrite`**, a file that already exists at a destination is kept, and the result is written to the first free name made by adding `_1`, `_2`, … up to `_999` before the extension, exactly as the GUI's export does. This applies to an output file too: if `out.png` exists, `opai photo.jpg --up -o out.png` writes `out_1.png`. Two inputs with the same name going to the same place (`opai one/a.jpg two/a.jpg --up -o out`) give `out/a.jpg` and `out/a_1.jpg`. In place, each result is numbered beside its source (`photos/a_1.jpg`).

**With `--overwrite`**, the existing file is replaced, without a prompt. The new file is written under a hidden name beside it first, and only renamed over it once it's complete, so a failed or stopped write leaves the original exactly as it was and nothing beside it. This makes in-place enhancement safe: `opai photos --dn --overwrite -o photos` replaces each photo with its denoised result, in its own format. This works when the result's name is the source's own name: the name follows the [export rule](#--format-format-default-preserve), so the extension is written in lowercase and camera RAW becomes TIFF. A camera RAW source (`b.nef`) keeps its file and gets `b.tiff` beside it. An uppercase extension (`a.JPG`) is written as `a.jpg`, which replaces the source on a filesystem that ignores case (the macOS and Windows defaults) but sits beside it on one that doesn't (most Linux filesystems). With two inputs writing to the same name, the second result replaces the first.

For a single image, the summary line names the file actually written, including its number (`file saved at out_1.png`). For a batch, the [log file](#log-file) records each file written.

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

## Autopilot

`--autopilot` (alias `--ap`) runs the same analysis as the GUI's Autopilot on each image, once it's loaded, and applies the enhancements that image calls for: denoise for visible grain, face recovery when there are faces to restore, colorization for a black & white photo, light adjustment for a badly exposed one, color balance for a color cast, sharpen for a blurred one, and an upscale for a small one. Different images in one batch can get different enhancements. They still run in the [fixed order](#processing-order).

```bash
opai photos --ap -o enhanced
```

- **It replaces the enhancement flags.** Combining `--ap` with any enhancement flag (`--dn`, `--fr`, `--cl`, `--la`, `--cb`, `--sh`, `--up`) or their options (`--dn-str`, `--sh-str`, `--la-bias`, `--cb-bias`, `--scale`) fails with exit code `2`, for example `error: the argument '--autopilot' cannot be used with '--upscale [<MODEL>]'`. Every other option (`-q`, `-p`, `-r`, `--format`, `--prefix`, `--suffix`, `--overwrite`, `-o`) works as usual, and `-q` picks the build of the models autopilot runs.
- **Each enhancement uses its default strength or bias**: strength `100` for denoise and sharpen, bias `50` for light adjustment and color balance. An upscale runs at the factor the image's size calls for (see [Upscale](#upscale---upscale-model---up-model)); an image above 4 MP is not upscaled.
- **An image that needs nothing is still written**, unchanged, in the format asked for. Its row ends with `Enhancements applied to` as any other.
- **If face detection can't run** (for example, its model can't be downloaded), the image still gets the other enhancements, without face recovery, and the [log file](#log-file) records a warning.
- **What was chosen for each image** is recorded in the [log file](#log-file). The terminal shows each enhancement in the image's row while it runs.

### Choosing the models

Each enhancement runs with its default model unless an environment variable names another one:

| Enhancement      | Environment variable    | Default model | Accepted values                       |
|------------------|-------------------------|---------------|---------------------------------------|
| Denoise          | `OPAI_AP_DENOISE`       | Stockholm     | `stockholm`, `gothenburg`, `malmo`, `off` |
| Face Recovery    | `OPAI_AP_FACE`          | Athens        | `athens`, `santorini`, `off`          |
| Colorization     | `OPAI_AP_COLORIZATION`  | Delhi         | `delhi`, `mumbai`, `jaipur`, `off`    |
| Light Adjustment | `OPAI_AP_LIGHT`         | Paris         | `paris`, `lyon`, `off`                |
| Color Balance    | `OPAI_AP_COLOR`         | Rio           | `rio`, `saopaulo`, `off`              |
| Sharpen          | `OPAI_AP_SHARPEN`       | Moscow        | `moscow`, `petersburg`, `novgorod`, `off` |
| Upscale          | `OPAI_AP_UPSCALE`       | Tokyo         | `kyoto`, `osaka`, `tokyo`, `saitama`, `off` |

```bash
# Upscale with Osaka whenever autopilot upscales
OPAI_AP_UPSCALE=osaka opai photos --ap -o out

# Never sharpen or restore faces, and use Malmo for denoise
OPAI_AP_SHARPEN=off OPAI_AP_FACE=off OPAI_AP_DENOISE=malmo opai photos --ap -o out
```

- Model names are spelled as the enhancement flags take them, in any case (`Osaka` = `osaka`).
- **`off`** (any case) leaves that enhancement out entirely: it's never applied, and the analysis doesn't even look for it, so `OPAI_AP_FACE=off` skips face recovery.
- An **empty** value is the same as not setting the variable.
- An **invalid** value fails with exit code `2` before anything is downloaded, naming the variable and listing what it accepts:

  ```
  error: invalid value 'paris' for the environment variable OPAI_AP_UPSCALE
    [possible values: kyoto, osaka, tokyo, saitama, off]
  ```
- The variables are read **only with `--ap`**. Without it they're ignored and not checked: `OPAI_AP_UPSCALE=paris opai photo.jpg --up -o out.png` upscales with Tokyo. The GUI never reads them; its Autopilot uses the models chosen in its settings.

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
2. **Dependencies are installed (first run only).** The first time `opai` runs, it downloads the components it needs: ONNX Runtime, plus CUDA and TensorRT on systems with a supported NVIDIA GPU. A `💾 Downloading dependency <name>... ⣾ 34% Complete` line shows the progress, with a spinner that keeps turning while the download works; it is rewritten when the next dependency starts, and shows a green `✓` once the dependency reaches `100%`. These are shared with the GUI, so if the GUI has already downloaded them, nothing is downloaded. This happens once per command, however many images it has. Then the [processor](#-p---processor-processor-default-auto) is checked against what this machine offers, and the run is refused with [exit code 2](#exit-codes) if it doesn't offer it. Once it's accepted, the line `🧠 Running inference using the processor <processor>; quality <HD|SD>` names the processor you asked for (`Auto` under `-p auto`) and the quality, followed by a blank line.
3. **Each image, one after another:**
   1. **The image is loaded.** Its row in the list reads `🖼️ [  0%] Preparing to enhance <path>`. With [`--autopilot`](#autopilot), the image is then analysed, and its row reads `🖼️ [  0%] Choosing enhancements for <path>` until the analysis has chosen its enhancements.
   2. **Models are downloaded (first use only).** Each model is downloaded the first time it's used. The image's row reads `Downloading <enhancement> for <path>` while this happens.
   3. **The enhancements run**, in the [fixed order](#processing-order). The image's row reads `Applying <enhancement> to <path>`, with one percentage for the whole image. When `--fr` is used, face detection is included in that percentage and reads as `Face Recovery`. The upscale factor, when `--scale` isn't given, is chosen from each image's own size.
   4. **The file is written**, and the image's row reads `🖼️ [100%] Enhancements applied to <path>`. Until then its percentage stops at `99%`. The [log file](#log-file) records the file written.
4. **A summary is printed** once every image has run: how many images were written and where, and the reason for each image that failed.

**A batch continues past a failure.** An image that fails to decode, to be enhanced or to be written has its row turn into a red `🧨 [ 42%] Failed to enhance <path>` row, keeping the percentage it had reached, and the next image runs. At the end, its reason is listed under the view, and the command exits with [code 1](#exit-codes).

### Hardware acceleration

By default (`-p auto`), the CLI uses the fastest hardware available, trying in this order: **TensorRT → CUDA → CoreML → WebGPU → CPU**. To choose the hardware yourself, use [`-p, --processor`](#-p---processor-processor-default-auto). The [log file](#log-file) records which hardware each image used.

- **A processor that can't run a model falls back to the CPU.** If the processor you chose (or `auto` picked) can't open one of the models, that model runs on the CPU and the run goes on. The terminal doesn't say so. The log file does, for example `ran on CPU (TensorRT could not open every model)`.
- **`-p` doesn't change what is installed.** The first run downloads what this machine needs, whichever processor you choose: `-p cpu` on a machine with a supported NVIDIA GPU still downloads CUDA and TensorRT. They're shared with the GUI and with later `auto` runs.
- **The [run cache](#run-cache) isn't kept per processor.** Running the same enhancements on the same image again within 24 hours with another `-p` reuses the cached result, and nothing runs on the new processor. The log file says `served from cache`. To compare processors, use another image or change the enhancements.

> [!NOTE]
> With TensorRT, the first run of each model can take several minutes while TensorRT optimises it. Later runs are much faster.

### Run cache

Every step's result is cached on disk for **24 hours**. If you run the same enhancements on the same image again within that time, the cached results are reused and the run finishes almost immediately. Changing anything (a model, strength, bias, scale or quality) creates a new cache entry from that step onwards.

### One instance at a time

Only one Open Photo AI process can run at a time, and that includes the GUI. If another one is running, `opai` exits with [code 1](#exit-codes) and the error `Open Photo AI is already running (…); close it and try again`, naming the other process when it can (for example `gui, pid 4821`). Close it, or wait for it to finish, and try again. To process several images, give them all to one command (see [Scripting](#scripting-and-batch-processing)).

## Terminal output

Everything `opai` prints goes to **stderr**. Nothing is written to stdout.

When stderr is a terminal, a `🧠` line names the processor and quality first, then you see one live view for the whole run, whatever the number of images: the batch's progress bar, a blank row, and a list of the five most recent images. A batch of eight, while its seventh image is being enhanced:

```
💾 Downloading dependency ONNX Runtime... ✓ 100% Complete    ← only when something was downloaded
🧠 Running inference using the processor Auto; quality HD

Enhancing [6/8]  ███████████████████████░░░░░   80.2%   ETA 1m12s

🖼️ [100%] Enhancements applied to       photos/c.jpg
🖼️ [100%] Enhancements applied to       photos/d.jpg
🧨 [ 42%] Failed to enhance             photos/e.jpg
🖼️ [100%] Enhancements applied to       photos/f.jpg
🖼️ [ 42%] Applying Upscale to           photos/g.jpg
```

And once the run ends, the view stays as it was last drawn, with the summary under it:

```
✨ 7 images enhanced successfully; files saved under out
🧨 could not enhance photos/e.jpg: …
🧨 1 of 8 images failed
```

- **The dependency line** appears only when a dependency is downloaded or expanded. Its spinner keeps turning while the line is drawn, and becomes a `✓` while the percentage reads `100%`. When several dependencies are installed, the same line is rewritten for each one.
- **The processor line** names the processor you asked for with [`-p`](#-p---processor-processor-default-auto), spelled `Auto`, `CPU`, `CoreML`, `CUDA`, `TensorRT` or `WebGPU`, and the [quality](#-q---quality-hdsd-default-hd), `HD` or `SD`. Under `-p auto` it reads `Auto`, not the hardware `auto` picks; the [log file](#log-file) records that.
- **The progress bar** covers the whole batch and is always labelled `Enhancing`. `[n/N]` counts the images that have ended, written or failed; `n` is padded to the width of `N`, as in `[02/15]`. It fills as `(n + p) / N`, where `p` is the current image's progress as its row shows it, so it moves forward within each image, never moves backwards when one image ends and the next starts, and reaches `100.0%` only once the last image has ended. It ends with `ETA <time>`, an estimate of the time the batch has left at the rate it has run so far; it reads `ETA --` until at least 1% has run. The first image's model loading (and, on first use, its download) makes the early estimate high, and it settles as the batch goes on.
- **The list** shows one row per image started so far, scrolling to the most recent five. Each row shows the image's own progress, right-aligned in three columns (`[  0%]`, `[ 42%]`, `[100%]`). It covers face detection and every enhancement, never decreases, and stops at `99%` until the file is written. A row reads as follows, its words padded with spaces so every path starts in the same column:
  - `🖼️ [  0%] Preparing to enhance <path>` before the image's run has reported anything, and for the whole time an image with nothing left to run is processed (an upscale skipped because the image is already large enough, or an image autopilot found nothing to do for);
  - `🖼️ [  0%] Choosing enhancements for <path>` with [`--autopilot`](#autopilot), while the image is analysed. On the first run, this includes downloading the face detection model, so it can stay at `0%` for a while;
  - `🖼️ [ 12%] Downloading <enhancement> for <path>` while a model is downloaded;
  - `🖼️ [ 42%] Applying <enhancement> to <path>` while an enhancement runs or is served from the [run cache](#run-cache);
  - `🖼️ [100%] Enhancements applied to <path>` once its result is written;
  - `🧨 [ 42%] Failed to enhance <path>` once it has failed;
  - `🧨 [ 42%] Stopped enhancing <path>` when you [stopped the run](#stopping-a-run) while it was being worked on.

  `<enhancement>` is the enhancement's name as the GUI shows it: `Denoise`, `Face Recovery`, `Colorize`, `Light Adjust`, `Color Balance`, `Sharpen` or `Upscale`.
- **The summary** starts with a success line when at least one image was written. For exactly one image it names the file written, so a numbered name is visible: `✨ Image enhanced successfully; file saved at out_1.png`. For more, it names the output directory: `✨ 7 images enhanced successfully; files saved under out`. Then a `🧨` line gives the reason for each image that failed, in the order they ran, since a failure can have scrolled out of the list. Last comes the run's closing `🧨` line, if it didn't succeed (see [Exit codes](#exit-codes)).

In color, the dependency's name and its percentage are bold white, the spinner magenta and the `✓` green. In the processor line, the processor is green and the quality yellow. The bar is drawn in a color gradient. In a `🖼️` row, the percentage (with its `%`) is blue, the enhancement's name (or the word `Enhancements`) magenta and the path cyan and underlined; a `🧨` row is red throughout. The success line is bold yellow.

With a single image, the view is three rows tall: the bar, a blank row and the image's row.

When stderr is **not** a terminal (for example, when redirected to a file or run in CI), there's no dependency line, no view and no progress bar. The processor line comes first, followed by a blank line. Then each image prints one line, without colors, when it ends, with the words its row ends with, its percentage included; a failed image's line adds the reason. Then come the summary's success line and the closing line:

```
🧠 Running inference using the processor Auto; quality HD

🖼️ [100%] Enhancements applied to       photos/a.jpg
🧨 [ 42%] Failed to enhance             photos/b.jpg: cannot write out/b.jpg: …
🖼️ [100%] Enhancements applied to       photos/c.jpg
✨ 2 images enhanced successfully; files saved under out
🧨 1 of 3 images failed
```

A terminal that can't draw the live view gets these same lines, in color.

**Colors** are turned off when the [`NO_COLOR`](https://no-color.org) environment variable is set to any non-empty value:

```bash
NO_COLOR=1 opai photo.jpg --up -o out.png
```

The terminal shows only the basic progress and the result. More detail, such as each file written, a skipped upscale, a skipped unreadable file, how many faces were found, the hardware used and full error messages, goes to the [log file](#log-file).

## Stopping a run

Press **Ctrl-C** at any time to stop.

- **While enhancing:** the run stops at the next safe point. Nothing is written for the image being enhanced, and its row reads `🧨 [ 42%] Stopped enhancing <path>`, keeping the percentage it had reached. No image after it is started. Files already written for earlier images are **kept**, and the summary says how many. The reasons of any images that failed before the stop are listed, then the line `🧨 stopped: nothing more was written` is printed. Exit code: `130`, even if images failed before the stop.
- **While downloading dependencies:** the download stops. Next time, it resumes from where it left off instead of starting over. Nothing is written. Exit code: `130`.

A file that the stopped image would have replaced with `--overwrite`, including its own source in place, is left exactly as it was, with no hidden file beside it.

## Exit codes

| Code  | Meaning                                                                                                     |
|-------|-------------------------------------------------------------------------------------------------------------|
| `0`   | Success. Every image was written.                                                                          |
| `1`   | The run failed: a setup error (including another instance running), or **any** image that failed to decode, enhance or be written. The other images in the batch were still processed. |
| `2`   | Usage error: bad arguments, an unreadable or unsupported named input, an output that can't be used as asked, no image to enhance, or a [processor](#-p---processor-processor-default-auto) this machine doesn't offer. Nothing was written. |
| `130` | Stopped with Ctrl-C. Results already written for earlier images are kept.                                  |

`--help` and `--version` exit with `0`.

A run that fails ends with one `🧨` line. When images failed in a run of several, it says how many: `🧨 1 of 8 images failed`. When the run has a single image, that image's own reason is the last line, for example `🧨 could not enhance photo.jpg: …`, and no count follows it.

## Log file

Each run writes detailed records to a log file shared by the CLI and the GUI:

| OS      | Path                                                              |
|---------|-------------------------------------------------------------------|
| macOS   | `~/Library/Application Support/io.vinicius.opai/logs/opai.log`    |
| Linux   | `~/.config/io.vinicius.opai/logs/opai.log`                        |
| Windows | `%APPDATA%\io.vinicius.opai\logs\opai.log`                        |

What's in it that the terminal doesn't show:

- An upscale that was skipped because the image was already large enough.
- With [`--autopilot`](#autopilot), the enhancements chosen for each image, and a warning when part of the analysis (such as face detection) couldn't run.
- A file found in a directory that was skipped because it couldn't be read, and why.
- Face detection results, e.g. `found 3 faces, restoring 2`, or `no faces were found; none will be restored`.
- Each file written, with its size and format, and the hardware it ran on, e.g. `ran on CoreML, CPU` or `served from cache`.
- The full error behind any `🧨` line.

**Please attach this file when [reporting a bug](https://github.com/vegidio/open-photo-ai/issues).**

## Common errors

| What you see                                                                 | Cause and fix |
|-------------------------------------------------------------------------------|---------------|
| `error: at least one enhancement is required: --denoise (--dn), …`             | No enhancement flag was given. Add at least one, or `--ap`. |
| `error: the argument '--autopilot' cannot be used with '--upscale [<MODEL>]'`   | `--ap` chooses the enhancements itself. Drop `--ap`, or drop the enhancement flags and their options. |
| `error: invalid value 'paris' for the environment variable OPAI_AP_UPSCALE` followed by `[possible values: …]` | An `OPAI_AP_*` variable names a model of another enhancement, or a misspelt one. Pick one from the list shown, `off`, or unset it. |
| `error: the following required arguments were not provided: --output <OUTPUT>` | `-o`/`--output` is missing. |
| `error: invalid value 'foo' for '--upscale [<MODEL>]'` followed by `[possible values: …]` | Unknown model name. Pick one from the list shown. |
| `error: invalid value 'photo.jpg' for '--upscale [<MODEL>]'`                   | An input was placed after a flag. Put the `INPUT`s first. |
| `error: invalid value '9' for '--scale <FACTOR>': the scale must be from 1.0 to 8.0` | `--scale` out of range or not a number. |
| `error: invalid value '0.5' for '--la-bias <BIAS>': the bias must be a whole number from -100 to 100` | Strength/bias must be a whole number within range. |
| `error: the following required arguments were not provided: --denoise [<MODEL>]` | A strength/bias/scale option was used without its enhancement flag (e.g. `--dn-str` without `--dn`, or `--scale` without `--up`). |
| `error: invalid value 'raw' for '--format <FORMAT>': "raw" is not a format; use preserve, …` | Unknown `--format`. Pick one from the list shown. |
| `error: invalid value 'cuda' for '--processor <PROCESSOR>'` followed by `[possible values: …]` | That processor doesn't exist for your operating system (`coreml` exists only on macOS; `cuda` and `tensorrt` never on macOS). Pick one from the list shown. |
| `🧨 cannot read missing.jpg: No such file or directory …`                      | A named input doesn't exist or can't be read. If it's a wildcard, it matched nothing. |
| `🧨 the codec refused notes.txt: could not determine an image format …`        | A named file isn't an image `opai` can read, or its extension isn't a supported format. |
| `🧨 cannot list photos: …`                                                     | Part of a directory input's tree can't be read. Fix its permissions, or leave it out. |
| `🧨 no image to enhance in photos`                                             | The inputs hold no image `opai` can read. Add `-r` if the images are in subfolders. |
| `🧨 an output file takes exactly one input file; …`                            | `-o` names a file, but there are several inputs or a directory input. Give a directory instead. |
| `🧨 --suffix needs an output directory; …`                                     | `--format`, `--prefix` and `--suffix` only work when `-o` is a directory. |
| `🧨 cannot write into out: it is a file, not a directory, …`                   | `-o` would be a directory, but a file already exists there. |
| `🧨 cannot write out/a.png: …`                                                 | The result couldn't be written there: the directory is missing or not writable, or every numbered name up to `_999` is taken. |
| `🧨 Open Photo AI is already running …; close it and try again`                | Another `opai` process or the GUI is running. Close it and try again. |
| `🧨 the processor … is not available on this machine; use …`                   | This machine doesn't offer the processor given with `-p` (for example, no supported NVIDIA GPU for `cuda`). Use one of the processors listed, or leave `-p` out. Exit code `2`. |
| `🧨 [ 42%] Failed to enhance photos/b.jpg`                                      | That image failed; the rest of the batch went on. Its reason is listed under the view when the run ends. |
| `🧨 could not enhance photos/b.jpg: …`                                         | That image failed; the rest of the batch went on. The log file has the full error. |
| `🧨 failed to decode photos/b.jpg: …`                                          | That image's header was readable, but the image itself wasn't; the rest of the batch went on. |
| `🧨 2 of 5 images failed`                                                      | The summary after a batch with failures. Exit code `1`. |
| `🧨 stopped: nothing more was written`                                         | You pressed Ctrl-C. |
