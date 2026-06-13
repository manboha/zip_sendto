# ZIP Send To Tool — PLAN


## 1. 프로젝트 구조

```
zip_sendto/
├── Cargo.toml
├── Cargo.lock          ← 커밋에 포함 (재현 가능 빌드)
├── install.ps1
└── src/
    ├── main.rs         진입점: parse → 경로 계산 → 코어 호출 → MessageBox
    ├── config.rs       압축 레벨, 파일 크기 임계값, STORE 확장자 상수
    ├── error.rs        AppError, SkipReason
    ├── args.rs         Mode enum, parse_args()
    ├── path.rs         순수 함수: 공통 부모, zip 내부 경로, 출력명, 충돌 리네임
    ├── compress.rs     compress() + CompressReport
    ├── extract.rs      extract() + ExtractReport
    └── ui.rs           MessageSink trait, 메시지 빌더, WindowsMessageBox
```

### 모듈별 역할

| 모듈 | 역할 | 외부 의존 |
|---|---|---|
| `config.rs` | 상수 전용 — 런타임 I/O 없음 | 없음 |
| `error.rs` | `AppError`, `SkipReason` | `thiserror`, `zip` |
| `args.rs` | `--mode` 파싱 → `Mode` enum, 파일 경로 수집 | `error.rs` |
| `path.rs` | 순수 함수 (fs 읽기만 허용, 쓰기 없음) | `error.rs`, `args.rs` |
| `compress.rs` | walkdir 순회 + ZipWriter | `path.rs`, `config.rs`, `error.rs` |
| `extract.rs` | ZipArchive 읽기 + fs 쓰기 + Zip Slip 방어 | `path.rs`, `error.rs` |
| `ui.rs` | 메시지 빌더(순수) + `MessageSink` trait | `error.rs` |
| `main.rs` | 최상위 조합 — `unwrap()`/`expect()` 절대 금지 | 전체 |

### 핵심 설계 결정

- **`next_available()`는 호출자 책임**: `compress()`와 `extract()` 코어 함수는 최종 경로를 받아서 쓴다. 경로 충돌 해소(`next_available`)는 `main.rs`(또는 각 모드 처리 블록)에서 호출 전에 수행한다. 두 책임을 분리해야 각각 독립적으로 테스트 가능하다.
- **`MessageSink` trait**: 실제 `WindowsMessageBox`와 테스트용 `CaptureMessageBox`를 교체 가능하게 분리. 메시지 빌더 로직(어떤 조건에서 어떤 문자열을 만드는가)을 TDD로 검증한다.

---

## 2. 의존성 크레이트

```toml
[package]
name    = "zip_sendto"
version = "0.1.0"
edition = "2021"

[dependencies]
zip       = { version = "2", default-features = false, features = ["deflate-zlib-ng"] }
walkdir   = "2"
thiserror = "2"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
] }

[profile.release]
opt-level = 3
lto       = true
strip     = true
panic     = "abort"   # 바이너리 크기 절감 + 코어 함수 외부 panic 추가 차단
```

> **빌드 주의**: `deflate-zlib-ng`는 빌드 시 C 컴파일러(MSVC cl.exe 또는 MinGW)가 필요하다.  
> `cargo build` 최초 실행 시 오류 없이 통과하는지 확인 후 다음 마일스톤으로 진행한다.

---

## 3. 마일스톤

---

### M0 — Cargo 프로젝트 초기화

**목표**: `cargo build`, `cargo test`, `cargo clippy`가 오류/경고 없이 통과하는 뼈대

#### (1) 작성할 테스트
없음 (뼈대 단계)

#### (2) 구현 항목
- `cargo init --name zip_sendto`
- `Cargo.toml` — 2절 의존성 그대로 작성
- 각 모듈 파일 생성 (`src/config.rs`, `error.rs`, `args.rs`, `path.rs`, `compress.rs`, `extract.rs`, `ui.rs`)
- `main.rs`에 `fn main() {}` 최소 구현
- 각 파일에 `mod` 선언 및 `// TODO` placeholder

