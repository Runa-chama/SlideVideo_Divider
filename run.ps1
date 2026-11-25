param(
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
dotnet run --project SlideTranscriber.csproj -c $Configuration
