; ai-image Inno Setup 打包脚本
; 版本号由 release.ps1 通过 /DAppVersion 传入（默认值仅作兜底）
; 产物：release/ai-image-v{AppVersion}-Setup.exe（免管理员，安装到用户目录）

#define MyAppName "ai-image"
#define MyAppExeName "ai-image.exe"

#ifndef AppVersion
#define AppVersion "0.3.0"
#endif

[Setup]
AppId={{B7E9D4F2-3C61-4A58-9D20-6F1E8A2B4C7D}}
AppName={#MyAppName}
AppVersion={#AppVersion}
AppVerName={#MyAppName} v{#AppVersion}
DefaultDirName={localappdata}\Programs\{#MyAppName}
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
OutputDir=..\..\release
OutputBaseFilename=ai-image-v{#AppVersion}-Setup
SetupIconFile=..\icons\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "附加任务："

[Files]
Source: "..\target\release\ai-image.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "立即运行 {#MyAppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; 卸载时清理运行日志（配置文件保留，避免误删用户密钥）
Type: filesandordirs; Name: "{app}\logs"
