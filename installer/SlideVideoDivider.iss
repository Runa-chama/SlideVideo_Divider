#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

#define AppName "SlideVideoDivider"
#define AppPublisher "Runa-chama"
#define AppUrl "https://github.com/Runa-chama/SlideVideo_Divider"

[Setup]
AppId={{C7B0F1A9-3B5C-4D8E-A6F2-91D5C3E7B820}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}
AppUpdatesURL={#AppUrl}/releases
DefaultDirName={localappdata}\Programs\SlideVideoDivider
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=no
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=SlideVideoDivider-{#AppVersion}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
UninstallDisplayIcon={app}\SlideVideoDivider.exe
UninstallDisplayName={#AppName}
CloseApplications=force
RestartApplications=no
SetupLogging=yes
VersionInfoVersion={#AppVersion}
VersionInfoDescription={#AppName} Setup

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[Tasks]
Name: "desktopicon"; Description: "デスクトップにショートカットを作成する"; GroupDescription: "追加タスク:"; Flags: unchecked

[Files]
Source: "staging\SlideVideoDivider.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\tools\ffmpeg.exe"; DestDir: "{app}\tools"; Flags: ignoreversion
Source: "staging\tools\ffprobe.exe"; DestDir: "{app}\tools"; Flags: ignoreversion
Source: "staging\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; DestName: "README.md"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\SlideVideoDivider.exe"
Name: "{group}\{#AppName} をアンインストール"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\SlideVideoDivider.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\SlideVideoDivider.exe"; Description: "{#AppName} を起動する"; Flags: nowait postinstall skipifsilent
