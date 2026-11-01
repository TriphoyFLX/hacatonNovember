param(
    [string]$Label = "instruments",
    [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not $OutputDirectory) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $OutputDirectory = Join-Path $Root "target\profiles\$stamp-$Label"
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$previousProfile = $env:AIR_CONTROL_PROFILE
$previousDirectory = $env:AIR_CONTROL_PROFILE_DIR
Push-Location $Root
try {
    cargo build --release -p air-control-app
    if ($LASTEXITCODE -ne 0) { throw "Release build failed ($LASTEXITCODE)" }
    $env:AIR_CONTROL_PROFILE = "1"
    $env:AIR_CONTROL_PROFILE_DIR = $OutputDirectory
    Write-Host "Profile: $OutputDirectory"
    Write-Host "Enable the camera and play for 60 seconds per instrument, then close the app."
    & (Join-Path $Root "target\release\air-control.exe")
    $python = Join-Path $Root "vision\.venv\Scripts\python.exe"
    if (-not (Test-Path -LiteralPath $python)) { $python = "python" }
    & $python (Join-Path $Root "scripts\summarize-profile.py") $OutputDirectory
} finally {
    $env:AIR_CONTROL_PROFILE = $previousProfile
    $env:AIR_CONTROL_PROFILE_DIR = $previousDirectory
    Pop-Location
}
