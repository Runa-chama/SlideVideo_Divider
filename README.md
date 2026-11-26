# SlideVideoDivider

音声付きスライド動画を、スライド画像と文字起こしのPDFへ変換するWindowsアプリのRust版です。

Pythonは使用しません。

## 技術構成

- UI：eframe / egui
- 音声認識：whisper-rs（whisper.cpp / Vulkan GPU）
- 動画・音声処理：FFmpeg / FFprobe
- PDF：printpdf
- 画像・文字描画：image / imageproc / ab_glyph
- 設定・プロジェクト：serde / JSON
- 自動更新：GitHub Releases API

## 画面

- 上部タブ：変換 / スライドノート / 設定
- 下部ステータスバー
- 左：スライド一覧
- 中央：スライドプレビュー
- 右：文字起こし編集
- ダーク／ライトテーマ
- Undo / Redo（`Ctrl+Z` / `Ctrl+Y` / `Ctrl+Shift+Z`）

## ビルド要件

- Rust 1.95以降
- Visual Studio Build Tools（MSVC）
- LLVM / libclang
- CMake
- FFmpeg / FFprobe（PATHに追加）
- Vulkan SDK 1.3以降（GPU版）
- Microsoft Visual C++ Redistributable 2022 x64

whisper-rsはビルド時にlibclangを使用します。LLVMを導入し、必要に応じて `LIBCLANG_PATH` を設定してください。

```powershell
winget install --id LLVM.LLVM --exact
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
```

GPU版はVulkanバックエンドを使用します。Vulkan SDKを導入すると `VULKAN_SDK` が設定されます。同梱の `run-rust.ps1` は `C:\VulkanSDK` 配下のSDKを自動検出してビルドします。

## GPU版ビルド

```powershell
cargo build --release
```

出力：

```text
target\release\SlideVideoDivider.exe
```

起動時にGPUを自動検出します。GPUがある場合は設定画面でGPU実行を選択でき、GPUがない場合はCPUで実行します。GPU初期化に失敗した場合もCPUへフォールバックします。

## CPU版ビルド

```powershell
cargo build --release --no-default-features
```

## 起動

```powershell
cargo run --release
```

または、Vulkan SDKを自動検出する同梱スクリプト：

```powershell
.\run-rust.ps1
```

または：

```text
target\release\SlideVideoDivider.exe
```

初回利用時に音声認識モデルをダウンロードします。既定は `large-v3-turbo / Q8` です。

## PDF出力

PDF出力時に次のモードを選択できます。

- スライド＋文字起こし（既定）
- スライドのみ
- 文字起こしのみ

すべてA4縦で、1スライド＝1ページです。最小フォントサイズは設定タブまたはPDF出力ダイアログで変更できます。

軽量化のためPDF内の文字起こしは画像として描画されます。

## プロジェクト互換性

以前の `.NET版` が保存した `*_project.json` を読み込めます。

- 画像はプロジェクトフォルダからの相対パスで保存
- 空行は自動除去
- 既存の音声認識モデル保存先を設定から再利用可能

## テスト

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --no-default-features -- -D warnings
cargo test --no-default-features
cargo clippy --release --all-targets -- -D warnings
cargo test --release

# 実動画のエンドツーエンド確認
cargo run --release --no-default-features --bin smoke -- --video "<動画ファイルのパス>" --model "<whisper-model.bin>" --model-index 4 --cpu --pdf
```

Vulkan版のビルドでは、リポジトリのパスが深いとWindowsのパス長制限でシェーダ生成が失敗します。その場合は短いパスを指定してください。

```powershell
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\svd"
cargo build --release
```

## インストーラ

`installer\build-installer.ps1` でInno Setup用のインストーラを作成します。FFmpeg / FFprobeは自動でダウンロードして同梱されます。

```powershell
cargo build --release
.\installer\build-installer.ps1 -Version 0.1.0
```

出力：`dist\SlideVideoDivider-0.1.0-setup.exe`

インストール先はユーザー単位（`%LOCALAPPDATA%\Programs\SlideVideoDivider`）で、管理者権限は不要です。

## 自動更新

起動時にGitHub Releasesの最新版を確認し、新しいバージョンがあれば画面上部に更新ボタンを表示します。ボタンを押すとインストーラをダウンロードしてサイレント更新し、アプリを再起動します。

- 設定タブから「今すぐ確認」で手動確認も可能
- 起動時の確認は設定タブで無効化可能

## ライセンス

- 同梱フォント：M PLUS 1（SIL Open Font License 1.1）
- whisper-rs：Unlicense
- eframe / egui：MIT / Apache-2.0
- printpdf：MIT
- FFmpeg / FFprobe（インストーラに同梱）：LGPL v3
