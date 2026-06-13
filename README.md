# ZIP Send To

Windows 탐색기 **"보내기(Send To)"** 메뉴에 ZIP 압축·해제 기능을 추가하는 초경량 도구입니다.

- **속도 우선** — zlib-ng 기반 SIMD 가속 Deflate
- **단일 exe** — 런타임 의존성 없음, 638 KB
- **무음 성공** — 정상 동작 시 팝업 없음. 충돌·스킵 등 예외 상황에만 알림

---

## 사용 방법

설치 후 탐색기에서 파일이나 폴더를 선택하고 **우클릭 → 보내기** 를 열면 아래 항목이 나타납니다.

| 메뉴 항목 | 동작 |
|-----------|------|
| **ZIP - 압축 (폴더명)** | 선택 항목의 상위 폴더 이름으로 `.zip` 생성 |
| **ZIP - 압축 (항목명)** | 첫 번째 선택 항목의 이름으로 `.zip` 생성 |
| **ZIP - 압축풀기 (여기에)** | zip 파일과 같은 폴더에 바로 압축 해제 |
| **ZIP - 압축풀기 (새 폴더)** | zip 파일 이름의 새 폴더를 만들고 그 안에 압축 해제 |

### 압축 시 알아두면 좋은 점

- 파일, 폴더, 혼합 다중 선택 모두 지원합니다.
- 같은 이름의 zip이 이미 있으면 `파일명 (1).zip`, `파일명 (2).zip` 순으로 자동으로 이름을 바꿉니다 (덮어쓰기 없음).
- jpg, png, mp4, mp3, zip 등 이미 압축된 포맷은 STORE(무압축)로 저장해 불필요한 재압축을 방지합니다.
- 심볼릭 링크·정션은 건너뛰고 완료 후 목록을 알려줍니다.

### 압축 해제 시 알아두면 좋은 점

- 여러 zip 파일을 한꺼번에 선택해 한 번에 해제할 수 있습니다.
- 같은 이름의 파일이 있으면 `파일명 (1).txt` 형식으로 자동으로 이름을 바꾸고 완료 후 알려줍니다.
- 경로 탈출 공격(Zip Slip)을 자동으로 차단합니다.

---

## 설치

### 일반 사용자 — 인스톨러 실행

1. [Releases](../../releases) 페이지에서 `ZIP_SendTo_Setup.exe` 를 내려받습니다.
2. 더블클릭해서 실행합니다. **관리자 권한(UAC) 팝업이 뜨지 않습니다.**
3. "다음 → 설치 → 완료" 를 클릭하면 끝납니다.

설치 직후 탐색기가 자동으로 재시작되고, 보내기 메뉴에 항목 4개가 바로 나타납니다.

> 설치 위치: `%LOCALAPPDATA%\Programs\ZIP SendTo\`

### 개발자 — 소스에서 직접 빌드

**빌드 요구 사항**

| 도구 | 설치 방법 |
|------|-----------|
| Rust 1.75 이상 | https://rustup.rs |
| MSVC (C 컴파일러) | [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) → "C++ 빌드 도구" 선택 |
| CMake 3.x 이상 | `winget install Kitware.CMake` |

**빌드 및 설치**

```powershell
git clone <이 저장소>
cd zip_sendto

# exe 빌드
cargo build --release

# 인스톨러(Setup.exe) 생성 — Inno Setup 6 필요
# winget install JRSoftware.InnoSetup
& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" installer\setup.iss
# → dist\ZIP_SendTo_Setup.exe 생성

# 또는 현재 PC에만 바로 설치하려면
Copy-Item .\target\release\zip_sendto.exe .\
.\install.ps1
```

---

## 제거

**인스톨러로 설치한 경우**

Windows 설정 → 앱 → "ZIP Send To" → 제거

**`install.ps1` 로 설치한 경우**

```powershell
Remove-Item "$env:APPDATA\Microsoft\Windows\SendTo\ZIP - *.lnk" -Force
```

---

## CLI 직접 실행 (개발자용)

탐색기 없이 터미널에서 동작을 확인할 수 있습니다.

```powershell
# 파일 압축 (항목명 모드)
.\zip_sendto.exe --mode compress-by-name "C:\작업폴더\보고서.docx"
# → C:\작업폴더\보고서.zip 생성

# 폴더 압축 (폴더명 모드)
.\zip_sendto.exe --mode compress-by-parent "C:\작업폴더\MyProject"
# → C:\작업폴더\작업폴더.zip 생성

# 압축 해제 (여기에)
.\zip_sendto.exe --mode extract-here "C:\Downloads\archive.zip"
# → C:\Downloads\ 에 바로 해제

# 압축 해제 (새 폴더)
.\zip_sendto.exe --mode extract-subdir "C:\Downloads\archive.zip"
# → C:\Downloads\archive\ 폴더 안에 해제

# 여러 파일 한 번에 압축
.\zip_sendto.exe --mode compress-by-name "C:\data\a.txt" "C:\data\b.txt" "C:\data\folder"
```

---

## 제한 사항

- **ZIP 포맷만 지원** — 7z, RAR, tar 등 다른 포맷은 읽기/쓰기 모두 불가
- **암호화·비밀번호 없음** — 호환성을 위해 의도적으로 제외
- **심볼릭 링크·정션** — 압축 시 따라가지 않고 건너뜀
- **리네임 상한** — 동일 이름이 999개 이상이면 오류 알림

---

## 라이선스

MIT
