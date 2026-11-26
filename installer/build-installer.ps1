param(
    [string]$Version = "0.1.0",
    [switch]$ForceFfmpegDownload,
    [string]$ExePath = ""
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

& (Join-Path $PSScriptRoot "prepare-assets.ps1") -ForceFfmpegDownload:$ForceFfmpegDownload -ExePath $ExePath

$candidates = @(
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)
$iscc = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $iscc) {
    throw "Inno Setup 6 が見つかりません。winget install --id JRSoftware.InnoSetup --exact で導入してください。"
}

& $iscc "/DAppVersion=$Version" (Join-Path $PSScriptRoot "SlideVideoDivider.iss")
if ($LASTEXITCODE -ne 0) {
    throw "インストーラの作成に失敗しました (ISCC exit code $LASTEXITCODE)"
}

$output = Join-Path $PSScriptRoot "..\dist\SlideVideoDivider-$Version-setup.exe"
if (-not (Test-Path -LiteralPath $output)) {
    throw "インストーラが生成されませんでした: $output"
}
Write-Host "インストーラ: $output"
