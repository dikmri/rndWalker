# rndWalker

<p align="center">
<img src="assets/icons/rndwalker-128.png" alt="rndWalker icon" width="128" height="128">
</p>

`rndWalker` is a Rust + eframe desktop app that randomly plays videos and music in local folders.

The old Electron version has been saved to `legacy/electron/`, and future development will proceed based on the Rust version.

## Main features

- Random play MP4 videos
- MP4 folder management for up to 3 groups
- Up to 4 folders can be registered in each MP4 group
- Independent random playback of MP3 audio
- Folder preset, volume, mute, settings save
- Automatic update check at startup using GitHub Releases
- Automatic release for Windows, macOS, and Linux

## install

Automatically detects the latest version of GitHub Release and installs it in the user area.

### Windows

Run the following in PowerShell:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.ps1 | iex"
```

The default installation location is `%LOCALAPPDATA%\Programs\rndWalker`. Create a start menu shortcut and add the installation location to the user's PATH.

### macOS/Linux

Run the following in your terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.sh | sh
```

The default installation location is `~/.local/share/rndWalker`. Create a launch link in `~/.local/bin/rndWalker`. macOS automatically detects Intel / Apple Silicon, and Linux supports x86_64.

macOS automatically checks for FFmpeg if Homebrew is available. On Linux, install the FFmpeg and SDL2 runtimes if `apt-get`, `dnf`, or `pacman` are available. On Linux, you may need to enter your `sudo` password. If you want to avoid automatic deployment of dependencies, run:

```sh
curl -fsSL https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.sh | env RNDWALKER_SKIP_DEPS=1 sh
```

## keyboard operation

| key | operation |
| --- | --- |
| F5 | Reload video folder |
| F11 | Full screen switching |
| Esc | Cancel full screen or close settings screen |
| M | Mute switch |
| W/S | Volume up / Volume down |
| D/A | Next random video / Previous video |
| left / top / right | Switch to folder group 1 / 2 / 3 after the current video ends |
| under | Return to all folders after the current video ends |

## build

Since `egui-video` uses FFmpeg, the development environment requires the FFmpeg 7 development library and `pkg-config`.

```powershell
cargo build --release
```

The settings are saved as `rndWalker/settings.json` in the OS standard settings directory.

## Releases and updates

Pushing a tag like `v0.1.12` will run `.github/workflows/release.yml` and upload the ZIP for the supported platforms to GitHub Releases.

For the release version, check GitHub Releases of `dikmri/rndWalker` when starting. If there is an update, it will replace the executable file and prompt you to restart once it is complete.

## multilingual documentation

Original copies of README and release notes will be managed in Japanese. When the Japanese documentation is updated in the `main` branch, `.github/workflows/docs-i18n.yml` generates the translation and reflects it in `docs/i18n/`.

- English: `docs/i18n/README.en.md`
- Chinese: `docs/i18n/README.zh-CN.md`
- Korean: `docs/i18n/README.ko.md`

Since the translation is automatically generated, please reflect any corrections that require accuracy to the original Japanese version.
