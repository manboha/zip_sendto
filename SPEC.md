# ZIP Send To Tool — SPEC

## 1. 개요

Windows 탐색기의 "보내기(Send To)" 메뉴에서 동작하는, ZIP 포맷 전용의 매우 가벼운 압축/압축 해제 도구.

- 대상 포맷: **ZIP만** (다른 포맷 미지원)
- 비밀번호/암호화 기능: **제외** (호환성 우선 원칙에 따라 스코프 외)
- 실행 방식: Send To를 통한 독립 프로세스 실행 (방법 A). COM 셸 확장(방법 B)은 향후 확장 옵션으로만 고려
- 우선순위: **속도 > 압축률**. 압축률 개선은 속도를 해치지 않는 범위 내에서만 적용

---

## 2. 핵심 원칙

1. **속도 우선**: zlib-ng 기반 SIMD 가속 Deflate를 사용해 압축/해제 속도를 확보
2. **압축률은 부차적**: 1단계에서 확보한 속도 여유분을 압축 레벨 상향으로 되돌림 (파일 크기별 차등 레벨)
3. **호환성 우선**: 표준 ZIP/Deflate만 사용. Deflate64, 암호화(ZipCrypto/AES) 등 비표준/제한적 호환 옵션은 사용하지 않음
4. **설정값은 하드코딩**: 압축 레벨, STORE 대상 확장자 등은 설정파일이 아닌 `config.rs`의 상수로 관리 (런타임 I/O 제거)
5. **panic 금지**: 코어 함수(`compress`, `extract`)는 모든 실패를 `Result<_, AppError>`로 흡수
6. **성공은 무음, 예외는 알림**: 정상 동작 시 별도 알림 없음. 자동 리네임 등 예외적 상황 발생 시에만 메시지박스로 알림

---

## 3. 메뉴 구성 (Send To)