#### (3) 완료 기준
- `cargo build` 오류 0건
- `cargo test` 통과 (테스트 없으므로 즉시 통과)
- `cargo clippy -- -D warnings` 경고 0건

---

### M1 — 에러 타입 (`error.rs`)

**목표**: 전체 프로젝트에서 사용할 `AppError`와 `SkipReason` 확정

#### (1) 작성할 테스트

```rust
// src/error.rs 내 #[cfg(test)] mod tests

test: appError_display_no_input
  → format!("{}", AppError::NoInput) 에 "선택한 항목이 없습니다" 포함

test: appError_display_io
  → format!("{}", AppError::Io(io::Error::other("x"))) 에 "입출력 오류:" 포함

test: appError_display_zip
  → format!("{}", AppError::Zip(...)) 에 "ZIP 처리 오류:" 포함

test: appError_display_invalid_path
  → format!("{}", AppError::InvalidPath(PathBuf::from("C:/a"))) 에 "C:/a" 포함

test: appError_display_rename_limit
  → format!("{}", AppError::RenameLimit(PathBuf::from("x"))) 에 "999" 포함

test: appError_display_unsafe_path
  → format!("{}", AppError::UnsafePath("../evil".into())) 에 "../evil" 포함

test: appError_from_io_error
  → let _: AppError = io::Error::other("e").into(); — 컴파일 + 타입 확인

test: appError_from_zip_error
  → AppError::from(zip::result::ZipError::InvalidArchive("")) — Zip variant
```

#### (2) 구현 항목
- `error.rs`: `AppError` enum (6개 variant — SPEC 7.1 기준, `OutputExists` 제외, `RenameLimit` 포함)
- `error.rs`: `SkipReason` enum (`SymlinkOrJunction`, `PermissionDenied`)
- `#[derive(Debug, thiserror::Error)]` + `#[error("...")]` 한글 메시지

#### (3) 완료 기준
- `cargo test error` 전체 통과
- `cargo clippy -- -D warnings` 경고 0건

---

### M2 — argv 파싱 (`args.rs`)

**목표**: `--mode <값>` → `Mode` enum 변환, 파일 경로 수집

#### (1) 작성할 테스트

```rust
// src/args.rs 내 #[cfg(test)] mod tests
// 헬퍼: fn argv(args: &[&str]) -> Vec<OsString>

test: parse_compress_by_parent
  → argv(["--mode", "compress-by-parent", "a.txt"])
  → Ok((Mode::CompressByParent, [PathBuf("a.txt")]))

test: parse_compress_by_name
  → argv(["--mode", "compress-by-name", "a.txt"]) → Ok((CompressByName, ...))

test: parse_extract_here
  → argv(["--mode", "extract-here", "a.zip"]) → Ok((ExtractHere, ...))

test: parse_extract_subdir
  → argv(["--mode", "extract-subdir", "a.zip"]) → Ok((ExtractSubdir, ...))

test: parse_mode_after_files
  → argv(["a.txt", "--mode", "compress-by-parent"]) → Ok(...)  // 순서 무관

test: parse_missing_mode_returns_err
  → argv(["a.txt"]) → Err(AppError)

test: parse_unknown_mode_returns_err
  → argv(["--mode", "bad-value", "a.txt"]) → Err(AppError)

test: parse_no_files_returns_no_input
  → argv(["--mode", "compress-by-parent"]) → Err(AppError::NoInput)

test: parse_multiple_files
  → argv(["--mode", "extract-here", "a.zip", "b.zip"])
  → Ok((ExtractHere, [PathBuf("a.zip"), PathBuf("b.zip")]))
```

#### (2) 구현 항목
- `args.rs`: `pub enum Mode { CompressByParent, CompressByName, ExtractHere, ExtractSubdir }`
- `args.rs`: `pub fn parse_args(args: &[OsString]) -> Result<(Mode, Vec<PathBuf>), AppError>`
  - `--mode <value>` 파싱 (공백 구분). `--mode=value` 형식도 지원.
  - 나머지 인자를 `PathBuf` 목록으로 수집
  - `--mode` 누락 또는 알 수 없는 값 → `AppError::InvalidPath` (또는 별도 variant)
  - 파일 목록이 비면 → `AppError::NoInput`

