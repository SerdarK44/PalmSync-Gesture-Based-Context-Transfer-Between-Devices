; PalmSync Inno Setup Installer Script
; Requires Inno Setup 6.0+

#define MyAppName "PalmSync"
; The version comes from the VERSION file via build.ps1 (/DMyAppVersion=x.y.z)
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#define MyAppPublisher "PalmSync Contributors"
#define MyAppURL "https://github.com/palmsync"
#define DotNetRuntimeExe "windowsdesktop-runtime-8.0-win-x64.exe"

[Setup]
AppId={{8A3F7C2D-B5E1-4D9A-A6C8-7F2E1B3D5A9C}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
OutputDir=..\dist
OutputBaseFilename=PalmSync-Setup-{#MyAppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
SetupIconFile=..\ui\Assets\palmsync.ico
UninstallDisplayIcon={app}\ui\palmsync-ui.exe
VersionInfoVersion={#MyAppVersion}
; In-app updates run this installer with /VERYSILENT: PalmSync closes itself
; first; Restart Manager handles anything still holding a file.
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"

[Files]
; Bundled .NET 8 Desktop Runtime — extracted to temp and run silently only if missing
Source: "redist\{#DotNetRuntimeExe}"; DestDir: "{tmp}"; \
    Flags: deleteafterinstall; Check: NeedsDotNet
Source: "..\build\palmsync-daemon.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\build\palmsync-eye.exe";    DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\build\gesture_sim.exe";     DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\build\*.dll";               DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\build\*.xml";               DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\build\ui\*";                DestDir: "{app}\ui"; Flags: ignoreversion recursesubdirs
Source: "..\build\LICENSE";             DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\SETUP.md";                  DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "PalmSync-PublicCert.cer";      DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "trust-cert.ps1";             DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\ui\palmsync-ui.exe"
Name: "{group}\PalmSync Setup Guide"; Filename: "{app}\SETUP.md"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\PalmSync"; Filename: "{app}\ui\palmsync-ui.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"

[Run]
; Install the .NET 8 Desktop Runtime silently if it isn't already present
Filename: "{tmp}\{#DotNetRuntimeExe}"; Parameters: "/install /quiet /norestart"; \
    StatusMsg: "Installing .NET 8 Desktop Runtime (one time)..."; \
    Flags: waituntilterminated; Check: NeedsDotNet

; Trust the self-signed certificate so WDAC / SmartScreen allows our binaries.
; Must run before the postinstall launch of palmsync-ui.exe.
; The installer already runs as admin (PrivilegesRequired=admin), so this
; Import-Certificate call has the elevation it needs.
Filename: "powershell.exe"; \
    Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\trust-cert.ps1"""; \
    StatusMsg: "Trusting PalmSync certificate..."; \
    Flags: runhidden waituntilterminated

; Open the firewall for the host listener (only matters on the machine that hosts).
; Program rule is the most reliable; port rule is a belt-and-suspenders backup.
Filename: "netsh"; \
    Parameters: "advfirewall firewall delete rule name=""PalmSync"""; \
    Flags: runhidden waituntilterminated
Filename: "netsh"; \
    Parameters: "advfirewall firewall delete rule name=""PalmSync (port)"""; \
    Flags: runhidden waituntilterminated
Filename: "netsh"; \
    Parameters: "advfirewall firewall add rule name=""PalmSync"" dir=in action=allow program=""{app}\palmsync-daemon.exe"" enable=yes profile=any"; \
    Flags: runhidden waituntilterminated
Filename: "netsh"; \
    Parameters: "advfirewall firewall add rule name=""PalmSync (port)"" dir=in action=allow protocol=TCP localport=9001 profile=any"; \
    Flags: runhidden waituntilterminated

; Launch the tray app right after install
Filename: "{app}\ui\palmsync-ui.exe"; Description: "Launch PalmSync"; \
    Flags: nowait postinstall skipifsilent

; In-app update (silent install): start PalmSync again as the normal user,
; it resumes the previous session and says "updated to x.y.z".
Filename: "{app}\ui\palmsync-ui.exe"; Parameters: "--updated"; \
    Flags: nowait runasoriginaluser; Check: WizardSilent

[UninstallRun]
Filename: "netsh"; \
    Parameters: "advfirewall firewall delete rule name=""PalmSync"""; \
    Flags: runhidden waituntilterminated; RunOnceId: "DeleteFwRule"
Filename: "netsh"; \
    Parameters: "advfirewall firewall delete rule name=""PalmSync (port)"""; \
    Flags: runhidden waituntilterminated; RunOnceId: "DeleteFwPortRule"

[Registry]
; Start the tray app at each user's login (it manages the daemon in-session)
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueName: "PalmSync"; ValueType: string; \
    ValueData: """{app}\ui\palmsync-ui.exe"" --silent"; Flags: uninsdeletevalue

Root: HKLM; Subkey: "SOFTWARE\PalmSync"; ValueName: "Version"; \
    ValueType: string; ValueData: "{#MyAppVersion}"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\PalmSync"; ValueName: "InstallDir"; \
    ValueType: string; ValueData: "{app}"; Flags: uninsdeletekey

[Code]
{ Detect .NET 8 Desktop Runtime by probing for the shared framework folder. }
function DotNetDesktop8Present: Boolean;
var
  FindRec: TFindRec;
  Base: String;
begin
  Result := False;
  Base := ExpandConstant('{commonpf}\dotnet\shared\Microsoft.WindowsDesktop.App');
  if not DirExists(Base) then
    Exit;
  if FindFirst(Base + '\8.*', FindRec) then
  begin
    try
      repeat
        if (FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
          Result := True;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
end;

{ Stop PalmSync before files are replaced (in-app update or reinstall while
  running). The tray app normally exits itself; this is the safety net. }
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM palmsync-ui.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM palmsync-eye.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM palmsync-daemon.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := '';
end;

{ True when the bundled runtime must be installed. Used by both the [Files]
  extraction and the [Run] silent install so we only touch the disk when needed. }
function NeedsDotNet: Boolean;
begin
  Result := not DotNetDesktop8Present;
end;
