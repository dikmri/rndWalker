# 兰德沃克

<p对齐=“中心”>
<img src="assets/icons/rndwalker-128.png" alt="rndWalker 图标" width="128" height="128">
</p>

`rndWalker` 是一个 Rust + eframe 桌面应用程序，可以随机播放本地文件夹中的视频和音乐。

旧的 Electron 版本已保存到 `legacy/electron/`，未来的开发将基于 Rust 版本进行。

## 主要特点

- 随机播放MP4视频
- MP4 文件夹管理最多 3 组
- 每个 MP4 组中最多可以注册 4 个文件夹
- 多视图播放，让视频充满屏幕
- MP3音频独立随机播放
- 文件夹预设、音量、静音、设置保存
- 使用 GitHub Releases 在启动时自动检查更新
- 适用于 Windows、macOS 和 Linux 的自动发布

## 安装

自动检测GitHub Release的最新版本并将其安装在用户区。

### 视窗

在 PowerShell 中运行以下命令：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.ps1 | iex"
```

默认安装位置是“%LOCALAPPDATA%\Programs\rndWalker”。创建开始菜单快捷方式并将安装位置添加到用户的 PATH 中。

### macOS/Linux

在终端中运行以下命令：

```sh
curl -fsSL https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.sh | sh
```

默认安装位置是“~/.local/share/rndWalker”。在 `~/.local/bin/rndWalker` 中创建启动链接。 macOS 自动检测 Intel / Apple Silicon，Linux 支持 x86_64。

如果 Homebrew 可用，macOS 会自动检查 FFmpeg。在 Linux 上，如果“apt-get”、“dnf”或“pacman”可用，请安装 FFmpeg 和 SDL2 运行时。在 Linux 上，您可能需要输入“sudo”密码。如果您想避免自动部署依赖项，请运行：

```sh
curl -fsSL https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.sh | env RNDWALKER_SKIP_DEPS=1 sh
```

## 键盘操作

| 钥匙 | 手术 |
| --- | --- |
| F5 | 重新加载视频文件夹 |
| F11 | 全屏切换 |
| Esc键 | 取消全屏或关闭设置屏幕 |
| 中号 | 静音开关 |
| 西/西 | 音量调高/音量调低 |
| 数/模 | 下一个随机视频/上一个视频。多视图重组中的“D” |
| 左/上/右 | 切换到文件夹组 1 / 2 / 3 |
| 在下面 | 返回所有文件夹 |

## 多视图

如果您在设置屏幕上选中“启用多视图”，则当前视频文件夹中的视频将在整个屏幕上同时播放。

您可以使用“视频大小”滑块调整大致的图块大小。每个视频均完整显示，不进行剪切，并调整每行的高度，以确保没有黑缝（仅在垂直方向可以调整百分之几的比例）。尽可能在同一屏幕上选择不同的视频。如果视频数量少于图块数量，它们将重叠。

每个多视图视频将无声播放，MP3 文件夹的音频播放将像以前一样独立继续。

每个图块都会以与显示尺寸相匹配的分辨率进行缩小和解码，因此即使有很多图块，也可以减少 CPU/GPU 负载。视频加载时会短暂出现黑色单元格，准备就绪后会发生变化。当您调整窗口大小时，播放图块保持不变，仅布局随之变化。

## 建造

对于视频解码，我们使用“vendor/egui-video”的分支版本。开发环境需要FFmpeg 7开发库和`pkg-config`。

```powershell
cargo build --release
```

设置在操作系统标准设置目录中保存为“rndWalker/settings.json”。

## 发布和更新

推送诸如“v0.1.12”之类的标签将运行“.github/workflows/release.yml”并将支持平台的 ZIP 上传到 GitHub Releases。

对于发布版本，请在启动时查看 `dikmri/rndWalker` 的 GitHub Releases。如果有更新，它将替换可执行文件并提示您在完成后重新启动。

## 多语言文档

自述文件和发行说明的原始副本将以日语进行管理。当日语文档在“main”分支中更新时，“.github/workflows/docs-i18n.yml”会生成翻译并将其反映在“docs/i18n/”中。

- 英语：`docs/i18n/README.en.md`
- 中文：`docs/i18n/README.zh-CN.md`
- 韩语：`docs/i18n/README.ko.md`

由于翻译是自动生成的，因此请反映对原始日语版本要求准确性的任何更正。
