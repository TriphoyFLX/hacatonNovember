$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Release = Join-Path $Root "target\release\air-control.exe"
$Debug = Join-Path $Root "target\debug\air-control.exe"
if (Test-Path $Release) {
    & $Release
} elseif (Test-Path $Debug) {
    & $Debug
} else {
    Set-Location $Root
    cargo run -p air-control-app --release
}
