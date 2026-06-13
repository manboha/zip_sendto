#Requires -Version 5.1
<#
.SYNOPSIS
    ZIP Send To 도구를 Windows "보내기" 메뉴에 설치합니다.

.DESCRIPTION
    shell:sendto\ 에 "ZIP - " 접두사 바로가기 4개를 생성합니다.
    탐색기 "보내기" 메뉴에 "ZIP - 압축 (폴더명)" 형식으로 표시됩니다.

.EXAMPLE
    .\install.ps1
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── exe 위치 확인 ─────────────────────────────────────────────────────────────

$exe = Join-Path $PSScriptRoot "zip_sendto.exe"
if (-not (Test-Path $exe)) {
    Write-Error "zip_sendto.exe 를 찾을 수 없습니다: $exe`n먼저 'cargo build --release' 를 실행하고 exe 를 이 스크립트와 같은 폴더에 놓으세요."
    exit 1
}
$exe = (Resolve-Path $exe).Path   # 절대 경로로 정규화

$sendTo = [Environment]::GetFolderPath("SendTo")

# ── 이전 서브폴더 방식 잔재 정리 ──────────────────────────────────────────────

$oldDir = Join-Path $sendTo "ZIP"
if (Test-Path $oldDir) {
    Remove-Item $oldDir -Recurse -Force
    Write-Host "  제거: $oldDir (이전 설치 정리)"
}

# ── 바로가기 정의 (평면 메뉴, "ZIP - " 접두사) ────────────────────────────────

$menus = @(
    [ordered]@{ Name = "ZIP - 압축 (폴더명)";    Mode = "compress-by-parent" },
    [ordered]@{ Name = "ZIP - 압축 (항목명)";    Mode = "compress-by-name"   },
    [ordered]@{ Name = "ZIP - 압축풀기 (여기에)"; Mode = "extract-here"       },
    [ordered]@{ Name = "ZIP - 압축풀기 (새 폴더)"; Mode = "extract-subdir"    }
)

# ── 바로가기 생성 ─────────────────────────────────────────────────────────────

$wsh = New-Object -ComObject WScript.Shell
foreach ($m in $menus) {
    $lnkPath             = Join-Path $sendTo "$($m.Name).lnk"
    $lnk                 = $wsh.CreateShortcut($lnkPath)
    $lnk.TargetPath      = $exe
    $lnk.Arguments       = "--mode $($m.Mode)"
    $lnk.Description     = $m.Name
    $lnk.WorkingDirectory = Split-Path $exe
    $lnk.Save()
    Write-Host "  생성: $lnkPath"
}

Write-Host ""
Write-Host "설치 완료: $sendTo"
Write-Host "탐색기를 재시작하거나 로그아웃/로그인 후 '보내기' 메뉴를 확인하세요."

# =============================================================================
# 참고: 캐스케이드 서브메뉴 방식 (shell:sendto\ZIP\ 하위 폴더)
# -----------------------------------------------------------------------------
# Windows 탐색기는 SendTo 하위 폴더를 캐스케이드 서브메뉴로 표시하지 않으므로
# 이 프로젝트는 평면 메뉴 + "ZIP - " 접두사 방식을 채택합니다.
# =============================================================================
