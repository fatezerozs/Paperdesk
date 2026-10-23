#define AppVersion "0.3.0"
[Setup]
AppId={{96E0CC88-62DF-4E7C-B891-A59653E2F541}
AppName=拾题 Paperdesk
AppVersion={#AppVersion}
AppPublisher=Paperdesk
DefaultDirName={localappdata}\Programs\Paperdesk
DefaultGroupName=拾题 Paperdesk
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=Paperdesk-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\..\assets\paperdesk.ico
UninstallDisplayIcon={app}\paperdesk.exe
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; Flags: unchecked

[Dirs]
Name: "{localappdata}\Paperdesk"; Flags: uninsneveruninstall

[Files]
Source: "..\..\target\release\paperdesk.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs
Source: "..\..\assets\fonts\OFL.txt"; DestDir: "{app}\licenses"; DestName: "NotoSansSC-OFL.txt"; Flags: ignoreversion
Source: "..\..\examples\sample-paper.txt"; DestDir: "{app}\examples"; Flags: ignoreversion

Source: "..\..\tools\ocr\*"; DestDir: "{app}\ocr"; Flags: ignoreversion recursesubdirs
Source: "..\..\启动拾题.cmd"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\scripts\start.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion

[Icons]
Name: "{group}\拾题 Paperdesk"; Filename: "{app}\paperdesk.exe"; Parameters: "--open"; WorkingDir: "{localappdata}\Paperdesk"
Name: "{group}\题库数据目录"; Filename: "{localappdata}\Paperdesk\data"
Name: "{group}\卸载拾题"; Filename: "{uninstallexe}"
Name: "{autodesktop}\拾题 Paperdesk"; Filename: "{app}\paperdesk.exe"; Parameters: "--open"; WorkingDir: "{localappdata}\Paperdesk"; Tasks: desktopicon

[Run]
Filename: "{app}\paperdesk.exe"; Parameters: "--open"; WorkingDir: "{localappdata}\Paperdesk"; Description: "启动拾题"; Flags: nowait postinstall skipifsilent

; The database is outside the install folder and is intentionally retained on uninstall.