#### (3) 완료 기준
- `cargo test args` 전체 통과
- `cargo clippy -- -D warnings` 경고 0건

---

### M3 — 경로 유틸리티 (`path.rs`)

**목표**: 공통 부모 계산, zip 내부 경로 변환, 출력 파일명 계산, 충돌 리네임, Zip Slip 검사 — 모두 순수/파일읽기 함수

#### (1) 작성할 테스트

```rust
// src/path.rs 내 #[cfg(test)] mod tests
// 주의: next_available, zip_slip_check 는 실제 fs를 사용하므로 tempdir 필요

// --- common_parent ---
test: common_parent_single_file
  → common_parent(&["C:/docs/a.txt"]) == Ok(PathBuf("C:/docs"))

test: common_parent_same_dir
  → common_parent(&["C:/docs/a.txt", "C:/docs/b.txt"]) == Ok(PathBuf("C:/docs"))

test: common_parent_nested
  → common_parent(&["C:/a/b/c.txt", "C:/a/d.txt"]) == Ok(PathBuf("C:/a"))

test: common_parent_drive_root
  → common_parent(&["C:/a.txt"]) == Ok(PathBuf("C:/"))
  // 부모가 drive root인 것 자체는 오류가 아님 (file_name()=None 처리는 output_zip_name에서)

// --- zip_entry_path ---
test: zip_entry_path_file
  → zip_entry_path(Path("C:/base/b.txt"), Path("C:/base"), false) == Ok("b.txt")

test: zip_entry_path_nested
  → zip_entry_path(Path("C:/base/sub/c.txt"), Path("C:/base"), false) == Ok("sub/c.txt")
  // Windows '\' → '/' 변환 확인

test: zip_entry_path_directory
  → zip_entry_path(Path("C:/base/dir"), Path("C:/base"), true) == Ok("dir/")

test: zip_entry_path_unicode
  → zip_entry_path(Path("C:/base/한글.txt"), Path("C:/base"), false) == Ok("한글.txt")

// --- output_zip_name ---
test: output_name_parent_normal
  → output_zip_name(CompressByParent, &["C:/Projects/MyApp/a.txt"])
  == Ok(PathBuf("C:/Projects/MyApp/Projects.zip"))
  // 부모=C:/Projects → "Projects.zip", 출력 위치=C:/Projects

test: output_name_parent_drive_root
  → items 공통 부모가 C:\ → PathBuf("C:/C.zip")

test: output_name_item_file
  → output_zip_name(CompressByName, &["C:/docs/report.pdf"])
  == Ok(PathBuf("C:/docs/report.zip"))   // file_stem = "report"

test: output_name_item_folder
  → output_zip_name(CompressByName, &["C:/docs/my.folder"])
  == Ok(PathBuf("C:/docs/my.folder.zip"))  // 폴더: file_name 그대로

test: output_name_item_no_extension
  → output_zip_name(CompressByName, &["C:/docs/Makefile"])
  == Ok(PathBuf("C:/docs/Makefile.zip"))

// --- next_available ---
test: next_available_no_conflict
  → 경로 미존재 → 원래 경로 그대로 반환

test: next_available_one_conflict
  → "a.txt" 존재, "a (1).txt" 미존재 → "a (1).txt" 반환

test: next_available_sequence
  → "a.txt", "a (1).txt", "a (2).txt" 존재 → "a (3).txt" 반환

test: next_available_folder
  → "MyDir" 폴더 존재 → "MyDir (1)" 반환 (확장자 없음)

test: next_available_zip_rename
  → "out.zip" 존재 → "out (1).zip" 반환

test: next_available_limit_exceeded
  → "a.txt" ~ "a (999).txt" 모두 존재 → Err(AppError::RenameLimit)

// --- zip_slip_check ---
test: zip_slip_check_safe_path
  → zip_slip_check("sub/file.txt", Path("/target")) == Ok(PathBuf("/target/sub/file.txt"))

test: zip_slip_check_dotdot
  → zip_slip_check("../evil.txt", Path("/target")) == Err(AppError::UnsafePath)

test: zip_slip_check_absolute
  → zip_slip_check("/etc/passwd", Path("/target")) == Err(AppError::UnsafePath)

test: zip_slip_check_encoded_traversal
  → zip_slip_check("a/../../etc/passwd", Path("/target")) == Err(AppError::UnsafePath)
```

