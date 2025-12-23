# Open Photo AI (OPAI)

<p align="center">
<img src="docs/assets/icon.avif" width="300" alt="Open Photo AI (OPAI)"/>
<br/>
<strong>Open Photo AI</strong> is an open source alternative for the popular photo AI editor.
<br/>
It currently supports the following enhancements:
<br/><br/>
<img src="https://img.shields.io/badge/Face Recovery-F9BE5A?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIGhlaWdodD0iMjRweCIgdmlld0JveD0iMCAtOTYwIDk2MCA5NjAiIHdpZHRoPSIyNHB4IiBmaWxsPSIjMDAwMDAwIj4KICAgIDxwYXRoIGQ9Ik0zNjAtMzkwcS0yMSAwLTM1LjUtMTQuNVQzMTAtNDQwcTAtMjEgMTQuNS0zNS41VDM2MC00OTBxMjEgMCAzNS41IDE0LjVUNDEwLTQ0MHEwIDIxLTE0LjUgMzUuNVQzNjAtMzkwWm0yNDAgMHEtMjEgMC0zNS41LTE0LjVUNTUwLTQ0MHEwLTIxIDE0LjUtMzUuNVQ2MDAtNDkwcTIxIDAgMzUuNSAxNC41VDY1MC00NDBxMCAyMS0xNC41IDM1LjVUNjAwLTM5MFpNNDgwLTE2MHExMzQgMCAyMjctOTN0OTMtMjI3cTAtMjQtMy00Ni41VDc4Ni01NzBxLTIxIDUtNDIgNy41dC00NCAyLjVxLTkxIDAtMTcyLTM5VDM5MC03MDhxLTMyIDc4LTkxLjUgMTM1LjVUMTYwLTQ4NnY2cTAgMTM0IDkzIDIyN3QyMjcgOTNabTAgODBxLTgzIDAtMTU2LTMxLjVUMTk3LTE5N3EtNTQtNTQtODUuNS0xMjdUODAtNDgwcTAtODMgMzEuNS0xNTZUMTk3LTc2M3E1NC01NCAxMjctODUuNVQ0ODAtODgwcTgzIDAgMTU2IDMxLjVUNzYzLTc2M3E1NCA1NCA4NS41IDEyN1Q4ODAtNDgwcTAgODMtMzEuNSAxNTZUNzYzLTE5N3EtNTQgNTQtMTI3IDg1LjVUNDgwLTgwWm0tNTQtNzE1cTQyIDcwIDExNCAxMTIuNVQ3MDAtNjQwcTE0IDAgMjctMS41dDI3LTMuNXEtNDItNzAtMTE0LTExMi41VDQ4MC04MDBxLTE0IDAtMjcgMS41dC0yNyAzLjVaTTE3Ny01ODFxNTEtMjkgODktNzV0NTctMTAzcS01MSAyOS04OSA3NXQtNTcgMTAzWm0yNDktMjE0Wm0tMTAzIDM2WiIvPgo8L3N2Zz4="/>
<img src="https://img.shields.io/badge/Upscale-984E7D?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIGhlaWdodD0iMjRweCIgdmlld0JveD0iMCAtOTYwIDk2MCA5NjAiIHdpZHRoPSIyNHB4IiBmaWxsPSIjZTNlM2UzIj48cGF0aCBkPSJNMTIwLTEyMHYtMzIwaDgwdjE4NGw1MDQtNTA0SDUyMHYtODBoMzIwdjMyMGgtODB2LTE4NEwyNTYtMjAwaDE4NHY4MEgxMjBaIi8+PC9zdmc+"/>
</p>

## 💡 Motivation

There are many excellent AI-based photo editing tools available today, ranging from open-source solutions – often powerful but complex to set up and use, such as ComfyUI – to commercial products that favor ease of use over deep customization, like those from Topaz Labs.

I have long used both ComfyUI and Topaz Labs solutions, choosing between them depending on the task. Recently, however, Topaz Labs moved from a perpetual license to a subscription-based pricing model, a change I strongly dislike. As a developer, I am happy to pay for software that is useful for me, whether open source or proprietary, but I believe subscription models are rarely designed to benefit users and instead primarily serve company interests.

That is why I created this project: an open-source alternative to Topaz Photo AI. It may never match the same level of polish or performance – Topaz has teams of full-time engineers, while this is a solo project built in my spare time – but I have ambitious goals and aim to reach feature parity with their product over time.

## 🖼️ Usage

There are two ways to use **Open Photo AI**: using the GUI or the CLI.

The GUI is the easiest way to use the app, with an intuitive interface that allows you to enhance images with just a few clicks. The CLI is more advanced and allows you to enhance images in a more automated way.