`shell:sendto\` 에 4개의 바로가기를 직접 배치하여 탐색기 "보내기" 메뉴에 표시.

> **설계 확정**: Windows 탐색기는 `shell:sendto\` 하위 폴더를 캐스케이드 서브메뉴로 표시하지 않는다(실 환경 검증 완료). 따라서 `shell:sendto\` 에 바로가기를 직접 배치하고 이름에 `ZIP - ` 접두사를 붙여 그룹을 구분하는 평면 메뉴 방식을 채택한다.

정렬 순서를 고려한 표시 이름(가나다 정렬상 "ZIP - 압축" 그룹이 "ZIP - 압축풀기" 그룹보다 먼저 오도록):

| 표시 이름 | 동작 | `--mode` 인자 |
|---|---|---|
| ZIP - 압축 (폴더명) | 선택 항목들을 zip 루트에 담고, 상위 폴더 이름으로 `.zip` 생성 | `compress-by-parent` |
| ZIP - 압축 (항목명) | 선택 항목들을 zip 루트에 담고, 첫 번째 선택 항목 이름으로 `.zip` 생성 | `compress-by-name` |
| ZIP - 압축풀기 (여기에) | 현재 폴더에 바로 압축 해제 (충돌 시 자동 리네임) | `extract-here` |
| ZIP - 압축풀기 (새 폴더) | `zip파일명/` 하위 폴더를 만들고 그 안에 압축 해제 | `extract-subdir` |

각 항목은 동일한 두 코어 함수(`compress`, `extract`)를 호출하며, 차이는 출력 경로 계산 방식뿐.

---

## 4. 압축 규칙

### 4.1 내부 구조
- 선택된 모든 항목(파일/폴더, 1개 이상, 혼합 가능)을 zip **루트**에 그대로 담음
- 폴더 선택 시 폴더 자체(이름 포함)가 zip 루트의 한 엔트리가 됨
- 선택 개수와 zip 내부 구조는 무관 — 항상 "선택한 그대로 루트에 담기"

### 4.2 출력 파일명 (사용자가 메뉴로 선택)

- **폴더명 모드**: 선택된 항목들의 공통 부모 디렉토리 이름 → `상위폴더명.zip`
- **항목명 모드**: argv 기준 첫 번째 선택 항목의 이름으로 `.zip` 생성
  - 첫 번째 항목이 **파일**이면 `Path::file_stem()`(확장자 제외 이름) 사용
  - 첫 번째 항목이 **폴더**이면 `Path::file_name()`(폴더 전체 이름) 사용 — 예: `my.folder` → `my.folder.zip`
- 출력 위치: 선택된 항목들의 부모 디렉토리
- **공통 부모가 드라이브 루트**(`C:\` 등)인 경우: `Path::file_name()`이 None을 반환하므로 드라이브 문자(`C`)를 파일명으로 사용 — 예: `C.zip`

### 4.3 출력 파일 충돌
- 출력 zip 파일이 이미 존재하면 `파일명 (1).zip`, `파일명 (2).zip` 순으로 자동 리네임하여 빈 번호를 사용
- 1부터 999까지 시도 후에도 모두 존재하면 `AppError`로 처리

### 4.4 빈 폴더
- 빈 폴더도 `이름/` 형태의 디렉토리 엔트리(데이터 크기 0)로 zip에 명시적으로 기록하여 보존

### 4.5 압축 실패 시 롤백
- `compress()` 처리 중 `AppError` 발생 시, 생성 중인 `.zip` 파일을 삭제한 뒤 오류를 반환
- 삭제 자체가 실패하더라도 원래 오류를 우선 반환 (삭제 실패는 무시)

---

## 5. 압축 해제 규칙

### 5.1 두 가지 모드 (사용자가 메뉴로 선택, zip 내부 구조와 무관)
- **여기에 풀기**: 현재 폴더(zip 파일이 위치한 폴더)에 바로 압축 해제
- **새 폴더에 풀기**: `zip파일명(확장자 제외)/` 폴더를 생성하고 그 안에 압축 해제

### 5.2 충돌 처리 — 자동 리네임
두 모드 모두 충돌 시 자동 리네임을 적용한다.

**리네임 형식**:
- 파일: `이름 (N).ext` — 파일명의 마지막 확장자 앞에 ` (N)` 삽입 (예: `report (1).txt`)
- 폴더 또는 확장자 없는 파일: `이름 (N)` (예: `MyFolder (1)`)
- N은 1부터 순차 증가, 999 초과 시 `AppError`로 처리

**"여기에 풀기"**: zip 내부 엔트리와 동일한 이름의 파일/폴더가 대상 위치에 이미 존재하면 자동 리네임.

**"새 폴더에 풀기"**: 대상 폴더(`zip파일명/`)가 이미 존재하면 폴더명을 자동 리네임(`zip파일명 (1)/` 등)하여 항상 새 폴더를 생성한다.

리네임이 1건 이상 발생하면 `ExtractReport.renamed`에 기록하고, 완료 후 메시지박스로 사용자에게 알림.

### 5.3 다중 zip 선택
- argv에 여러 zip 파일이 포함된 경우, 각 zip마다 독립적으로 `extract()` 호출
- `.zip` 확장자가 아닌 파일이 argv에 포함된 경우, 해당 항목은 실패로 기록하고 나머지 zip 파일은 계속 처리
- 일부 zip 처리 실패 시에도 나머지는 계속 처리. 모든 처리 완료 후 실패 목록을 한 번에 메시지박스로 표시

### 5.4 보안 — Zip Slip 방어
- 각 엔트리 경로를 target 디렉토리에 join한 결과가 target 디렉토리 하위에 있는지 검증
- 벗어나는 경로(`../` 등 포함)는 처리 거부 및 `AppError::UnsafePath`로 보고

---

## 6. 경로 처리 (압축 시)

- 재귀 순회: `walkdir` 사용, `follow_links(false)` (심볼릭 링크/정션 내부로 진입하지 않음)
- 상대경로 기준: 선택 항목들의 공통 부모 디렉토리(base)
- 경로 구분자: ZIP 표준에 맞춰 `/` 사용 (Windows `\`는 컴포넌트 단위 변환)
- 파일명 인코딩: Rust `zip` 크레이트의 기본 UTF-8 플래그(general purpose bit 11) 사용. 별도 인코딩 처리 불필요

### 6.1 스킵 대상 (압축 중단 없이 건너뛰고 기록)
- 심볼릭 링크 / 디렉토리 정션 → `SkipReason::SymlinkOrJunction`
- 접근 권한 오류 → `SkipReason::PermissionDenied`
- `CompressReport.skipped: Vec<(PathBuf, SkipReason)>`에 누적, 완료 후 1건 이상이면 알림

---

## 7. 코어 함수 인터페이스

```rust
pub struct CompressReport {
    pub output_path: PathBuf,
    pub entry_count: usize,
    pub skipped: Vec<(PathBuf, SkipReason)>,
}

pub struct ExtractReport {
    pub target_dir: PathBuf,
    pub renamed: Vec<(String, String)>, // zip 내부 전체 경로 기준 (원래 경로, 변경된 경로)
                                        // 예: ("dir/report.txt", "dir/report (1).txt")
}

pub enum SkipReason {
    SymlinkOrJunction,
    PermissionDenied,
}