#### (2) 구현 항목
- `path.rs`: `pub fn common_parent(items: &[PathBuf]) -> Result<PathBuf, AppError>`
  - 단일 항목: `parent()` 반환
  - 다중 항목: `Path::ancestors()` 교집합으로 공통 조상 탐색
- `path.rs`: `pub fn zip_entry_path(path: &Path, base: &Path, is_dir: bool) -> Result<String, AppError>`
  - `path.strip_prefix(base)` → 상대경로
  - 컴포넌트 단위 `'/'` 조합
  - `is_dir=true` 면 끝에 `'/'` 추가
- `path.rs`: `pub fn output_zip_name(mode: &Mode, items: &[PathBuf]) -> Result<PathBuf, AppError>`
  - `CompressByParent`: `common_parent` → `file_name()`. `None`이면(drive root) 드라이브 문자 사용
  - `CompressByName`: 첫 항목이 폴더면 `file_name()`, 파일이면 `file_stem()`
  - 출력 위치: `common_parent(items)?`에 파일명 붙이기
- `path.rs`: `pub fn next_available(path: &Path) -> Result<PathBuf, AppError>`
  - `!path.exists()` → `path` 그대로 반환
  - `(stem + " (N)" + ext)` 형식으로 N=1..=999 순회
  - 999 초과 → `AppError::RenameLimit(path.to_path_buf())`
  - 폴더/확장자 없는 파일: `" (N)"` 끝에 붙이기
- `path.rs`: `pub fn zip_slip_check(entry_name: &str, target_dir: &Path) -> Result<PathBuf, AppError>`
  - `target_dir.join(entry_name).canonicalize()` 대신 **lexical 정규화** 사용 (대상 파일이 아직 없으므로 canonicalize 불가)
  - `Path::components()`로 `..` 컴포넌트 검사
  - 절대경로 엔트리 거부
  - 결과 경로가 `target_dir`로 시작하는지 검사

#### (3) 완료 기준
- `cargo test path` 전체 통과
- `cargo clippy -- -D warnings` 경고 0건

---

### M4 — 압축 코어 (`compress.rs`)

**목표**: 파일/폴더 목록 → zip 파일 생성. 스킵 보고. 오류 시 롤백.

#### (1) 작성할 테스트

```rust
// src/compress.rs 내 #[cfg(test)] mod tests
// 헬퍼: fn build_test_tree(tmp: &TempDir) — 알려진 파일 트리 생성

test: compress_single_file
  → 파일 1개 압축 → zip에 엔트리 1개, entry_count == 1

test: compress_single_folder
  → 폴더 1개 (하위 2개 파일 포함) → zip에 폴더+파일 엔트리 모두 존재

test: compress_multi_items
  → 파일 2개 + 폴더 1개 선택 → 모두 zip 루트에 배치
  (zip 내부에서 파일들이 공통 부모 없이 루트에 위치하는지 확인)

test: compress_empty_folder_preserved
  → 빈 폴더 포함 트리 압축 → zip에 "empty_dir/" 엔트리 존재, 크기 0

test: compress_unicode_filename
  → "한글파일.txt" 포함 → zip 엔트리 이름 UTF-8 보존

test: compress_entry_separator_is_slash
  → 중첩 폴더의 zip 엔트리 이름에 '\' 없고 '/' 사용

test: compress_skips_symlink
  → 심볼릭 링크 포함 트리 → report.skipped 에 SymlinkOrJunction 기록
  → zip에 링크 엔트리 없음

test: compress_skips_permission_denied
  → 읽기 불가 파일 포함 → report.skipped 에 PermissionDenied 기록
  → 압축은 계속 진행

test: compress_rollback_on_error
  → output_path를 존재하는 디렉토리 경로로 지정 (파일 생성 불가)
  → Err 반환 + output_path 파일 미존재 확인
  // 주의: 이 테스트는 플랫폼 동작에 의존. 실패 시 #[ignore] 처리 가능

test: compress_report_output_path
  → report.output_path == 전달한 output_path
```

