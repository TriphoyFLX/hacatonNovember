$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
cargo build --release -p air-control-app
$Dist = Join-Path $Root "dist\AirControl"
if (Test-Path $Dist) { Remove-Item $Dist -Recurse -Force }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null
Copy-Item (Join-Path $Root "target\release\air-control.exe") $Dist
Copy-Item (Join-Path $Root "README.md") $Dist
New-Item -ItemType Directory -Force -Path (Join-Path $Dist "vision") | Out-Null
Copy-Item (Join-Path $Root "vision\requirements.txt") (Join-Path $Dist "vision\requirements.txt") -Force
if (Test-Path (Join-Path $Root "vision\requirements-windows.txt")) {
    Copy-Item (Join-Path $Root "vision\requirements-windows.txt") (Join-Path $Dist "vision\requirements-windows.txt") -Force
}
New-Item -ItemType Directory -Force -Path (Join-Path $Dist "vision\air_control_vision") | Out-Null
Get-ChildItem (Join-Path $Root "vision\air_control_vision") -File | Where-Object { $_.Extension -in ".py",".txt",".md" } | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $Dist "vision\air_control_vision\$($_.Name)") -Force
}
New-Item -ItemType Directory -Force -Path (Join-Path $Dist "voice\air_control_voice") | Out-Null
Copy-Item (Join-Path $Root "voice\requirements.txt") (Join-Path $Dist "voice\requirements.txt") -Force
Get-ChildItem (Join-Path $Root "voice\air_control_voice") -File | Where-Object { $_.Extension -in ".py",".txt",".md" } | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $Dist "voice\air_control_voice\$($_.Name)") -Force
}
Copy-Item (Join-Path $Root "scripts\setup-dist.ps1") (Join-Path $Dist "setup.ps1") -Force
$Model = Join-Path $Root "vision\models\hand_landmarker.task"
if (Test-Path $Model) {
    New-Item -ItemType Directory -Force -Path (Join-Path $Dist "vision\models") | Out-Null
    Copy-Item $Model (Join-Path $Dist "vision\models\hand_landmarker.task")
}
# Prefer the already-built repo venv so the package runs without a long pip install.
$RepoVenv = Join-Path $Root "vision\.venv"
$DistVenv = Join-Path $Dist "vision\.venv"
if ((Test-Path $RepoVenv) -and -not (Test-Path $DistVenv)) {
    cmd /c mklink /J "$DistVenv" "$RepoVenv" | Out-Null
}
$Iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (Test-Path $Iscc) {
    & $Iscc (Join-Path $Root "installer\AirControl.iss")
    Write-Host "Installer created in dist"
} else {
    Write-Host "Package folder: $Dist"
    Write-Host "Inno Setup 6 is not installed, so the .exe installer was not compiled. The folder is ready to copy."
}