pub fn compress(items: &[PathBuf], output_path: &Path) -> Result<CompressReport, AppError>;
pub fn extract(zip_path: &Path, target_dir: &Path) -> Result<ExtractReport, AppError>;
```

### 7.1 에러 타입 (`thiserror` 사용)

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("선택한 항목이 없습니다")]
    NoInput,

    #[error("입출력 오류: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP 처리 오류: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("'{0}' 경로를 찾을 수 없습니다")]
    InvalidPath(PathBuf),

    #[error("리네임 번호가 상한(999)을 초과했습니다: '{0}'")]
    RenameLimit(PathBuf),

    #[error("'{0}' 경로가 압축 해제 대상 폴더를 벗어납니다 (Zip Slip)")]
    UnsafePath(String),
}
```

> `AppError::OutputExists`는 삭제됨 — 압축 출력 충돌은 자동 리네임으로 처리하므로 오류 variant 불필요.

---

## 8. 사용자 피드백

- 콘솔 출력 사용 안 함 (Send To 실행 환경에서 도달하지 않음)
- `windows` 크레이트의 `MessageBoxW`로 팝업 표시
- **MessageBox 창 제목**: `ZIP 도구`
- **표시 시점**:
  - 에러 발생 시 (항상)
  - 자동 리네임 발생 시 (`ExtractReport.renamed`가 비어있지 않을 때)
  - 항목 스킵 발생 시 (`CompressReport.skipped`가 비어있지 않을 때)
  - 위 조건이 모두 없으면 무음 종료
- **메시지 형식** (확정):
  - 에러: `AppError::to_string()` 그대로 출력 (한글)
  - 리네임 알림: `"파일 이름 충돌로 다음 항목을 자동으로 이름 변경했습니다:\n  원래이름 → 새이름"` 형식
  - 스킵 알림: `"다음 항목은 건너뛰었습니다:\n  경로 (사유)"` 형식
  - 다중 실패: `"다음 파일을 처리하지 못했습니다:\n  경로: 오류내용"` 형식

---

## 9. 압축 성능 정책 (`config.rs` 상수)

- **압축 엔진**: `zip` 크레이트의 `deflate-zlib-ng` feature 활성화 (zlib-ng 기반, SIMD 가속 Deflate 제공. 별도 libdeflater 크레이트 불필요)
- 파일 크기 기준 압축 레벨 차등 (`config.rs` 상수로 확정):
  - **SIZE_THRESHOLD** = 1 MB (1,048,576 bytes)
  - 임계값 **이하** (작은 파일): **COMPRESS_LEVEL_HIGH = 6**
  - 임계값 **초과** (큰 파일): **COMPRESS_LEVEL_MED = 3**
- STORE(무압축) 대상 확장자 (`STORE_EXTENSIONS`, 확정):
  - 이미지: `jpg` `jpeg` `png` `gif` `webp` `avif` `heic` `heif`
  - 동영상: `mp4` `mov` `avi` `mkv` `webm` `m4v` `wmv`
  - 오디오: `mp3` `aac` `flac` `ogg` `m4a` `wma` `opus`
  - 아카이브: `zip` `gz` `bz2` `xz` `zst` `7z` `rar` `br`
  - 문서/오피스: `docx` `xlsx` `pptx` `odt` `ods` `odp` `epub`

---

## 10. 기술 스택

- 언어: Rust (edition 2021+)
- 주요 크레이트:
  - `zip` — ZIP 컨테이너 읽기/쓰기 (`deflate-zlib-ng` feature로 SIMD 가속 Deflate 활성화)
  - `walkdir` — 디렉토리 재귀 순회
  - `thiserror` — 에러 타입
  - `windows` — MessageBoxW 호출
- 빌드: 단일 정적 링크 exe, 런타임 의존성 없음

---

## 11. 셸 통합 (설치)

