# Open Photo AI (OPAI)

<p align="center">
<img src="docs/assets/icon.avif" width="300" alt="Open Photo AI (OPAI)"/>
<br/>
<strong>Open Photo AI</strong> is an open source alternative to the popular photo AI editor.
<br/>
It currently supports the following enhancements:
<br/><br/>
<img src="https://img.shields.io/badge/Denoise-FDEBD3?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIGhlaWdodD0iMjRweCIgdmlld0JveD0iMCAtOTYwIDk2MCA5NjAiIHdpZHRoPSIyNHB4IiBmaWxsPSIjMDAwMDAwIj48cGF0aCBkPSJNMTA2LTM4NnEtNi02LTYtMTR0Ni0xNHE2LTYgMTQtNnQxNCA2cTYgNiA2IDE0dC02IDE0cS02IDYtMTQgNnQtMTQtNlptMC0xNjBxLTYtNi02LTE0dDYtMTRxNi02IDE0LTZ0MTQgNnE2IDYgNiAxNHQtNiAxNHEtNiA2LTE0IDZ0LTE0LTZabTEwNS41IDMzNC41UTIwMC0yMjMgMjAwLTI0MHQxMS41LTI4LjVRMjIzLTI4MCAyNDAtMjgwdDI4LjUgMTEuNVEyODAtMjU3IDI4MC0yNDB0LTExLjUgMjguNVEyNTctMjAwIDI0MC0yMDB0LTI4LjUtMTEuNVptMC0xNjBRMjAwLTM4MyAyMDAtNDAwdDExLjUtMjguNVEyMjMtNDQwIDI0MC00NDB0MjguNSAxMS41UTI4MC00MTcgMjgwLTQwMHQtMTEuNSAyOC41UTI1Ny0zNjAgMjQwLTM2MHQtMjguNS0xMS41Wm0wLTE2MFEyMDAtNTQzIDIwMC01NjB0MTEuNS0yOC41UTIyMy02MDAgMjQwLTYwMHQyOC41IDExLjVRMjgwLTU3NyAyODAtNTYwdC0xMS41IDI4LjVRMjU3LTUyMCAyNDAtNTIwdC0yOC41LTExLjVabTAtMTYwUTIwMC03MDMgMjAwLTcyMHQxMS41LTI4LjVRMjIzLTc2MCAyNDAtNzYwdDI4LjUgMTEuNVEyODAtNzM3IDI4MC03MjB0LTExLjUgMjguNVEyNTctNjgwIDI0MC02ODB0LTI4LjUtMTEuNVptMTQ2IDMzNFEzNDAtMzc1IDM0MC00MDB0MTcuNS00Mi41UTM3NS00NjAgNDAwLTQ2MHQ0Mi41IDE3LjVRNDYwLTQyNSA0NjAtNDAwdC0xNy41IDQyLjVRNDI1LTM0MCA0MDAtMzQwdC00Mi41LTE3LjVabTAtMTYwUTM0MC01MzUgMzQwLTU2MHQxNy41LTQyLjVRMzc1LTYyMCA0MDAtNjIwdDQyLjUgMTcuNVE0NjAtNTg1IDQ2MC01NjB0LTE3LjUgNDIuNVE0MjUtNTAwIDQwMC01MDB0LTQyLjUtMTcuNVptMTQgMzA2UTM2MC0yMjMgMzYwLTI0MHQxMS41LTI4LjVRMzgzLTI4MCA0MDAtMjgwdDI4LjUgMTEuNVE0NDAtMjU3IDQ0MC0yNDB0LTExLjUgMjguNVE0MTctMjAwIDQwMC0yMDB0LTI4LjUtMTEuNVptMC00ODBRMzYwLTcwMyAzNjAtNzIwdDExLjUtMjguNVEzODMtNzYwIDQwMC03NjB0MjguNSAxMS41UTQ0MC03MzcgNDQwLTcyMHQtMTEuNSAyOC41UTQxNy02ODAgNDAwLTY4MHQtMjguNS0xMS41Wk0zODYtMTA2cS02LTYtNi0xNHQ2LTE0cTYtNiAxNC02dDE0IDZxNiA2IDYgMTR0LTYgMTRxLTYgNi0xNCA2dC0xNC02Wm0wLTcyMHEtNi02LTYtMTR0Ni0xNHE2LTYgMTQtNnQxNCA2cTYgNiA2IDE0dC02IDE0cS02IDYtMTQgNnQtMTQtNlptMTMxLjUgNDY4LjVRNTAwLTM3NSA1MDAtNDAwdDE3LjUtNDIuNVE1MzUtNDYwIDU2MC00NjB0NDIuNSAxNy41UTYyMC00MjUgNjIwLTQwMHQtMTcuNSA0Mi41UTU4NS0zNDAgNTYwLTM0MHQtNDIuNS0xNy41Wm0wLTE2MFE1MDAtNTM1IDUwMC01NjB0MTcuNS00Mi41UTUzNS02MjAgNTYwLTYyMHQ0Mi41IDE3LjVRNjIwLTU4NSA2MjAtNTYwdC0xNy41IDQyLjVRNTg1LTUwMCA1NjAtNTAwdC00Mi41LTE3LjVabTE0IDMwNlE1MjAtMjIzIDUyMC0yNDB0MTEuNS0yOC41UTU0My0yODAgNTYwLTI4MHQyOC41IDExLjVRNjAwLTI1NyA2MDAtMjQwdC0xMS41IDI4LjVRNTc3LTIwMCA1NjAtMjAwdC0yOC41LTExLjVabTAtNDgwUTUyMC03MDMgNTIwLTcyMHQxMS41LTI4LjVRNTQzLTc2MCA1NjAtNzYwdDI4LjUgMTEuNVE2MDAtNzM3IDYwMC03MjB0LTExLjUgMjguNVE1NzctNjgwIDU2MC02ODB0LTI4LjUtMTEuNVpNNTQ2LTEwNnEtNi02LTYtMTR0Ni0xNHE2LTYgMTQtNnQxNCA2cTYgNiA2IDE0dC02IDE0cS02IDYtMTQgNnQtMTQtNlptMC03MjBxLTYtNi02LTE0dDYtMTRxNi02IDE0LTZ0MTQgNnE2IDYgNiAxNHQtNiAxNHEtNiA2LTE0IDZ0LTE0LTZabTE0NS41IDYxNC41UTY4MC0yMjMgNjgwLTI0MHQxMS41LTI4LjVRNzAzLTI4MCA3MjAtMjgwdDI4LjUgMTEuNVE3NjAtMjU3IDc2MC0yNDB0LTExLjUgMjguNVE3MzctMjAwIDcyMC0yMDB0LTI4LjUtMTEuNVptMC0xNjBRNjgwLTM4MyA2ODAtNDAwdDExLjUtMjguNVE3MDMtNDQwIDcyMC00NDB0MjguNSAxMS41UTc2MC00MTcgNzYwLTQwMHQtMTEuNSAyOC41UTczNy0zNjAgNzIwLTM2MHQtMjguNS0xMS41Wm0wLTE2MFE2ODAtNTQzIDY4MC01NjB0MTEuNS0yOC41UTcwMy02MDAgNzIwLTYwMHQyOC41IDExLjVRNzYwLTU3NyA3NjAtNTYwdC0xMS41IDI4LjVRNzM3LTUyMCA3MjAtNTIwdC0yOC41LTExLjVabTAtMTYwUTY4MC03MDMgNjgwLTcyMHQxMS41LTI4LjVRNzAzLTc2MCA3MjAtNzYwdDI4LjUgMTEuNVE3NjAtNzM3IDc2MC03MjB0LTExLjUgMjguNVE3MzctNjgwIDcyMC02ODB0LTI4LjUtMTEuNVpNODI2LTM4NnEtNi02LTYtMTR0Ni0xNHE2LTYgMTQtNnQxNCA2cTYgNiA2IDE0dC02IDE0cS02IDYtMTQgNnQtMTQtNlptMC0xNjBxLTYtNi02LTE0dDYtMTRxNi02IDE0LTZ0MTQgNnE2IDYgNiAxNHQtNiAxNHEtNiA2LTE0IDZ0LTE0LTZaIi8+PC9zdmc+"/>
<img src="https://img.shields.io/badge/Face Recovery-F9BE5A?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIGhlaWdodD0iMjRweCIgdmlld0JveD0iMCAtOTYwIDk2MCA5NjAiIHdpZHRoPSIyNHB4IiBmaWxsPSIjMDAwMDAwIj4KICAgIDxwYXRoIGQ9Ik0zNjAtMzkwcS0yMSAwLTM1LjUtMTQuNVQzMTAtNDQwcTAtMjEgMTQuNS0zNS41VDM2MC00OTBxMjEgMCAzNS41IDE0LjVUNDEwLTQ0MHEwIDIxLTE0LjUgMzUuNVQzNjAtMzkwWm0yNDAgMHEtMjEgMC0zNS41LTE0LjVUNTUwLTQ0MHEwLTIxIDE0LjUtMzUuNVQ2MDAtNDkwcTIxIDAgMzUuNSAxNC41VDY1MC00NDBxMCAyMS0xNC41IDM1LjVUNjAwLTM5MFpNNDgwLTE2MHExMzQgMCAyMjctOTN0OTMtMjI3cTAtMjQtMy00Ni41VDc4Ni01NzBxLTIxIDUtNDIgNy41dC00NCAyLjVxLTkxIDAtMTcyLTM5VDM5MC03MDhxLTMyIDc4LTkxLjUgMTM1LjVUMTYwLTQ4NnY2cTAgMTM0IDkzIDIyN3QyMjcgOTNabTAgODBxLTgzIDAtMTU2LTMxLjVUMTk3LTE5N3EtNTQtNTQtODUuNS0xMjdUODAtNDgwcTAtODMgMzEuNS0xNTZUMTk3LTc2M3E1NC01NCAxMjctODUuNVQ0ODAtODgwcTgzIDAgMTU2IDMxLjVUNzYzLTc2M3E1NCA1NCA4NS41IDEyN1Q4ODAtNDgwcTAgODMtMzEuNSAxNTZUNzYzLTE5N3EtNTQgNTQtMTI3IDg1LjVUNDgwLTgwWm0tNTQtNzE1cTQyIDcwIDExNCAxMTIuNVQ3MDAtNjQwcTE0IDAgMjctMS41dDI3LTMuNXEtNDItNzAtMTE0LTExMi41VDQ4MC04MDBxLTE0IDAtMjcgMS41dC0yNyAzLjVaTTE3Ny01ODFxNTEtMjkgODktNzV0NTctMTAzcS01MSAyOS04OSA3NXQtNTcgMTAzWm0yNDktMjE0Wm0tMTAzIDM2WiIvPgo8L3N2Zz4="/>
<img src="https://img.shields.io/badge/Light Adjustment-53C0E0?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIGhlaWdodD0iMjRweCIgdmlld0JveD0iMCAtOTYwIDk2MCA5NjAiIHdpZHRoPSIyNHB4IiBmaWxsPSIjZTNlM2UzIj48cGF0aCBkPSJNNDgwLTM2MHE1MCAwIDg1LTM1dDM1LTg1cTAtNTAtMzUtODV0LTg1LTM1cS01MCAwLTg1IDM1dC0zNSA4NXEwIDUwIDM1IDg1dDg1IDM1Wm0wIDgwcS04MyAwLTE0MS41LTU4LjVUMjgwLTQ4MHEwLTgzIDU4LjUtMTQxLjVUNDgwLTY4MHE4MyAwIDE0MS41IDU4LjVUNjgwLTQ4MHEwIDgzLTU4LjUgMTQxLjVUNDgwLTI4MFpNMjAwLTQ0MEg0MHYtODBoMTYwdjgwWm03MjAgMEg3NjB2LTgwaDE2MHY4MFpNNDQwLTc2MHYtMTYwaDgwdjE2MGgtODBabTAgNzIwdi0xNjBoODB2MTYwaC04MFpNMjU2LTY1MGwtMTAxLTk3IDU3LTU5IDk2IDEwMC01MiA1NlptNDkyIDQ5Ni05Ny0xMDEgNTMtNTUgMTAxIDk3LTU3IDU5Wm0tOTgtNTUwIDk3LTEwMSA1OSA1Ny0xMDAgOTYtNTYtNTJaTTE1NC0yMTJsMTAxLTk3IDU1IDUzLTk3IDEwMS01OS01N1ptMzI2LTI2OFoiLz48L3N2Zz4="/>
<br/>
<img src="https://img.shields.io/badge/Color Balance-C3E88D?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIGhlaWdodD0iMjRweCIgdmlld0JveD0iMCAtOTYwIDk2MCA5NjAiIHdpZHRoPSIyNHB4IiBmaWxsPSIjMDAwMDAwIj48cGF0aCBkPSJNNDgwLTgwcS04MiAwLTE1NS0zMS41dC0xMjcuNS04NlExNDMtMjUyIDExMS41LTMyNVQ4MC00ODBxMC04MyAzMi41LTE1NnQ4OC0xMjdRMjU2LTgxNyAzMzAtODQ4LjVUNDg4LTg4MHE4MCAwIDE1MSAyNy41dDEyNC41IDc2cTUzLjUgNDguNSA4NSAxMTVUODgwLTUxOHEwIDExNS03MCAxNzYuNVQ2NDAtMjgwaC03NHEtOSAwLTEyLjUgNXQtMy41IDExcTAgMTIgMTUgMzQuNXQxNSA1MS41cTAgNTAtMjcuNSA3NFQ0ODAtODBabTAtNDAwWm0tMTc3IDIzcTE3LTE3IDE3LTQzdC0xNy00M3EtMTctMTctNDMtMTd0LTQzIDE3cS0xNyAxNy0xNyA0M3QxNyA0M3ExNyAxNyA0MyAxN3Q0My0xN1ptMTIwLTE2MHExNy0xNyAxNy00M3QtMTctNDNxLTE3LTE3LTQzLTE3dC00MyAxN3EtMTcgMTctMTcgNDN0MTcgNDNxMTcgMTcgNDMgMTd0NDMtMTdabTIwMCAwcTE3LTE3IDE3LTQzdC0xNy00M3EtMTctMTctNDMtMTd0LTQzIDE3cS0xNyAxNy0xNyA0M3QxNyA0M3ExNyAxNyA0MyAxN3Q0My0xN1ptMTIwIDE2MHExNy0xNyAxNy00M3QtMTctNDNxLTE3LTE3LTQzLTE3dC00MyAxN3EtMTcgMTctMTcgNDN0MTcgNDNxMTcgMTcgNDMgMTd0NDMtMTdaTTQ4MC0xNjBxOSAwIDE0LjUtNXQ1LjUtMTNxMC0xNC0xNS0zM3QtMTUtNTdxMC00MiAyOS02N3Q3MS0yNWg3MHE2NiAwIDExMy0zOC41VDgwMC01MThxMC0xMjEtOTIuNS0yMDEuNVQ0ODgtODAwcS0xMzYgMC0yMzIgOTN0LTk2IDIyN3EwIDEzMyA5My41IDIyNi41VDQ4MC0xNjBaIi8+PC9zdmc+"/>
<img src="https://img.shields.io/badge/Sharpen-F55951?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIGhlaWdodD0iMjRweCIgdmlld0JveD0iMCAtOTYwIDk2MCA5NjAiIHdpZHRoPSIyNHB4IiBmaWxsPSIjRkZGRkZGIj48cGF0aCBkPSJtODAtMTYwIDQwMC02NDAgNDAwIDY0MEg4MFptMTQ0LTgwaDUxMkw0ODAtNjUwIDIyNC0yNDBabTI1Ni0yMDVaIi8+PC9zdmc+"/>
<img src="https://img.shields.io/badge/Upscale-984E7D?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIGhlaWdodD0iMjRweCIgdmlld0JveD0iMCAtOTYwIDk2MCA5NjAiIHdpZHRoPSIyNHB4IiBmaWxsPSIjZTNlM2UzIj48cGF0aCBkPSJNMTIwLTEyMHYtMzIwaDgwdjE4NGw1MDQtNTA0SDUyMHYtODBoMzIwdjMyMGgtODB2LTE4NEwyNTYtMjAwaDE4NHY4MEgxMjBaIi8+PC9zdmc+"/>
</p>