#### (2) 구현 항목
- `compress.rs`: `pub struct CompressReport { pub output_path: PathBuf, pub entry_count: usize, pub skipped: Vec<(PathBuf, SkipReason)> }`
- `compress.rs`: `pub fn compress(items: &[PathBuf], output_path: &Path) -> Result<CompressReport, AppError>`
  1. `BufWriter<File>` + `ZipWriter` 생성 (실패 시 즉시 `?`)
  2. `walkdir::WalkDir` 순회 (`follow_links(false)`)
     - `walkdir::Error` → `PermissionDenied` 로 스킵 기록 후 `continue`
     - 심볼릭 링크(`entry.path_is_symlink()`) → `SymlinkOrJunction` 스킵 후 `continue`
     - 디렉토리 엔트리: `zip.add_directory(entry_name, options)`
     - 파일 엔트리: 크기 기반 압축 레벨 선택 → `zip.start_file(entry_name, options)` → `io::copy`
  3. `zip.finish()` 후 `Ok(report)` 반환
  4. `?`로 전파된 오류 → 클로저/래퍼 구조로 롤백 실행 후 오류 재반환
- `config.rs`: `COMPRESS_LEVEL_HIGH: i32`, `COMPRESS_LEVEL_MED: i32`, `SIZE_THRESHOLD: u64`, `STORE_EXTENSIONS: &[&str]` (값은 구현 시 결정)

#### (3) 완료 기준
- `cargo test compress` 전체 통과 (`compress_rollback_on_error` 는 통과 또는 `#[ignore]`)
- `cargo clippy -- -D warnings` 경고 0건

---

### M5 — 압축 해제 코어 (`extract.rs`)

**목표**: zip → `target_dir` 해제. 충돌 자동 리네임. Zip Slip 방어.

#### (1) 작성할 테스트

```rust
// src/extract.rs 내 #[cfg(test)] mod tests

test: extract_single_file
  → 파일 1개 엔트리 zip → 해제 후 파일 존재, 내용 일치

test: extract_nested_dirs
  → "a/b/c.txt" 엔트리 → target_dir/a/b/c.txt 생성

test: extract_empty_dir_entry
  → "empty/" 엔트리 → target_dir/empty/ 폴더 생성

test: extract_unicode_filename
  → "한글.txt" 엔트리 → target_dir/한글.txt 생성

test: extract_rename_on_conflict
  → "file.txt" 가 target_dir에 이미 존재 → "file (1).txt" 생성
  → report.renamed == [("file.txt", "file (1).txt")]

test: extract_rename_sequence
  → "f.txt", "f (1).txt" 모두 존재 → "f (2).txt" 생성

test: extract_rename_records_full_path
  → "dir/file.txt" 충돌 → renamed에 ("dir/file.txt", "dir/file (1).txt") 기록
  (zip 내부 전체 경로 기준)

test: extract_rename_limit_error
  → "a.txt" ~ "a (999).txt" 모두 존재 → Err(AppError::RenameLimit)

test: extract_zip_slip_dotdot
  → "../evil" 엔트리 포함 zip → Err(AppError::UnsafePath)

test: extract_zip_slip_absolute
  → "/etc/passwd" 형식 엔트리 → Err(AppError::UnsafePath)

test: extract_non_zip_file
  → 임의의 텍스트 파일을 zip_path로 전달 → Err(AppError::Zip)

test: extract_report_target_dir
  → report.target_dir == 전달한 target_dir
```

헬퍼:
- `fn make_zip_with_entries(path: &Path, entries: &[(&str, &[u8])])` — 테스트용 zip 생성
- `fn make_malicious_zip(path: &Path, entry_name: &str)` — Zip Slip 테스트용

