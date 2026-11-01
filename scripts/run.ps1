$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Push-Location $Root
try {
    cargo build --release -p air-control-app
    if ($LASTEXITCODE -ne 0) { throw "Release build failed ($LASTEXITCODE)" }
    & (Join-Path $Root "target\release\air-control.exe")
} finally {
    Pop-Location
}
