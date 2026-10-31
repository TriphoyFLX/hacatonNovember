$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot
Set-Location $Root

$BuildTools = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"

function AddCargoPath {
    $cargo = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargo) {
        $env:Path = "$cargo;$env:Path"
    }
}

function ImportVcVars {
    $vcvars = Join-Path $BuildTools "VC\Auxiliary\Build\vcvars64.bat"
    if (-not (Test-Path $vcvars)) {
        return $false
    }

    cmd /c "`"$vcvars`" && set" | ForEach-Object {
        if ($_ -match "^(.*?)=(.*)$") {
            [Environment]::SetEnvironmentVariable($matches[1], $matches[2], "Process")
        }
    }
    return [bool](Get-Command link.exe -ErrorAction SilentlyContinue)
}

function TestWindowsSdk {
    $kits = "C:\Program Files (x86)\Windows Kits\10"
    if (-not (Test-Path $kits)) {
        return $false
    }
    return [bool](Get-ChildItem -Path (Join-Path $kits "Lib") -Recurse -Filter "kernel32.lib" -ErrorAction SilentlyContinue | Select-Object -First 1)
}

function EnsureRust {
    AddCargoPath
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        return
    }

    Write-Host "Ставлю Rust..."
    $installer = Join-Path $env:TEMP "rustup-init.exe"
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $installer
    & $installer -y --default-toolchain stable --profile minimal
    AddCargoPath
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "Rust не появился в PATH. Закрой терминал, открой заново и снова запусти start.cmd"
    }
}

function WaitForVsInstaller {
    $names = @("vs_BuildTools", "vs_setup_bootstrapper", "vs_installer", "setup")
    $deadline = (Get-Date).AddMinutes(25)
    $seen = $false
    while ((Get-Date) -lt $deadline) {
        $busy = Get-Process -Name $names -ErrorAction SilentlyContinue
        if (-not $busy) {
            if ($seen) { Start-Sleep -Seconds 3 }
            return
        }
        $seen = $true
        Write-Host "Жду, пока доставится Visual C++..."
        Start-Sleep -Seconds 8
    }
}

function EnsureMsvc {
    WaitForVsInstaller
    if (ImportVcVars -and (TestWindowsSdk)) {
        return
    }

    $setup = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\setup.exe"
    if (-not (Test-Path $setup)) {
        Write-Host "Ставлю Visual Studio Build Tools..."
        winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements --accept-package-agreements --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    }

    if (-not (TestWindowsSdk)) {
        Write-Host "Доставляю Windows SDK. Это один раз..."
        Start-Process -FilePath $setup -ArgumentList @(
            "modify",
            "--installPath", "`"$BuildTools`"",
            "--add", "Microsoft.VisualStudio.Workload.VCTools",
            "--add", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "--add", "Microsoft.VisualStudio.Component.Windows11SDK.22621",
            "--includeRecommended",
            "--passive",
            "--norestart"
        ) -Wait
        WaitForVsInstaller
    }

    if (-not (ImportVcVars)) {
        throw "Не вижу компилятор C++. Перезапусти компьютер и снова запусти start.cmd"
    }
    if (-not (TestWindowsSdk)) {
        throw "Windows SDK ещё ставится. Подожди 2-3 минуты и снова запусти start.cmd"
    }
}

function EnsurePython {
    $setup = Join-Path $Root "scripts\setup.ps1"
    $venv = Join-Path $Root "vision\.venv\Scripts\python.exe"
    $model = Join-Path $Root "vision\models\hand_landmarker.task"
    if ((Test-Path $venv) -and (Test-Path $model)) {
        return
    }
    Write-Host "Готовлю Python и модель..."
    & $setup
}

EnsureRust
EnsureMsvc
EnsurePython

$exe = Join-Path $Root "target\release\air-control.exe"
if (-not (Test-Path $exe)) {
    Write-Host "Собираю AIR CONTROL..."
    cargo build --release -p air-control-app
}

Write-Host "Запускаю..."
& (Join-Path $Root "scripts\run.ps1")
