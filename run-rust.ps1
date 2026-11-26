param(
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$llvmBin = 'C:\Program Files\LLVM\bin'
if (Test-Path -LiteralPath (Join-Path $llvmBin 'libclang.dll')) {
    $env:LIBCLANG_PATH = $llvmBin
}

if (-not $env:VULKAN_SDK) {
    $candidate = Get-ChildItem 'C:\VulkanSDK' -Directory -ErrorAction SilentlyContinue |
        Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'Include\vulkan\vulkan.h') } |
        Sort-Object Name -Descending |
        Select-Object -First 1
    if ($candidate) {
        $env:VULKAN_SDK = $candidate.FullName
    }
}
if (-not $env:VULKAN_SDK) {
    throw 'Vulkan SDK が見つかりません。VULKAN_SDK を設定してください。'
}

$workspace = (Get-Location).Path
if (-not $env:CARGO_TARGET_DIR -and $workspace.Length -gt 40) {
    # Vulkan のシェーダ生成はWindowsのパス長制限(250文字)に掛かりやすいため、
    # リポジトリが深い場合は短いターゲットディレクトリへ逃がす。
    $shortTarget = Join-Path $env:USERPROFILE 'svd-target'
    New-Item -ItemType Directory -Force -Path $shortTarget | Out-Null
    $env:CARGO_TARGET_DIR = $shortTarget
    Write-Host "CARGO_TARGET_DIR=$shortTarget (パス長対策)"
}

cargo run --release
