param(
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$llvmBin = 'C:\Program Files\LLVM\bin'
if (Test-Path -LiteralPath (Join-Path $llvmBin 'libclang.dll')) {
    $env:LIBCLANG_PATH = $llvmBin
}

$cuda13Nvcc = 'C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.4\bin\nvcc.exe'
if (Test-Path -LiteralPath $cuda13Nvcc) {
    $env:CUDACXX = $cuda13Nvcc
    $env:CUDA_HOME = 'C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.4'
}

$env:CMAKE_CUDA_ARCHITECTURES = '86;89'
cargo run --release --features cuda --bin SlideVideoDivider
