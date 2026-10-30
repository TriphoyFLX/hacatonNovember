$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

Write-Host "AIR CONTROL setup"
$Python = $null
foreach ($candidate in @("py -3.11", "py -3.12", "python")) {
    $parts = $candidate.Split(" ")
    try {
        if ($parts.Length -eq 2) {
            & $parts[0] $parts[1] -c "import sys; raise SystemExit(0 if sys.version_info >= (3, 11) and sys.version_info < (3, 13) else 1)"
        } else {
            & $parts[0] -c "import sys; raise SystemExit(0 if sys.version_info >= (3, 11) and sys.version_info < (3, 13) else 1)"
        }
        if ($LASTEXITCODE -eq 0) { $Python = $parts; break }
    } catch { }
}
if (-not $Python) {
    throw "Нужен Python 3.11 или 3.12. MediaPipe не ставится на Python 3.14."
}

$VenvPython = Join-Path $Root "vision\.venv\Scripts\python.exe"
if (-not (Test-Path $VenvPython)) {
    if ($Python.Length -eq 2) { & $Python[0] $Python[1] -m venv (Join-Path $Root "vision\.venv") }
    else { & $Python[0] -m venv (Join-Path $Root "vision\.venv") }
}
& $VenvPython -m pip install --upgrade pip
& $VenvPython -m pip install -r (Join-Path $Root "vision\requirements.txt")

$Model = Join-Path $Root "vision\models\hand_landmarker.task"
if (-not (Test-Path $Model)) {
    $Url = "https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task"
    Write-Host "Downloading hand landmarker model"
    New-Item -ItemType Directory -Force -Path (Split-Path $Model) | Out-Null
    Invoke-WebRequest -Uri $Url -OutFile $Model
}
& $VenvPython (Join-Path $Root "scripts\make_icon.py")
Write-Host "Python environment is ready. Build the app with: cargo build --release -p air-control-app"
