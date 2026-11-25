param(
    [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
dotnet run --project SlideVideoDivider.csproj -c $Configuration
