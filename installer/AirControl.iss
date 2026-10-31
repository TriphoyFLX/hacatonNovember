#define MyAppName "AIR CONTROL"
#define MyAppVersion "0.9.0"
#define MyAppPublisher "AIR CONTROL"
#define MyAppExeName "air-control.exe"

[Setup]
AppId={{8B5E5B2A-1E4A-4A4E-9C1A-6E0A0B6F2C11}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\AirControl
DefaultGroupName={#MyAppName}
OutputDir=..\dist
OutputBaseFilename=AirControl-Setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
WizardStyle=modern

[Files]
; .venv in the packaged folder is a junction to the repo and must not ship.
Source: "..\dist\AirControl\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion; Excludes: "vision\.venv,vision\.venv\*"

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"

[Run]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\setup.ps1"""; WorkingDir: "{app}"; Description: "Настроить камеру (Python 3.11 или 3.12, один раз)"; Flags: postinstall waituntilterminated
Filename: "{app}\{#MyAppExeName}"; Description: "Запустить AIR CONTROL"; Flags: nowait postinstall skipifsilent
