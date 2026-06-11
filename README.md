# rndWalker

<p align="center">
  <img src="assets/icons/rndwalker-128.png" alt="rndWalker icon" width="128" height="128">
</p>

`rndWalker` は、ローカルフォルダ内の動画と音楽をランダムに再生する Rust + eframe 製のデスクトップアプリです。

旧 Electron 版は `legacy/electron/` に退避してあり、今後の開発は Rust 版を基準に進めます。

## 主な機能

- MP4 動画をランダム再生
- 最大 3 グループの MP4 フォルダ管理
- 各 MP4 グループに最大 4 つのフォルダを登録可能
- 動画を画面いっぱいに敷き詰めるマルチビュー再生
- MP3 音声の独立ランダム再生
- フォルダプリセット、音量、ミュート、設定保存
- GitHub Releases を使った起動時の自動更新チェック
- Windows、macOS、Linux 向けの自動リリース

## インストール

最新版の GitHub Release を自動判定し、ユーザー領域にインストールします。

### Windows

PowerShell で次を実行してください。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.ps1 | iex"
```

既定のインストール先は `%LOCALAPPDATA%\Programs\rndWalker` です。スタートメニューのショートカットを作成し、ユーザー PATH にインストール先を追加します。

### macOS / Linux

ターミナルで次を実行してください。

```sh
curl -fsSL https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.sh | sh
```

既定のインストール先は `~/.local/share/rndWalker` です。`~/.local/bin/rndWalker` に起動用リンクを作成します。macOS は Intel / Apple Silicon を自動判定し、Linux は x86_64 に対応しています。

macOS では Homebrew が利用できる場合に FFmpeg を自動確認します。Linux では `apt-get`、`dnf`、`pacman` のいずれかが利用できる場合に FFmpeg と SDL2 のランタイムを導入します。Linux では `sudo` のパスワード入力が必要になる場合があります。依存関係の自動導入を避ける場合は、次のように実行してください。

```sh
curl -fsSL https://raw.githubusercontent.com/dikmri/rndWalker/main/scripts/install.sh | env RNDWALKER_SKIP_DEPS=1 sh
```

## キーボード操作

| キー | 動作 |
| --- | --- |
| F5 | 動画フォルダを再読み込み |
| F11 | フルスクリーン切替 |
| Esc | フルスクリーン解除、または設定画面を閉じる |
| M | ミュート切替 |
| W / S | 音量アップ / 音量ダウン |
| D / A | 次のランダム動画 / 前の動画。マルチビュー中の `D` は再シャッフル |
| 左 / 上 / 右 | フォルダグループ 1 / 2 / 3 へ切替 |
| 下 | 全フォルダへ戻す |

## マルチビュー

設定画面で `マルチビューを有効にする` にチェックを入れると、現在の動画フォルダ内の動画を画面全体に敷き詰めて同時再生します。

`動画サイズ` スライダーでタイルサイズの目安を調整できます。動画のアスペクト比は維持され、同じ画面内では可能な限り別々の動画を選びます。動画数がタイル数より少ない場合は重複します。

マルチビューの各動画は無音で再生され、MP3 フォルダの音声再生は従来どおり独立して続きます。

## ビルド

`egui-video` が FFmpeg を使うため、開発環境には FFmpeg 7 の開発ライブラリと `pkg-config` が必要です。

```powershell
cargo build --release
```

設定は OS 標準の設定ディレクトリに `rndWalker/settings.json` として保存されます。

## リリースと更新

`v0.1.12` のようなタグを push すると `.github/workflows/release.yml` が実行され、対応プラットフォーム向けの ZIP が GitHub Releases にアップロードされます。

リリース版は起動時に `dikmri/rndWalker` の GitHub Releases を確認します。更新がある場合は実行ファイルを置き換え、完了後に再起動を促します。

## 多言語ドキュメント

README やリリースノートの正本は日本語で管理します。`main` ブランチで日本語ドキュメントが更新されると、`.github/workflows/docs-i18n.yml` が翻訳を生成して `docs/i18n/` に反映します。

- 英語: `docs/i18n/README.en.md`
- 中国語: `docs/i18n/README.zh-CN.md`
- 韓国語: `docs/i18n/README.ko.md`

翻訳は自動生成のため、正確性が必要な修正は日本語の正本へ反映してください。
