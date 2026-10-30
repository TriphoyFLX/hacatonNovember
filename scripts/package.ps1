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
New-Item -ItemType Directory -Force -Path (Join-Path $Dist "vision\air_control_vision") | Out-Null
Copy-Item (Join-Path $Root "vision\air_control_vision\*") (Join-Path $Dist "vision\air_control_vision") -Recurse
Copy-Item (Join-Path $Root "scripts\setup.ps1") (Join-Path $Dist "setup.ps1")
$Model = Join-Path $Root "vision\models\hand_landmarker.task"
if (Test-Path $Model) {
    New-Item -ItemType Directory -Force -Path (Join-Path $Dist "vision\models") | Out-Null
    Copy-Item $Model (Join-Path $Dist "vision\models\hand_landmarker.task")
}
$Iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (Test-Path $Iscc) {
    & $Iscc (Join-Path $Root "installer\AirControl.iss")
    Write-Host "Installer created in dist"
} else {
    Write-Host "Package folder: $Dist"
    Write-Host "Inno Setup 6 is not installed, so the .exe installer was not compiled. The folder is ready to copy."
}
