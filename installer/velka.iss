; Installateur de Velka : sans droits administrateur, raccourcis Bureau et Menu Démarrer.
; Compilation locale :  ISCC.exe installer\velka.iss   (après `cargo build --release`)

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif
#ifndef ExePath
  #define ExePath "..\target\release\velka.exe"
#endif

[Setup]
AppId={{9DE056F7-528B-4258-8911-B0EEA4C2EFF3}
AppName=Velka
AppVersion={#AppVersion}
AppPublisher=Velka
DefaultDirName={autopf}\Velka
DefaultGroupName=Velka
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=Output
OutputBaseFilename=velka-setup
SetupIconFile=..\assets\velka.ico
UninstallDisplayIcon={app}\velka.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#ExePath}"; DestDir: "{app}"; Flags: ignoreversion
; Notices des composants tiers, générées par tools\notices\generer.py : licences exigées par les bibliothèques.
Source: "THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
; Licence de Velka (GPL-3.0 ou ultérieure).
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
; L'identifiant d'application donne le nom « Velka » et le logo aux notifications Windows.
Name: "{autoprograms}\Velka"; Filename: "{app}\velka.exe"; AppUserModelID: "Velka.App"
Name: "{autodesktop}\Velka"; Filename: "{app}\velka.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\velka.exe"; Description: "{cm:LaunchProgram,Velka}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Les outils téléchargés, les réglages et le cache sont supprimés avec l'application.
Type: filesandordirs; Name: "{localappdata}\Velka"

[Code]
// Le démarrage avec Windows est une valeur de la clé « Run » posée par l'application (pas par l'installateur) :
// on la retire à la désinstallation pour que Windows ne cherche plus un programme absent.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Velka');
end;