#### (2) 구현 항목
- `extract.rs`: `pub struct ExtractReport { pub target_dir: PathBuf, pub renamed: Vec<(String, String)> }`
- `extract.rs`: `pub fn extract(zip_path: &Path, target_dir: &Path) -> Result<ExtractReport, AppError>`
  1. `ZipArchive::new(BufReader<File>)`
  2. 각 엔트리 반복:
     a. `zip_slip_check(entry.name(), target_dir)` → 실제 대상 경로
     b. `next_available(&dest)` → 리네임 경로 확정
     c. 리네임 발생 시 `renamed`에 `(entry.name().to_owned(), renamed_path_as_zip_str)` 기록
     d. 폴더 엔트리: `fs::create_dir_all(dest)`
     e. 파일 엔트리: 부모 디렉토리 생성 → `BufWriter<File>` + `io::copy(entry, writer)`
  3. `Ok(report)` 반환

#### (3) 완료 기준
- `cargo test extract` 전체 통과
- `cargo clippy -- -D warnings` 경고 0건

---

### M6 — 라운드트립 통합 테스트

**목표**: SPEC 13.1의 핵심 — `compress` + `extract` 후 원본과 동일한 디렉토리 트리 복원

#### (1) 작성할 테스트

```rust
// tests/roundtrip.rs  (Cargo integration test)

// 헬퍼 함수 (tests/common/mod.rs 또는 인라인)
fn setup_test_tree(tmp: &TempDir)          // 알려진 파일 트리 생성
fn assert_dir_trees_equal(a: &Path, b: &Path)  // 구조 + 파일 내용 비교
fn make_malicious_zip(path: &Path)         // "../" 포함 엔트리 zip 생성
fn fill_rename_slots(dir: &Path, stem: &str, ext: &str, count: usize) // 999개 파일 생성

test: roundtrip_single_file
test: roundtrip_single_folder            (하위 파일 + 빈 폴더 포함)
test: roundtrip_multi_items              (파일 + 폴더 혼합)
test: roundtrip_empty_folder_preserved
test: roundtrip_unicode_paths            (한글 파일명/폴더명)
test: roundtrip_skips_symlink            → 압축 결과에서 제외, skipped 기록
test: roundtrip_extract_here_conflict    → "여기에 풀기" 충돌 → 리네임 후 해제, renamed 기록
test: roundtrip_new_subdir_conflict      → "새 폴더에 풀기" 폴더 충돌 → 폴더 리네임 후 해제
test: roundtrip_output_zip_conflict      → 출력 zip 이미 존재 → next_available 거쳐 압축 성공
test: roundtrip_zip_slip                 → AppError::UnsafePath
test: roundtrip_rename_limit_compress    → 999 zip 충돌 → AppError::RenameLimit
test: roundtrip_rename_limit_extract     → 999 파일 충돌 → AppError::RenameLimit
```

#### (2) 구현 항목
- 신규 구현 없음 (M4·M5 함수 조합)
- 테스트 헬퍼 함수 작성 (`tests/common/`)
- `assert_dir_trees_equal`: 재귀적으로 파일명·크기·내용 비교 (mtime 제외)

#### (3) 완료 기준
- `cargo test` 전체 통과 (M1~M6 누적, `roundtrip_` 포함)
- `cargo clippy -- -D warnings` 경고 0건

---

### M7 — UI 레이어 (`ui.rs`)

**목표**: 메시지 빌더 로직 TDD. `WindowsMessageBox`는 수동 1회 확인.

#### (1) 작성할 테스트

```rust
// src/ui.rs 내 #[cfg(test)] mod tests
// CaptureMessageBox 사용

test: message_error_format
  → error_message(&AppError::NoInput) 가 "선택한 항목이 없습니다" 포함

test: message_renamed_none_when_empty
  → renamed_message(&[]) == None

test: message_renamed_some_when_nonempty
  → renamed_message(&[("a.txt".into(), "a (1).txt".into())]) == Some(...)

test: message_renamed_contains_both_names
  → 반환 문자열에 "a.txt" 와 "a (1).txt" 모두 포함

test: message_skipped_none_when_empty
  → skipped_message(&[]) == None

test: message_skipped_some_when_nonempty
  → skipped에 1건 이상 → Some(...)

test: message_skipped_contains_path
  → 메시지에 스킵된 파일 경로 포함

test: message_multi_failure_contains_all
  → 2개 실패 항목 → 메시지에 두 경로 모두 포함

test: capture_sink_records_message
  → CaptureMessageBox에 show_error("x") 호출 → messages 에 "x" 기록
```

