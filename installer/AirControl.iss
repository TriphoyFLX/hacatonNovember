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
Source: "..\dist\AirControl\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Запустить AIR CONTROL"; Flags: nowait postinstall skipifsilent
