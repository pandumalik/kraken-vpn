; Kraken VPN Windows Installer (Inno Setup 6)
; Builds a standard Windows installer that checks for OpenVPN and OpenConnect-GUI
; and optionally downloads & runs their official installers during setup.

#ifndef MyAppVersion
  #define MyAppVersion "2.0.0"
#endif

#define MyAppName "Kraken VPN"
#define MyAppPublisher "Pandu Malik"
#define MyAppURL "https://github.com/pandumalik/kraken-vpn"
#define MyAppExeName "kraken-vpn.exe"

; Official upstream dependency installers (Windows x64)
#define OpenVpnInstallerUrl "https://swupdate.openvpn.org/community/releases/OpenVPN-2.7.9-I002-amd64.msi"
#define OpenConnectInstallerUrl "https://www.infradead.org/openconnect-gui/download/openconnect-gui-1.6.2-win64.exe"

[Setup]
AppId={{8C9E5B2A-4D1F-4F7A-9B3C-6E2A1D8F4C09}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\LICENSE
OutputDir=..\release-artifacts
OutputBaseFilename=KrakenVPN-Setup-{#MyAppVersion}-windows-x86_64
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\{#MyAppExeName}
#if FileExists("kraken-vpn.ico")
SetupIconFile=kraken-vpn.ico
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\sample.ovpn"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
; Run downloaded dependency installers if user opted in and download succeeded
Filename: "msiexec.exe"; Parameters: "/i ""{tmp}\openvpn-installer.msi"" /passive"; StatusMsg: "Installing OpenVPN Community..."; Flags: waituntilterminated; Check: ShouldRunOpenVpnInstaller
Filename: "{tmp}\openconnect-gui-installer.exe"; StatusMsg: "Launching OpenConnect-GUI setup..."; Flags: waituntilterminated; Check: ShouldRunOpenConnectInstaller
; Launch Kraken VPN after setup completes
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
var
  DepsPage: TInputOptionWizardPage;
  DownloadPage: TDownloadWizardPage;
  HasOpenVpn: Boolean;
  HasOpenConnect: Boolean;
  OpenVpnDownloaded: Boolean;
  OpenConnectDownloaded: Boolean;

function IsOpenVpnInstalled: Boolean;
begin
  Result :=
    FileExists(ExpandConstant('{commonpf}\OpenVPN\bin\openvpn.exe')) or
    FileExists(ExpandConstant('{commonpf32}\OpenVPN\bin\openvpn.exe'));
end;

function IsOpenConnectInstalled: Boolean;
begin
  Result :=
    FileExists(ExpandConstant('{commonpf}\OpenConnect-GUI\openconnect.exe')) or
    FileExists(ExpandConstant('{commonpf32}\OpenConnect-GUI\openconnect.exe'));
end;

function OnDownloadProgress(const Url, FileName: String; const Progress, ProgressMax: Int64): Boolean;
begin
  if Progress = ProgressMax then
    Log(Format('Successfully downloaded %s', [FileName]));
  Result := True;
end;

procedure InitializeWizard;
begin
  HasOpenVpn := IsOpenVpnInstalled;
  HasOpenConnect := IsOpenConnectInstalled;
  OpenVpnDownloaded := False;
  OpenConnectDownloaded := False;

  DepsPage := CreateInputOptionPage(
    wpSelectTasks,
    'VPN Engine Dependencies',
    'Kraken VPN orchestrates OpenVPN and OpenConnect CLI engines on your PC.',
    'Select any missing VPN engines you want Setup to download and install automatically. You can also configure or install them later from Settings.',
    False,
    False
  );

  if HasOpenVpn then
  begin
    DepsPage.Add('OpenVPN Community — already installed (skip)');
    DepsPage.Values[0] := False;
    DepsPage.CheckListBox.ItemEnabled[0] := False;
  end
  else
  begin
    DepsPage.Add('Download & install OpenVPN Community (required for .ovpn profiles)');
    DepsPage.Values[0] := True;
  end;

  if HasOpenConnect then
  begin
    DepsPage.Add('OpenConnect-GUI — already installed (skip)');
    DepsPage.Values[1] := False;
    DepsPage.CheckListBox.ItemEnabled[1] := False;
  end
  else
  begin
    DepsPage.Add('Download & install OpenConnect-GUI (required for FortiGate / AnyConnect / GlobalProtect)');
    DepsPage.Values[1] := True;
  end;

  DownloadPage := CreateDownloadPage(
    SetupMessage(msgWizardPreparing),
    SetupMessage(msgPreparingDesc),
    @OnDownloadProgress
  );
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var
  WantOpenVpn, WantOpenConnect: Boolean;
begin
  Result := True;
  if CurPageID = wpReady then
  begin
    WantOpenVpn := (not HasOpenVpn) and DepsPage.Values[0];
    WantOpenConnect := (not HasOpenConnect) and DepsPage.Values[1];

    if WantOpenVpn or WantOpenConnect then
    begin
      DownloadPage.Clear;
      if WantOpenVpn then
        DownloadPage.Add('{#OpenVpnInstallerUrl}', 'openvpn-installer.msi', '');
      if WantOpenConnect then
        DownloadPage.Add('{#OpenConnectInstallerUrl}', 'openconnect-gui-installer.exe', '');

      DownloadPage.Show;
      try
        try
          DownloadPage.Download;
          OpenVpnDownloaded := WantOpenVpn and FileExists(ExpandConstant('{tmp}\openvpn-installer.msi'));
          OpenConnectDownloaded := WantOpenConnect and FileExists(ExpandConstant('{tmp}\openconnect-gui-installer.exe'));
        except
          if DownloadPage.AbortedByUser then
          begin
            Log('Dependency download cancelled by user.');
            Result := False;
          end
          else
          begin
            SuppressibleMsgBox(
              'Could not download one or more VPN engine installers (' + GetExceptionMessage + ').' + #13#10 + #13#10 +
              'Kraken VPN will still be installed, and you can download OpenVPN or OpenConnect later from Settings.',
              mbInformation, MB_OK, IDOK
            );
            OpenVpnDownloaded := WantOpenVpn and FileExists(ExpandConstant('{tmp}\openvpn-installer.msi'));
            OpenConnectDownloaded := WantOpenConnect and FileExists(ExpandConstant('{tmp}\openconnect-gui-installer.exe'));
          end;
        end;
      finally
        DownloadPage.Hide;
      end;
    end;
  end;
end;

function ShouldRunOpenVpnInstaller: Boolean;
begin
  Result := OpenVpnDownloaded and FileExists(ExpandConstant('{tmp}\openvpn-installer.msi'));
end;

function ShouldRunOpenConnectInstaller: Boolean;
begin
  Result := OpenConnectDownloaded and FileExists(ExpandConstant('{tmp}\openconnect-gui-installer.exe'));
end;
