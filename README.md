# Slide Transcriber

音声付きのスライド動画を、**上半分にスライド画像・下半分に文字起こし**を配置したA4縦のノートPDFへ変換するWindowsアプリです。

Pythonは使用しません。.NET WinForms、Whisper.net（whisper.cppベース）、FFmpeg、PDFsharpで構成されています。

## 画面

- ダーク／ライトテーマ
- 上部タブ、下部ステータスバー
- スライド一覧・プレビュー・文字起こし編集の3ペイン
- スライドの中点分割、削除、画像差し替え、文字の前後移動
- プロジェクトの保存・再開

## 動作要件

- Windows 10 / 11 x64
- .NET 10 Desktop Runtime
- FFmpeg / FFprobe（PATHに追加）
- NVIDIA GPU + CUDA 13 Toolkit（GPU使用時。未検出時はCPUへフォールバック）
- Microsoft Visual C++ Redistributable 2022 x64

GPUがなくてもCPUで動作しますが、処理時間は長くなります。

## ビルド・起動

```powershell
git clone https://github.com/Runa-chama/SlideVideo_Divider.git
cd SlideVideo_Divider
dotnet build -c Release
dotnet run --project SlideTranscriber.csproj -c Release
```

単体exeとして配布する場合：

```powershell
.\publish.ps1
```

出力先は `publish\SlideTranscriber.exe` です。音声認識モデルはexeに含めず、初回利用時にダウンロードします。

## 使い方

1. 「変換」タブで音声付き動画を選択、またはドラッグ＆ドロップ
2. 出力フォルダを確認
3. 認識モデルを選択
   - 既定は `large-v3-turbo / Q8`
   - 初回は「ダウンロード」をクリック
4. 必要に応じてスライド検出しきい値・専門用語ヒントを調整
5. 「変換開始」
6. 完了後、「スライドノート」タブで内容を確認・修正
7. 「PDF出力」

## 出力

```text
出力フォルダ/
  動画名/
    slides/
      slide_001.jpg
      slide_002.jpg
      ...
    動画名_project.json
```

PDFは必要に応じて好きな場所へ保存できます。

`*_project.json` は「開く」ボタン、またはexeへドラッグ＆ドロップして再開できます。画像はプロジェクトフォルダからの相対パスで保存されます。

## PDF仕様

- A4縦
- 上半分：スライド画像を16:9のまま中央配置
- 下半分：対応する日本語文字起こし
- スライド番号・表示時間を表記
- 文字量に応じて自動でフォント縮小
- 最小サイズでも収まらない場合はPDF出力を中止し、該当スライドを警告

軽量化と日本語フォントの安定表示のため、PDF内の文字起こしは画像としてレンダリングされます。検索・コピーはできません。元の文字はプロジェクトJSONと編集画面に残ります。

## 処理の流れ

1. FFmpegでスライド切替を検出
2. 各スライドの安定した代表フレームをJPEG抽出
3. FFmpegで音声を16kHzモノラルWAVへ変換
4. Whisper.netで日本語文字起こし
5. 発話の開始・終了時刻とスライド表示時間を照合
6. 確認・修正后用PDF生成

処理はすべてローカルで行われ、動画や音声は外部サービスへ送信されません。モデルダウンロード時のみHugging Faceへアクセスします。

## 注意点

- 自動スライド検出は完璧ではありません。アニメーションやフェードでは「中点で分割」「削除」で調整してください。
- 音声とスライドの切替がずれる場合は、開始・終了秒を直接編集できます。
- 専門用語は「専門用語ヒント」に入れると精度が改善する場合があります。
- GPU／CPUの切り替えはアプリ再起動後に確実に反映されます。

## テスト

```powershell
# スライド検出・画像抽出・PDF生成
dotnet run --project tests\SmokeTest\SmokeTest.csproj -c Release -- --video "<動画ファイルのパス>"

# 小さなモデルでGPU文字起こしを検証
dotnet run --project tests\SmokeTest\SmokeTest.csproj -c Release -- --video "<動画ファイルのパス>" --whisper

# 実際の動画全体で文字起こし・対応付けを検証
dotnet run --project tests\SmokeTest\SmokeTest.csproj -c Release -- --video "<動画ファイルのパス>" --full-whisper
```

`SLIDE_TRANSCRIBER_TEST_VIDEO` 環境変数で動画パスを指定することもできます。

## 参考・依存ライブラリ

- [Whisper.net](https://github.com/sandrohanea/whisper.net) — MIT
- [PDFsharp](https://github.com/empira/PDFsharp) — MIT
- FFmpeg — 外部実行ファイルとして使用
