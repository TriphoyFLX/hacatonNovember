$ErrorActionPreference = "Stop"
# Runs next to air-control.exe inside a packaged folder.
$Root = $PSScriptRoot
Set-Location $Root

Write-Host "AIR CONTROL — настройка камеры и голоса"
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

$Vision = Join-Path $Root "vision"
$VenvPython = Join-Path $Vision ".venv\Scripts\python.exe"
if (-not (Test-Path $VenvPython)) {
    Write-Host "Создаём virtualenv…"
    if ($Python.Length -eq 2) { & $Python[0] $Python[1] -m venv (Join-Path $Vision ".venv") }
    else { & $Python[0] -m venv (Join-Path $Vision ".venv") }
}

$Req = Join-Path $Vision "requirements-windows.txt"
if (-not (Test-Path $Req)) { $Req = Join-Path $Vision "requirements.txt" }
& $VenvPython -m pip install --upgrade pip
& $VenvPython -m pip install -r $Req
$VoiceReq = Join-Path $Root "voice\requirements.txt"
if (Test-Path $VoiceReq) {
    & $VenvPython -m pip install -r $VoiceReq
}

$Model = Join-Path $Vision "models\hand_landmarker.task"
if (-not (Test-Path $Model)) {
    $Url = "https://storage.googleapis.com/mediapipe-models/hand_landmarker/hand_landmarker/float16/1/hand_landmarker.task"
    Write-Host "Скачиваем модель руки…"
    New-Item -ItemType Directory -Force -Path (Split-Path $Model) | Out-Null
    Invoke-WebRequest -Uri $Url -OutFile $Model
}

$VoiceModel = Join-Path $Root "voice\models\vosk-model-small-ru-0.22"
if (-not (Test-Path (Join-Path $VoiceModel "conf\model.conf"))) {
    $Zip = Join-Path $Root "voice\models\vosk-model-small-ru-0.22.zip"
    Write-Host "Скачиваем модель голоса…"
    New-Item -ItemType Directory -Force -Path (Split-Path $Zip) | Out-Null
    Invoke-WebRequest -Uri "https://alphacephei.com/vosk/models/vosk-model-small-ru-0.22.zip" -OutFile $Zip
    Expand-Archive -Path $Zip -DestinationPath (Join-Path $Root "voice\models") -Force
    Remove-Item $Zip -Force
}

& $VenvPython -c "import cv2, mediapipe; print('OK', cv2.__version__)"
Write-Host "Готово. Запустите air-control.exe"