- `shell:sendto\` 에 바로가기 4개 직접 생성 (평면 메뉴 방식 — 3절 참고)
- 각 바로가기: 동일 exe + `--mode` 인자, 이름에 `ZIP - ` 접두사:
  - `ZIP - 압축 (폴더명).lnk` → `--mode compress-by-parent`
  - `ZIP - 압축 (항목명).lnk` → `--mode compress-by-name`
  - `ZIP - 압축풀기 (여기에).lnk` → `--mode extract-here`
  - `ZIP - 압축풀기 (새 폴더).lnk` → `--mode extract-subdir`
- 설치용 PowerShell 스크립트(`install.ps1`)로 일괄 생성
- 재실행 시 기존 바로가기 덮어쓰기(멱등 동작), `shell:sendto\ZIP\` 서브폴더가 존재하면 자동 삭제

---

## 12. 스코프 외 / 향후 고려사항

- 비밀번호/암호화 압축 — 제외 확정
- COM 셸 확장(IContextMenu/IExplorerCommand, 방법 B) — 향후 확장 옵션. 코어 로직(`compress`/`extract`)은 재사용 가능하도록 분리 유지
- 설정파일 — 제외 확정 (필요 시 `config.rs` 상수 수정 후 재빌드)

---

## 13. 테스트 전략

### 13.1 TDD 적용 — 코어 로직

다음 영역은 테스트를 먼저 작성하고, 이를 통과시키는 구현을 작성한다. 구현 중 테스트가 실패하면 **구현을 수정**하며, 테스트 자체를 완화/수정하지 않는다 (테스트 변경이 필요하다고 판단되면 별도로 보고 후 합의).

- `compress` / `extract` 핵심 동작
- 경로 처리(상대경로 계산, `/` 변환, UTF-8 파일명, 빈 폴더 보존)
- `AppError` 각 variant의 트리거 조건
- `CompressReport.skipped`, `ExtractReport.renamed` 채워지는 조건
- argv 파싱 → 모드(`Mode`) 변환 (순수 함수로 분리)
- 자동 리네임 로직 (압축 출력 충돌, 해제 충돌, 새 폴더 충돌)

**핵심 테스트 — 라운드트립(round-trip)**
SPEC 4~5절의 압축↔해제 대칭 설계를 검증하는 통합 테스트를 최우선으로 둔다.

```rust
#[test]
fn roundtrip_single_folder() {
    let tmp = setup_test_tree(); // 폴더 1개 + 하위 파일/빈 폴더 포함
    let report = compress(&[tmp.path().join("MyFolder")], &zip_path)?;
    extract(&zip_path, &extract_dir)?;
    assert_dir_trees_equal(tmp.path().join("MyFolder"), extract_dir.join("MyFolder"));
}
```

라운드트립 테스트는 최소 다음 케이스를 커버한다:
- 단일 파일 / 단일 폴더 / 다중 항목(파일+폴더 혼합)
- 빈 폴더 포함
- 한글(유니코드) 파일명/폴더명 포함
- 심볼릭 링크·정션 포함 (결과물에서 제외되고 `skipped`에 기록되는지)
- "여기에 풀기" 시 동일 이름 존재 → 자동 리네임 및 `renamed` 기록
- "새 폴더에 풀기" 시 대상 폴더 이미 존재 → 자동 리네임 후 새 폴더에 해제
- 압축 출력 파일 이미 존재 → 자동 리네임 후 압축 성공
- 악의적 경로(`../`)를 포함한 zip → `AppError::UnsafePath` 반환
- 리네임 번호 999 초과 → `AppError::RenameLimit` 반환

### 13.2 수동/체크리스트 기반 검증 — 셸·UI 레이어

자동 테스트로 검증하기 어렵거나 비용이 큰 영역은 수동 체크리스트로 검증한다.

- **Send To 설치(`install.ps1`)**: `shell:sendto\`에 `ZIP - ` 접두사 바로가기 4개 생성 여부, 탐색기 "보내기" 메뉴에 4개 항목으로 표시되는지, 각 바로가기가 올바른 `--mode` 인자로 exe를 호출하는지
- **MessageBoxW 알림**: 에러/리네임/스킵 각 케이스에서 실제 팝업이 표시되고 메시지가 올바른지
- **실제 탐색기 동작**: 4개 메뉴 항목으로 단일 파일, 단일 폴더, 다중 선택(파일+폴더 혼합), 대용량 폴더에 대해 실제 압축/해제 수행 후 결과 확인

`MessageBoxW` 호출은 얇은 래퍼(trait 등)로 분리하여, "어떤 조건에서 어떤 메시지를 호출하는가"라는 로직 자체는 13.1의 TDD 대상에 포함한다 (팝업 실제 표시만 수동 확인).

### 13.3 PLAN.md 마일스톤 매핑

| 마일스톤 | 검증 방식 | 완료 기준 |
|---|---|---|
| 코어 함수 + 경로 처리 | TDD | `cargo test` 전체 통과 |
| 에러 타입 / Report 구조 | TDD | `cargo test` 통과 |
| argv 파싱 / 모드 분기 | TDD | `cargo test` 통과 |
| MessageBoxW 래퍼 | 로직 TDD + 수동 1회 | 로직 테스트 통과 + 팝업 수동 확인 |
| Send To 설치 스크립트 | 체크리스트 | 13.2 체크리스트 전체 확인 |

---

## 14. 확정된 항목 (구현 완료)

- [x] "작은 파일" 임계값 → **1 MB** (9절 참고)
- [x] 압축 레벨 → **HIGH=6 / MED=3** (9절 참고)
- [x] STORE 무압축 대상 확장자 전체 목록 → 9절 참고
- [x] SendTo 서브메뉴 동작 검증 → **캐스케이드 미지원** 확인. 평면 메뉴 + `ZIP - ` 접두사 방식 채택 (3절, 11절 참고)