#### (2) 구현 항목
- `ui.rs`: `pub trait MessageSink { fn show_error(&self, msg: &str); fn show_info(&self, msg: &str); }`
- `ui.rs`: `pub struct WindowsMessageBox;` impl `MessageSink`
  - `MessageBoxW(null, title_w, msg_w, MB_OK | MB_ICONERROR)` / `MB_ICONINFORMATION`
  - 창 제목: `"ZIP 도구"`
  - `&str` → UTF-16 변환 (`encode_utf16().chain([0]).collect::<Vec<u16>>()`)
- `ui.rs` (tests 전용): `pub struct CaptureMessageBox { pub messages: RefCell<Vec<String>> }`
- `ui.rs`: 메시지 빌더 순수 함수
  - `pub fn error_message(err: &AppError) -> String`
  - `pub fn renamed_message(renamed: &[(String, String)]) -> Option<String>`
  - `pub fn skipped_message(skipped: &[(PathBuf, SkipReason)]) -> Option<String>`
  - `pub fn multi_failure_message(failures: &[(PathBuf, AppError)]) -> String`

#### (3) 완료 기준
- `cargo test ui` 전체 통과
- `cargo clippy -- -D warnings` 경고 0건
- 수동 1회: 테스트 바이너리에서 `WindowsMessageBox::show_error("테스트")` 호출 → 실제 팝업 표시 확인

---

### M8 — `main.rs` 연결 (수동 체크리스트)

**목표**: 모든 모듈을 연결하여 실제 Send To 동작 구현

#### (1) 작성할 테스트
없음 (수동 체크리스트)

#### (2) 구현 항목

```
main() 흐름:

parse_args(args_os()) → (mode, items) | show_error + return

match mode:
  CompressByParent | CompressByName:
      initial = output_zip_name(mode, &items)?
      output  = next_available(&initial)?
      report  = compress(&items, &output)?
      if !report.skipped.is_empty() → show_info(skipped_message)

  ExtractHere:
      failures = []
      for zip in &items:
          target = zip.parent()
          match extract(zip, target):
              Ok(report) → if renamed: show_info(renamed_message)
              Err(e)     → failures.push((zip, e))
      if !failures: show_error(multi_failure_message)

  ExtractSubdir:
      failures = []
      for zip in &items:
          initial_dir = zip.parent() / zip.file_stem()
          target = next_available(&initial_dir)?
          match extract(zip, &target):
              Ok(report) → if renamed: show_info(renamed_message)
              Err(e)     → failures.push((zip, e))
      if !failures: show_error(multi_failure_message)

※ 모든 오류는 ? 대신 match/unwrap_or_else 로 MessageBox 경유
※ unwrap()/expect()/panic!() 절대 사용 금지
```

#### (3) 완료 기준

수동 체크리스트 (SPEC 13.2):
- [ ] `cargo build --release` 성공, 단일 exe 생성
- [ ] 단일 파일 → "압축 (항목명)": `파일명.zip` 생성, 내용 확인
- [ ] 단일 파일 → "압축 (폴더명)": 부모 폴더 이름으로 zip 생성
- [ ] 단일 폴더 → 양쪽 모드: 구조 보존 확인
- [ ] 다중 선택(파일+폴더 혼합) → 압축: 모두 zip 루트에 배치
- [ ] 압축 zip 이미 존재 → `(1)` 자동 리네임으로 새 zip 생성, 팝업 없음
- [ ] zip → "압축풀기 (여기에)": 올바른 위치에 해제, 충돌 시 리네임 + 알림 팝업
- [ ] zip → "압축풀기 (새 폴더)": `zip파일명/` 폴더 생성 후 해제
- [ ] 존재하지 않는 파일 전달 → 한글 오류 메시지 팝업
- [ ] `cargo clippy -- -D warnings` 경고 0건

---

### M9 — 설치 스크립트 `install.ps1` (수동 체크리스트)

**목표**: Send To 메뉴 항목 자동 설치

