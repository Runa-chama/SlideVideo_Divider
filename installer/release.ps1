param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [string]$NotesFile = "",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot "build-installer.ps1") -Version $Version
}
$asset = Join-Path $PSScriptRoot "..\dist\SlideVideoDivider-$Version-setup.exe"
if (-not (Test-Path -LiteralPath $asset)) {
    throw "インストーラが見つかりません: $asset"
}

$tag = "v$Version"
if (-not (git tag --list $tag)) {
    git tag $tag
}
git push origin $tag

$notes = "SlideVideoDivider $Version"
if ($NotesFile -and (Test-Path -LiteralPath $NotesFile)) {
    $notes = Get-Content -LiteralPath $NotesFile -Raw -Encoding UTF8
}

$credential = "protocol=https`nhost=github.com`n`n" | git credential fill
$token = ($credential | Select-String -Pattern "^password=(.+)$").Matches.Groups[1].Value
if (-not $token) {
    throw "GitHubの認証情報を取得できませんでした。git credential manager の設定を確認してください。"
}

$headers = @{
    Authorization          = "token $token"
    "User-Agent"           = "SlideVideoDivider-release"
    Accept                 = "application/vnd.github+json"
    "X-GitHub-Api-Version" = "2022-11-28"
}
$body = @{
    tag_name   = $tag
    name       = "SlideVideoDivider $Version"
    body       = $notes
    draft      = $false
    prerelease = $false
} | ConvertTo-Json

$existing = $null
try {
    $existing = Invoke-RestMethod -Method Get -Uri "https://api.github.com/repos/Runa-chama/SlideVideo_Divider/releases/tags/$tag" -Headers $headers
} catch {
    # タグに対応するリリースが存在しない場合は新規作成する。
}
$release = if ($existing) {
    $existing
} else {
    Invoke-RestMethod -Method Post -Uri "https://api.github.com/repos/Runa-chama/SlideVideo_Divider/releases" -Headers $headers -Body $body -ContentType "application/json"
}
$uploadUrl = ($release.upload_url -replace "\{.*\}$", "")
$assetName = [uri]::EscapeDataString((Split-Path $asset -Leaf))
Invoke-RestMethod -Method Post -Uri "${uploadUrl}?name=$assetName" -Headers $headers -ContentType "application/octet-stream" -InFile $asset | Out-Null

Write-Host "リリース完了: $($release.html_url)"
