# 兰德沃克

`rndWalker` 是一个 Rust + eframe 桌面应用程序，可以随机播放本地文件夹中的视频和音乐。

旧的 Electron 版本已保存到 `legacy/electron/`，未来的开发将基于 Rust 版本进行。

## 主要特点

- 随机播放MP4视频
- MP4 文件夹管理最多 3 组
- 每个 MP4 组中最多可以注册 4 个文件夹
- MP3音频独立随机播放
- 文件夹预设、音量、静音、设置保存
- 使用 GitHub Releases 在启动时自动检查更新
- 适用于 Windows、macOS 和 Linux 的自动发布

## 键盘操作

| 钥匙 | 手术 |
| --- | --- |
| F5 | 重新加载视频文件夹 |
| F11 | 全屏切换 |
| Esc键 | 取消全屏或关闭设置屏幕 |
| 中号 | 静音开关 |
| 西/西 | 音量调高/音量调低 |
| 数/模 | 下一个随机视频/上一个视频 |
| 左/上/右 | 当前视频结束后切换到文件夹组1/2/3 |
| 在下面 | 当前视频结束后返回所有文件夹 |

## 建造

由于`egui-video`使用FFmpeg，因此开发环境需要FFmpeg 7开发库和`pkg-config`。

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
