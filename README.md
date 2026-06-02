# rndWalker

`rndWalker` is a Rust + eframe port of the original Electron random video/audio player.

## Features

- Random MP4 playback from up to 3 folder groups.
- Each MP4 group supports up to 4 folders.
- Independent random MP3 playback.
- Folder presets, local JSON settings, volume and mute controls.
- Startup self-update from GitHub Releases in release builds.
- Cross-platform release workflow for Windows, macOS, and Linux.

## Keyboard

| Key | Action |
| --- | --- |
| F5 | Reload video folders |
| F11 | Toggle fullscreen |
| Esc | Exit fullscreen or close settings |
| M | Toggle mute |
| W / S | Volume up / down |
| D / A | Next random video / previous video |
| Left / Up / Right | Switch to folder group 1 / 2 / 3 after current video |
| Down | Return to all folders after current video |

## Build

`egui-video` uses FFmpeg. Install FFmpeg 7 development libraries and `pkg-config` before building.

```powershell
cargo build --release
```

The app stores settings in the platform config directory under `rndWalker/settings.json`.

## Releases And Updates

Pushing a tag such as `v0.1.0` runs `.github/workflows/release.yml` and uploads ZIP assets for supported targets.

Release builds check `dikmri/rndWalker` on startup. If a newer compatible release is available, the executable is downloaded and replaced in place. Restart the app after the update notice appears.

## Legacy Electron Version

The previous JavaScript/Electron implementation is preserved under `legacy/electron/` for reference only. New development should target the Rust codebase.
