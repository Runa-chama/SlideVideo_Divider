# SlideVideoDivider

音声付きスライド動画を、スライド画像と文字起こしのPDFへ変換するWindowsアプリのRust版です。

Pythonは使用しません。

## 技術構成

- UI：eframe / egui
- 音声認識：whisper-rs（whisper.cpp）
- 動画・音声処理：FFmpeg / FFprobe
- PDF：printpdf
- 画像・文字描画：image / imageproc / ab_glyph
- 設定・プロジェクト：serde / JSON

## 画面

- 上部タブ：変換 / スライドノート / 設定
- 下部ステータスバー
- 左：スライド一覧
- 中央：スライドプレビュー
- 右：文字起こし編集
- ダーク／ライトテーマ
- Undo / Redo（`Ctrl+Z` / `Ctrl+Y` / `Ctrl+Shift+Z`）
- 文字起こしの再関連付け（Whisperの単語時刻を使用）

## ビルド要件

- Rust 1.95以降
- Visual Studio Build Tools（MSVC）
- LLVM / libclang
- CMake
- FFmpeg / FFprobe（PATHに追加）
- CUDA Toolkit 12.4以降（GPU版）
- Microsoft Visual C++ Redistributable 2022 x64

whisper-rsはビルド時にlibclangを使用します。LLVMを導入し、必要に応じて `LIBCLANG_PATH` を設定してください。

```powershell
winget install --id LLVM.LLVM --exact
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
```

Visual Studio 2026とCUDA 12.9の組み合わせはNVCCに未対応です。CUDA 13.4以降を推奨します。同梱の `run-rust.ps1` はCUDA 13.4を優先的に選択します。

## GPU版ビルド

```powershell
$env:CMAKE_CUDA_ARCHITECTURES = "86;89"
cargo build --release --features cuda
```

出力：

```text
target\release\SlideVideoDivider.exe
```

GPU初期化に失敗した場合はCPUへフォールバックします。

## CPU版ビルド

```powershell
cargo build --release --no-default-features
```

## 起動

```powershell
cargo run --release --features cuda
```

または、CUDA 13.4を優先的に選択する同梱スクリプト：

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

# 実動画のエンドツーエンド確認
cargo run --release --no-default-features --bin smoke -- --video "<動画ファイルのパス>" --model "<whisper-model.bin>" --model-index 4 --cpu --pdf
```

## ライセンス

- 同梱フォント：M PLUS 1（SIL Open Font License 1.1）
- whisper-rs：Unlicense
- eframe / egui：MIT / Apache-2.0
- printpdf：MIT
