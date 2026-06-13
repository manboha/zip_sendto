; ZIP Send To — Inno Setup 스크립트
; 빌드: ISCC.exe installer\setup.iss
; 출력: dist\ZIP_SendTo_Setup.exe

#define AppName    "ZIP Send To"
#define AppVersion "0.1.0"
#define AppExe     "zip_sendto.exe"
#define AppURL     "https://github.com/"

; ── 기본 설정 ─────────────────────────────────────────────────────────────────

[Setup]
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisherURL={#AppURL}

; 관리자 권한 없이 현재 사용자 AppData에 설치
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\ZIP SendTo
DisableDirPage=yes
DisableProgramGroupPage=yes

; 출력
OutputDir=..\dist
OutputBaseFilename=ZIP_SendTo_Setup
SetupIconFile=
Compression=lzma2
SolidCompression=yes

; 제거
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\{#AppExe}

; 64비트 전용
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible

; 언어 자동 선택 (대화상자 표시 안 함)
ShowLanguageDialog=no

; ── 언어 ──────────────────────────────────────────────────────────────────────

[Languages]
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

; ── 복사할 파일 ───────────────────────────────────────────────────────────────

[Files]
; release 빌드 exe를 설치 폴더로 복사
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

; ── 보내기 메뉴 바로가기 4개 ──────────────────────────────────────────────────

[Icons]
Name: "{usersendto}\ZIP - 압축 (폴더명)";     Filename: "{app}\{#AppExe}"; Parameters: "--mode compress-by-parent"; Comment: "선택 항목을 상위 폴더 이름으로 ZIP 압축"
Name: "{usersendto}\ZIP - 압축 (항목명)";     Filename: "{app}\{#AppExe}"; Parameters: "--mode compress-by-name";   Comment: "선택 항목 이름으로 ZIP 압축"
Name: "{usersendto}\ZIP - 압축풀기 (여기에)"; Filename: "{app}\{#AppExe}"; Parameters: "--mode extract-here";       Comment: "현재 폴더에 바로 ZIP 압축 해제"
Name: "{usersendto}\ZIP - 압축풀기 (새 폴더)"; Filename: "{app}\{#AppExe}"; Parameters: "--mode extract-subdir";   Comment: "새 폴더를 만들고 ZIP 압축 해제"

; ── Pascal 스크립트 ───────────────────────────────────────────────────────────

[Code]

{ 탐색기 재시작 — SendTo 변경이 즉시 반영되도록 }
procedure RestartExplorer;
var
  ResultCode: Integer;
begin
  Exec('powershell.exe',
    '-NonInteractive -Command "Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue"',
    '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    { 이전 서브폴더 방식 잔재 제거 (있을 경우) }
    DelTree(ExpandConstant('{usersendto}\ZIP'), True, True, True);
    RestartExplorer;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RestartExplorer;
end;
