param(
    [string]$OutputDirectory = "publish"
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
dotnet publish SlideTranscriber.csproj -c Release -r win-x64 --self-contained false -o $OutputDirectory
Write-Host "Published to: $(Join-Path $PSScriptRoot $OutputDirectory)"