#### 선행 확인 (스크립트 작성 전 필수)
`shell:sendto\ZIP\` 하위 폴더가 탐색기 "보내기" 메뉴에서 캐스케이드 서브메뉴로 실제 표시되는지 직접 테스트. 미동작 시 대안(아래 주석 참고)으로 전환.

#### (1) 작성할 테스트
없음 (수동 체크리스트)

#### (2) 구현 항목

```powershell
# install.ps1

$exe = Join-Path $PSScriptRoot "zip_sendto.exe"
if (-not (Test-Path $exe)) { Write-Error "exe 없음: $exe"; exit 1 }

# 서브메뉴 방식 (캐스케이드 확인된 경우)
$sendTo = [Environment]::GetFolderPath("SendTo")
$menuDir = Join-Path $sendTo "ZIP"
New-Item -ItemType Directory -Force $menuDir | Out-Null

$menus = @(
    @{ Name = "압축 (폴더명)"; Mode = "compress-by-parent" },
    @{ Name = "압축 (항목명)"; Mode = "compress-by-name"   },
    @{ Name = "압축풀기 (여기에)"; Mode = "extract-here"   },
    @{ Name = "압축풀기 (새 폴더)"; Mode = "extract-subdir" }
)

$wsh = New-Object -ComObject WScript.Shell
foreach ($m in $menus) {
    $lnk = $wsh.CreateShortcut("$menuDir\$($m.Name).lnk")
    $lnk.TargetPath  = $exe
    $lnk.Arguments   = "--mode $($m.Mode)"
    $lnk.Description = $m.Name
    $lnk.Save()
}

Write-Host "설치 완료: $menuDir"

# --- 대안 (캐스케이드 미동작 시) ---
# $menuDir = $sendTo
# $menus 의 Name을 "ZIP - 압축 (폴더명)" 등 접두사 방식으로 변경
```

#### (3) 완료 기준

수동 체크리스트 (SPEC 13.2):
- [ ] `install.ps1` 실행 후 `shell:sendto\ZIP\`에 바로가기 4개 생성 확인
- [ ] 탐색기 "보내기" → "ZIP >" 서브메뉴 (또는 대안) 표시 확인
- [ ] 각 항목 클릭 → Task Manager에서 올바른 `--mode` 인자로 exe 실행 확인
- [ ] 스크립트 재실행 → 오류 없이 덮어쓰기 (멱등 동작) 확인

---

## 4. 모듈 의존 관계

```
main.rs
 ├─ args.rs     (M2)   ──→ error.rs
 ├─ path.rs     (M3)   ──→ error.rs, args.rs
 ├─ compress.rs (M4)   ──→ path.rs, config.rs, error.rs
 ├─ extract.rs  (M5)   ──→ path.rs, error.rs
 └─ ui.rs       (M7)   ──→ error.rs

config.rs (M0)  ← 의존성 없음 (상수만)
error.rs  (M1)  ← 의존성 없음 (최하위)
```

각 마일스톤은 화살표 왼쪽이 완료된 후 시작한다.  
M4·M5는 M3 완료 후 **병렬 진행 가능**.  
M6(통합)은 M4·M5 모두 완료 후 시작.

---

## 5. 마일스톤 요약

| # | 마일스톤 | 핵심 산출물 | 검증 |
|---|---|---|---|
| M0 | 프로젝트 초기화 | `Cargo.toml` + 뼈대 | `cargo build` |
| M1 | 에러 타입 | `error.rs` | `cargo test error` |
| M2 | argv 파싱 | `args.rs` | `cargo test args` |
| M3 | 경로 유틸리티 | `path.rs` | `cargo test path` |
| M4 | 압축 코어 | `compress.rs` | `cargo test compress` |
| M5 | 압축 해제 코어 | `extract.rs` | `cargo test extract` |
| M6 | 라운드트립 통합 | `tests/roundtrip.rs` | `cargo test` |
| M7 | UI 레이어 | `ui.rs` | `cargo test ui` + 수동 1회 |
| M8 | main.rs 연결 | 동작하는 exe | 수동 체크리스트 |
| M9 | 설치 스크립트 | `install.ps1` | 수동 체크리스트 |