Both versions are available for Windows, macOS, and Linux. Download the [latest release](https://github.com/vegidio/open-photo-ai/releases/) that matches your computer architecture and operating system and follow the instructions below:

### GUI ([video](https://www.youtube.com/watch?v=NdSfeyiXPf8) 🎥)

<p align="center">
<img src="docs/assets/gui-screenshot.avif" width="80%" alt="Open Photo AI - GUI"/>
</p>

1. Click on the button `Browse images` to select one or more images that you would like to enhance.
2. The images are enhanced automatically or manually depending on the toggle `Autopilot` in the top right side of the screen:
   1. If enabled, the app will automatically analyse the images and suggest enhancements for them.
   2. If disabled, you will need to select the enhancements yourself, using on the button `Add enhancement`.
3. Select one or more images that you would like to export on the image drawer at the bottom of the screen.
4. Click on the button `Export image`, select the location and image format, then click on `Export`.

### CLI

Coming soon...

## ✨ Enhancements

All enhancements available here come from open-source AI models that were adapted and converted to work on this project. The models and the credits to the original works can be found in the Hugging Face repository [vegidio/open-photo-ai](https://huggingface.co/vegidio/open-photo-ai):

### Face Recovery

- **Athens**: use when you need to restore a face while preserving the original identity. It reconstructs details conservatively, avoids inventing features, and works well on heavily degraded images. This makes it suitable for real people and any case where facial accuracy matters more than visual polish.
- **Santorini**: use when your priority is maximum visual realism and sharpness. It works well for moderately degraded faces and produces very clean, attractive results, but it can hallucinate features and drift from the original identity. It is better suited for creative, entertainment, or non-critical use cases where realism matters more than strict accuracy.

*Verdict*: if identity matters, start with **Athens**; if aesthetics matter more, use **Santorini**.

### Upscale

- **Tokyo**: use when you want accurate, conservative upscaling that stays close to the original image. It focuses on structure and texture consistency, avoids inventing details, and works well for clean images, illustrations, screenshots, and content where correctness matters more than sharpness. It is a good choice when artifacts or hallucinated details would be unacceptable.
- **Kyoto**: use when you want visually strong, sharp upscaling, even if new details are introduced. It aggressively enhances textures and edges, can add perceived detail that was not present before, and performs especially well on photos and noisy or compressed images. It is best suited for creative or perceptual use cases where impact matters more than strict fidelity.

*Verdict*: start with **Tokyo**, then try **Kyoto** if the result looks too soft.

## 🛣️ Roadmap

These are the features I plan to implement in the future, in no particular order:

- [ ] Model selection and enhancements customization.
- [ ] Crop and rotate images in the GUI.
- [ ] Support different preview layouts.
- [ ] Add new models for denoise, sharpening, light, and color correction.
- [ ] Simplify the app installation using packages and installers.
- [ ] Add app preferences so you don't have to configure them every time.
- [ ] Enable TensorRT acceleration when pre warm-up is implemented.
- [ ] Attempt to include diffusion-based models (this will be hard!)
- [ ] CLI implementation.
- [ ] Improve documentation for the library.
- [ ] Internationalization to other languages.

## 💬 FAQ

### "App Is Damaged/Blocked..." (Windows & macOS only)

For a couple of years now, Microsoft and Apple have required developers to join their "Developer Program" to gain the pretentious status of an _identified developer_ 😛.

Translating to non-BS language, this means that if you’re not registered with them (i.e., paying the fee), you can’t freely distribute Windows or macOS software. Apps from unidentified developers will display a message saying the app is damaged or blocked and can’t be opened.

To bypass this, open the Terminal and run one of the commands below (depending on your operating system), replacing `<path-to-app>` with the correct path to where you’ve installed the app:

- Windows: `Unblock-File -Path <path-to-app>`
- macOS: `xattr -d com.apple.quarantine <path-to-app>`

### "Error loading libraries: libwebkit2gtk-4.1.so..." (Linux only)

To run the GUI version of the app on Linux, you will need to install the following dependencies: `libgtk` and `libwebkit2gtk`. To do that, open your terminal and run the following command, depending on your distribution:

- Debian/Ubuntu: `sudo apt install libgtk-3-0 libwebkit2gtk-4.1-0`
- Fedora 40+: `sudo dnf install gtk3 webkit2gtk4.1`
- Arch Linux: `sudo pacman -S gtk3 webkit2gtk-4.1`
- openSUSE: `sudo zypper install libgtk-3-0 libwebkit2gtk-4_1-0`

## 🐞 Known Issues

1. Using half-precision (FP16) models with CPU execution provider often doesn't give any performance boost; a bug fix for this is expected to be available in the next ONNX release.
2. The ONNX Runtime has a [bug](https://github.com/microsoft/onnxruntime/pull/26443) when running half-precision (FP16) models on Apple's M-series chip; a bug fix for this is expected to be available in the next ONNX release. Meanwhile, all image processing on Macs will be done in full precision, which gives the best quality possible, but it's often unnecessarily slow.
3. The **Tokyo** model doesn't work with Apple's CoreML. This is a limitation on CoreML's architecture, so any upscaling using this model on a Mac will be slow.

## 🛠️ Build

### Dependencies

To build this project, you will need the following dependencies installed in your computer:

-   [Golang](https://go.dev/doc/install)
-   [Task](https://taskfile.dev/installation/)

If you want to build the GUI you will also need:

-   [Node.js](https://nodejs.org/en/download/)
-   [PNPM](https://pnpm.io/installation)
-   [Wails 3+](https://v3alpha.wails.io/getting-started/installation)

### Compiling

With all the dependencies installed, in the project's root folder run the command:

```bash
$ task <interface> arch=<architecture>
```

Where:

- `<interface>`: can be `cli` or `gui`.
- `<architecture>`: can be `amd64` or `arm64`.

For example, if I wanted to build a GUI version of the app, on architecture AMD64, I would run the command:

```bash
$ task gui arch=amd64
```

## 📝 License

**Open Photo AI** is released under the AGPL-3.0 License. See [LICENSE](LICENSE) for details.

## 👨🏾‍💻 Author

Vinicius Egidio ([vinicius.io](http://vinicius.io))
