param(
    [switch]$ForceFfmpegDownload,
    [string]$ExePath = ""
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$staging = Join-Path $PSScriptRoot "staging"
$tools = Join-Path $staging "tools"
New-Item -ItemType Directory -Force -Path $tools | Out-Null

$exeCandidates = @()
if ($ExePath) {
    $exeCandidates += $ExePath
}
if ($env:CARGO_TARGET_DIR) {
    $exeCandidates += Join-Path $env:CARGO_TARGET_DIR "release\SlideVideoDivider.exe"
}
$exeCandidates += Join-Path $PSScriptRoot "..\target\release\SlideVideoDivider.exe"
$exeCandidates += Join-Path $env:USERPROFILE "svd-target\release\SlideVideoDivider.exe"
$exeCandidates += Join-Path $env:USERPROFILE "svd\release\SlideVideoDivider.exe"
$exe = $exeCandidates |
    Where-Object { Test-Path -LiteralPath $_ } |
    ForEach-Object { Get-Item -LiteralPath $_ } |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $exe) {
    throw "ビルド済みの実行ファイルがありません。先に cargo build --release を実行してください。"
}
Write-Host "実行ファイル: $($exe.FullName)"
Copy-Item -LiteralPath $exe.FullName -Destination (Join-Path $staging "SlideVideoDivider.exe") -Force

$ffmpeg = Join-Path $tools "ffmpeg.exe"
$ffprobe = Join-Path $tools "ffprobe.exe"
if ($ForceFfmpegDownload -or -not (Test-Path -LiteralPath $ffmpeg) -or -not (Test-Path -LiteralPath $ffprobe)) {
    $zip = Join-Path $env:TEMP "ffmpeg-release-essentials.zip"
    $extract = Join-Path $env:TEMP "ffmpeg-release-essentials"
    Write-Host "FFmpegをダウンロードしています..."
    Invoke-WebRequest -Uri "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -OutFile $zip -UseBasicParsing
    if (Test-Path -LiteralPath $extract) {
        Remove-Item -LiteralPath $extract -Recurse -Force
    }
    Expand-Archive -LiteralPath $zip -DestinationPath $extract
    $bin = Get-ChildItem -LiteralPath $extract -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
    if (-not $bin) {
        throw "FFmpegの展開に失敗しました。"
    }
    Copy-Item -LiteralPath $bin.FullName -Destination $ffmpeg -Force
    Copy-Item -LiteralPath (Join-Path $bin.DirectoryName "ffprobe.exe") -Destination $ffprobe -Force
    $license = Get-ChildItem -LiteralPath $extract -Recurse -Filter "LICENSE*" | Select-Object -First 1
    if ($license) {
        Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $tools "FFMPEG-LICENSE.txt") -Force
    }
}

Copy-Item -LiteralPath (Join-Path $PSScriptRoot "THIRD-PARTY-NOTICES.txt") -Destination (Join-Path $staging "THIRD-PARTY-NOTICES.txt") -Force
Write-Host "ステージング完了: $staging"
