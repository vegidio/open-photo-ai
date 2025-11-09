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

## 🖼️ Usage

You can use **Open Photo AI** in two ways: as an app (both GUI and CLI) or as a Go library.

### GUI

<p align="center">
<img src="docs/assets/gui-screenshot.avif" width="80%" alt="Open Photo AI - GUI"/>
</p>

## 💡 Features

### Face Recovery

Coming soon...

### Upscale

Coming soon...

## 💬 FAQ

### "App Is Damaged/Blocked..." (Windows & macOS only)

For a couple of years now, Microsoft and Apple have required developers to join their "Developer Program" to gain the pretentious status of an _identified developer_ 😛.

Translating to non-BS language, this means that if you’re not registered with them (i.e., paying the fee), you can’t freely distribute Windows or macOS software. Apps from unidentified developers will display a message saying the app is damaged or blocked and can’t be opened.

To bypass this, open the Terminal and run one of the commands below (depending on your operating system), replacing `<path-to-app>` with the correct path to where you’ve installed the app:

-   Windows: `Unblock-File -Path <path-to-app>`
-   macOS: `xattr -d com.apple.quarantine <path-to-app>`

## 🛠️ Build

### Dependencies

In order to build this project you will need the following dependencies installed in your computer:

-   [Golang](https://go.dev/doc/install)
-   [Task](https://taskfile.dev/installation/)

If you want to build the GUI you will also need:

-   [Node.js](https://nodejs.org/en/download/)
-   [PNPM](https://pnpm.io/installation)
-   [Wails 3+](https://v3alpha.wails.io/getting-started/installation)

### Compiling

With all the dependencies installed, in the project's root folder run the command:

```bash
$ task <interface> os=<operating-system> arch=<architecture>
```

Where:

-   `<interface>`: can be `cli` or `gui`.
-   `<operating-system>`: can be `windows`, `darwin` (macOS), or `linux`.
-   `<architecture>`: can be `amd64` or `arm64`.

For example, if I wanted to build a GUI version of the app for Windows, on architecture AMD64, I would run the command:

```bash
$ task gui os=windows arch=amd64
```

## 📈 Telemetry

This app collects information about the data that you're downloading to help me track bugs and improve the general stability of the software.

**No identifiable information about you or your computer is tracked.** But if you still want to stop the telemetry, you can do that by adding the flag `--no-telemetry` in the CLI tool.

## 📝 License

**Open Photo AI** is released under the AGPL-3.0 License. See [LICENSE](LICENSE) for details.

## 👨🏾‍💻 Author

Vinicius Egidio ([vinicius.io](http://vinicius.io))
