#![allow(dead_code)]

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn make_tempdir() -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("zip_st_rt_{}", id));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 알려진 파일 트리를 `dir` 안에 생성한다.
///
/// ```text
/// dir/
///   file_a.txt  ("file_a content")
///   file_b.txt  ("file_b content")
///   sub/
///     file_c.txt  ("file_c content")
///   empty_dir/
/// ```
pub fn setup_test_tree(dir: &Path) {
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::create_dir_all(dir.join("empty_dir")).unwrap();
    fs::write(dir.join("file_a.txt"), b"file_a content").unwrap();
    fs::write(dir.join("file_b.txt"), b"file_b content").unwrap();
    fs::write(dir.join("sub").join("file_c.txt"), b"file_c content").unwrap();
}

/// 두 디렉토리의 구조와 파일 내용이 동일한지 재귀적으로 검증한다 (mtime 제외).
pub fn assert_dir_trees_equal(a: &Path, b: &Path) {
    let paths_a = collect_relative_paths(a);
    let paths_b = collect_relative_paths(b);

    assert_eq!(
        paths_a,
        paths_b,
        "디렉토리 구조 불일치\n  좌({:?}): {:?}\n  우({:?}): {:?}",
        a,
        paths_a,
        b,
        paths_b
    );

    for rel in &paths_a {
        let pa = a.join(rel);
        let pb = b.join(rel);
        if pa.is_file() {
            let ca = fs::read(&pa).unwrap();
            let cb = fs::read(&pb).unwrap();
            assert_eq!(ca, cb, "파일 내용 불일치: {:?}", rel);
        } else {
            assert!(pb.is_dir(), "경로가 디렉토리여야 함: {:?}", pb);
        }
    }
}

fn collect_relative_paths(base: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_recursive(base, base, &mut paths);
    paths.sort();
    paths
}

fn collect_recursive(base: &Path, dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap().to_path_buf();
        paths.push(rel);
        if path.is_dir() {
            collect_recursive(base, &path, paths);
        }
    }
}

/// `"../evil.txt"` 엔트리를 담은 Zip Slip용 악성 zip 파일 생성
pub fn make_malicious_zip(zip_path: &Path) {
    let file = File::create(zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    zip.start_file(
        "../evil.txt",
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
    )
    .unwrap();
    zip.write_all(b"evil").unwrap();
    zip.finish().unwrap();
}

/// 지정한 엔트리를 담은 zip 파일 생성 (테스트용)
pub fn make_zip_with_entries(zip_path: &Path, entries: &[(&str, &[u8])]) {
    let file = File::create(zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    for (name, content) in entries {
        if name.ends_with('/') {
            zip.add_directory(*name, SimpleFileOptions::default()).unwrap();
        } else {
            zip.start_file(
                *name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(content).unwrap();
        }
    }
    zip.finish().unwrap();
}

/// `next_available`이 실패하도록 리네임 슬롯을 모두 채운다.
///
/// 생성: `stem.ext`, `stem (1).ext`, …, `stem (count-1).ext` (합계 `count`개)
pub fn fill_rename_slots(dir: &Path, stem: &str, ext: &str, count: usize) {
    let orig = if ext.is_empty() {
        dir.join(stem)
    } else {
        dir.join(format!("{}.{}", stem, ext))
    };
    fs::write(&orig, b"").unwrap();
    for n in 1..count {
        let name = if ext.is_empty() {
            format!("{} ({})", stem, n)
        } else {
            format!("{} ({}).{}", stem, n, ext)
        };
        fs::write(dir.join(name), b"").unwrap();
    }
}

/// 플랫폼에 맞게 심볼릭 링크 생성 시도. 실패하면 false 반환.
#[cfg(windows)]
pub fn try_create_symlink(src: &Path, dst: &Path) -> bool {
    std::os::windows::fs::symlink_file(src, dst).is_ok()
}
#[cfg(unix)]
pub fn try_create_symlink(src: &Path, dst: &Path) -> bool {
    std::os::unix::fs::symlink(src, dst).is_ok()
}
