#ifndef AppVersion
  #error AppVersion must be supplied by build.ps1
#endif

[Setup]
AppId=Jupiter.ChessEngineCompiler
AppName=Chess Engine Compiler
AppVersion={#AppVersion}
AppPublisher=Ian Rastall
AppPublisherURL=https://github.com/ianrastall/chess-engine-compiler
AppUpdatesURL=https://github.com/ianrastall/chess-engine-compiler/releases
DefaultDirName={localappdata}\Programs\Jupiter\ChessEngineCompiler
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.26100
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=ChessEngineCompiler-{#AppVersion}-win-x64-setup
UninstallDisplayIcon={app}\ChessEngineCompiler.exe
SetupIconFile=..\src\ChessEngineCompiler.App\Assets\app.ico
AppMutex=Local\Jupiter.ChessEngineCompiler
CloseApplications=no
RestartApplications=no
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Tell Explorer and Start to refresh icons after install and uninstall, so a reinstall
; does not leave a blank Start menu icon.
ChangesAssociations=yes

[Files]
Source: "..\dist\ChessEngineCompiler\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs; Excludes: "*.pdb"

[InstallDelete]
; Versions before 24 October 2026 put the shortcut in a Jupiter folder; Start now lists the app by name.
Type: files; Name: "{autoprograms}\Jupiter\Chess Engine Compiler.lnk"
Type: dirifempty; Name: "{autoprograms}\Jupiter"

[Icons]
Name: "{autoprograms}\Chess Engine Compiler"; Filename: "{app}\ChessEngineCompiler.exe"; WorkingDir: "{app}"; IconFilename: "{app}\Assets\app.ico"; AppUserModelID: "Jupiter.ChessEngineCompiler"

[Run]
Filename: "{app}\ChessEngineCompiler.exe"; Description: "Open Chess Engine Compiler"; Flags: nowait postinstall skipifsilent

; Stable AppId upgrades the same per-user installation. No user-data directories are
; installed or deleted: %LOCALAPPDATA%\Jupiter\ChessEngineCompiler and engine outputs
; survive both upgrade and uninstall. A running app must be closed before replacement.