## 💡 Motivation

There are many excellent AI-based photo editing tools available today, ranging from open-source solutions – often powerful but complex to set up and use, such as ComfyUI – to commercial products that favor ease of use over deep customization, like those from Topaz Labs.

I have long used both ComfyUI and Topaz Labs solutions, choosing between them depending on the task. Recently, however, Topaz Labs moved from a perpetual license to a subscription-based pricing model, a change I strongly dislike. As a developer, I am happy to pay for software that is useful for me, whether open source or proprietary, but I believe subscription models are rarely designed to benefit users and instead primarily serve company interests.

That is why I created this project: an open-source alternative to Topaz Photo AI. It may never match the same level of polish or performance – Topaz has teams of full-time engineers, while this is a solo project built in my spare time – but I have ambitious goals and aim to reach feature parity with their product over time.

## ⬇️ Installation

This app has versions for Windows, macOS, and Linux. Download the [latest release](https://github.com/vegidio/open-photo-ai/releases) that matches your computer architecture and operating system.

However, the recommended (and easiest) way to install **Open Photo AI** is using one of the following scripts; copy and paste the command below in the terminal, and the script will automatically detect and install the correct version of the app:

### macOS & Linux

```bash
curl -fsSL https://vegidio.github.io/open-photo-ai/install.sh | sh
```

### Windows (PowerShell)

```powershell
irm https://vegidio.github.io/open-photo-ai/install.ps1 | iex
```

## 🖼️ Usage

There are two ways to use **Open Photo AI**: using the GUI or the CLI.

The GUI is the easiest way to use the app, with an intuitive interface that allows you to enhance images with just a few clicks. The CLI is more advanced and allows you to enhance images in a more automated way.

### GUI ([video](https://www.youtube.com/watch?v=NdSfeyiXPf8) 🎥)

<p align="center">
<img src="docs/assets/gui-screenshot.avif" width="80%" alt="Open Photo AI - GUI"/>
</p>

1. Click on the button `Browse images` to select one or more images that you would like to enhance.
2. The images are enhanced automatically or manually depending on the toggle `Autopilot` in the top right side of the screen:
   - If enabled, the app will automatically analyse the images and suggest enhancements for them.
   - If disabled, you will need to select the enhancements yourself, using the button `Add enhancement`.
3. Select one or more images that you would like to export on the image drawer at the bottom of the screen.
4. Click on the button `Export image`, select the location and image format, then click on `Export`.

### CLI

Coming soon...

## ✨ Enhancements

All enhancements available here come from open-source AI models that were adapted and converted to work on this project. The models and the credits to the original works can be found in the Hugging Face repository [vegidio/open-photo-ai](https://huggingface.co/vegidio/open-photo-ai):

### Face Recovery

- **Athens**: use when identity fidelity matters most. This model lets you preserve facial structure while restoring details, even on heavily degraded faces. Best when you want restoration without changing the person.
- **Santorini**: use when you want aggressive, fast enhancement and can tolerate identity drift. It produces sharp, visually pleasing faces on moderate degradation, but may hallucinate features and alter identity on very low-quality inputs.

*Verdict*: if identity matters, start with **Athens**; if aesthetics matter more, use **Santorini**.

### Light Adjustment

- **Paris**: use when working with images affected by poor or uneven lighting, such as night scenes, backlit photos, shadows, or overexposed areas. It’s useful when you need to enhance visibility and contrast so that images look clearer.

### Color Balance

- **Rio**: use when your photos look too orange, too blue, or just have an off, unnatural tint, like indoor shots under warm lamps, cloudy outdoor scenes, or pictures taken in mixed lighting conditions where the colors simply don't look natural.

### Upscale

- **Tokyo**: use when you want a natural upscale without exaggeration. It focuses on preserving the original look and fine structures instead of "inventing" new details, making it ideal when realism and faithfulness matter more than sharpness.
- **Kyoto**: use for real-world photos (people, landscapes, products). It excels at restoring details while handling noise, blur, and compression artifacts. Ideal for practical applications where images are imperfect, and you want visually pleasing, robust results fast.
- **Saitama**: use for cartoon, drawings, line art, and digital illustrations. It preserves clean lines, flat colors, and stylized shading without introducing photo-like textures. Best when sharp edges and stylistic consistency matter more than realism.

*Verdict*: start with **Tokyo** if you have a powerful GPU, then try **Kyoto** it's taking too long.

### Denoise

- **Stockholm**: use when you need fast, high-quality denoising of real sensor noise and computational efficiency matters. It's a good choice when throughput and resource constraints are real concerns, keeping inference times low without sacrificing quality.
- **Gothenburg**: use when your photos contain real-world sensor noise, the kind produced by shooting in low light or at high ISO with a smartphone or DSLR. It handles complex noise patterns that cameras produce, making it the right choice for photography.
- **Malmö**: use to remove rain streaks from outdoor images, whether captured in light drizzle or heavy downpour. It handles rain of varying scale, density, and direction, restoring fine details behind streaks. A good choice when weather artifacts obscure the scene.

### Sharpen

- **Moscow**: use when blur comes from the camera being out of focus rather than from movement — e.g. portraits with a blurry background or foreground, macro photography gone soft, or any scene where a lens failed to focus on the right plane.
- **St. Petersburg**: use when you need fast, lightweight motion deblurring and efficiency matters more than squeezing out every last bit of quality. It's well-suited for action footage and handheld camera shake, and it's a solid choice when running on limited hardware.
- **Novgorod**: use when blur is caused by camera shake or fast-moving subjects — e.g. sports, handheld shots in low light, or any photo where something moved during exposure. It prioritizes maximum restoration quality over speed; good when results matter most.

## 🛣️ Roadmap

These are the features I plan to implement in the future, in no particular order:

- [x] Model selection and enhancements customization.
- [x] Support different preview layouts.
- [x] Add new model for light adjustment.
- [x] Add app preferences so you don't have to configure them every time.
- [x] Enable TensorRT acceleration when pre warm-up is implemented.
- [x] Simplify the app installation using packages and installers.
- [x] Add new model for color balance.
- [x] Add new models for denoise, sharpening.
- [x] Crop and rotate images in the GUI.
- [ ] Rework the architecture of some models to improve performance.
- [ ] Add new model to colorize black and white photos.
- [ ] Add new model to fix imperfections and remove objects from photos.
- [ ] Attempt to include diffusion-based models (this will be hard!)
- [ ] CLI implementation.
- [ ] Improve documentation for the library.
- [ ] Internationalization to other languages.

## 💣 Troubleshooting

### "App Is Damaged/Blocked..." (Windows & macOS only)

For a couple of years now, Microsoft and Apple have required developers to join their "Developer Program" to gain the pretentious status of an _identified developer_ 😛.

Translating to non-BS language, this means that if you’re not registered with them (i.e., paying the fee), you can’t freely distribute Windows or macOS software. Apps from unidentified developers will display a message saying the app is damaged or blocked and can’t be opened.

To bypass this, open the Terminal and run one of the commands below (depending on your operating system), replacing `<path-to-app>` with the correct path to where you’ve installed the app:

- Windows: `Unblock-File -Path <path-to-app>`
- macOS: `xattr -d com.apple.quarantine <path-to-app>`

### "Error loading libraries: libwebkitgtk-6.0.so..." (Linux only)

To run the GUI version of the app on Linux, you will need to install the following dependencies: `libgtk` and `libwebkitgtk`. To do that, open your terminal and run the following command, depending on your distribution:

- Debian/Ubuntu: `sudo apt install libgtk-4-1 libwebkitgtk-6.0-4`
- Fedora 40+: `sudo dnf install gtk4 webkitgtk6.0`
- Arch Linux: `sudo pacman -S gtk4 webkitgtk-6.0`
- openSUSE: `sudo zypper install libgtk-4-1 libwebkitgtk-6_0-4`

### The app is taking too long to download dependencies

This app has some important dependencies that can't be bundled with the app itself because they are rather big, like ONNX Runtime, CUDA and TensorRT (if supported by your system). They are hosted on Github and the app will download them the first time it opens.

Unfortunately, Github has a rate limit that will throttle the download speed if these files are downloaded too frequently. Since this is an open-source and free project I can't afford to pay for a hosted solution where we wouldn't have this problem.

If someday this project receives enough funds/donations then I will pay for a better hosting solution. Meanwhile, bare with me on this one.

### The model is taking too long to load (TensorRT only)

When you open the app for the first time, if it detects that TensorRT is available on your system, it will prompt you to enable it or not. If you choose to enable it, all models will run with TensorRT acceleration.

This is one of the fastest ways to run the models, however TensorRT needs to optimize the model graphs the first time it's used, which can take a few minutes. This is why it seems to be taking too long or even stuck when you run the models for the first time. But on subsequent runs, when the TensorRT optimization is already done, all enhancements will run much faster.

If you don't want to use TensorRT acceleration, you can disable it in the app Settings.

## 🐞 Known Issues

1. Using half-precision (FP16) models with CPU execution provider often doesn't give any performance boost; a bug fix for this is expected to be available in the next ONNX release.
2. The ONNX Runtime has a [bug](https://github.com/microsoft/onnxruntime/pull/26443) when running half-precision (FP16) models on Apple's M-series chip; a bug fix for this is expected to be available in the next ONNX release. Meanwhile, all image processing on Macs will be done in full precision, which gives the best quality possible, but it's often unnecessarily slow.
3. The **Tokyo** model doesn't work with Apple's CoreML. This is a limitation on CoreML's architecture, so any upscaling using this model on a Mac will be slow.

## 🐛 Error Reporting

If you encounter any issues while using the app, please report them by [creating a new issue](https://github.com/vegidio/open-photo-ai/issues) on our repository and give as much detail as possible, including steps to reproduce the issue, screenshots, and any error messages you receive.

Errors reported by e-mail or other channels will not be tracked, so please make sure to report them on Github.

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
task <interface> arch=<architecture>
```

Where:

- `<interface>`: can be `cli` or `gui`.
- `<architecture>`: can be `amd64` or `arm64`.

For example, if I wanted to build a GUI version of the app, on architecture AMD64, I would run the command:

```bash
task gui arch=amd64
```

## 📝 License

**Open Photo AI** is released under the AGPL-3.0 License. See [LICENSE](LICENSE) for details.

## 👨🏾‍💻 Author

Vinicius Egidio ([vinicius.io](http://vinicius.io))
