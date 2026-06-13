use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::config;
use crate::error::{AppError, SkipReason};
use crate::path::zip_entry_path;

pub struct CompressReport {
    pub output_path: PathBuf,
    pub entry_count: usize,
    pub skipped: Vec<(PathBuf, SkipReason)>,
}

pub fn compress(items: &[PathBuf], output_path: &Path) -> Result<CompressReport, AppError> {
    compress_inner(items, output_path).inspect_err(|_| {
        fs::remove_file(output_path).ok();
    })
}

fn compress_inner(items: &[PathBuf], output_path: &Path) -> Result<CompressReport, AppError> {
    let file = File::create(output_path)?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let mut entry_count = 0usize;
    let mut skipped: Vec<(PathBuf, SkipReason)> = Vec::new();

    for item in items {
        let base = item
            .parent()
            .ok_or_else(|| AppError::InvalidPath(item.clone()))?;

        for entry_result in WalkDir::new(item).follow_links(false) {
            let entry = match entry_result {
                Ok(e) => e,
                Err(e) => {
                    let path = e
                        .path()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| item.clone());
                    skipped.push((path, SkipReason::PermissionDenied));
                    continue;
                }
            };

            if entry.path_is_symlink() {
                skipped.push((entry.path().to_path_buf(), SkipReason::SymlinkOrJunction));
                continue;
            }

            let is_dir = entry.file_type().is_dir();
            let entry_name = zip_entry_path(entry.path(), base, is_dir)?;

            if is_dir {
                zip.add_directory(&entry_name, SimpleFileOptions::default())?;
            } else {
                let size = entry.path().metadata().map(|m| m.len()).unwrap_or(0);
                let ext = entry
                    .path()
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();

                let options = if config::STORE_EXTENSIONS.contains(&ext.as_str()) {
                    SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
                } else if size >= config::SIZE_THRESHOLD {
                    SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Deflated)
                        .compression_level(Some(config::COMPRESS_LEVEL_MED))
                } else {
                    SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Deflated)
                        .compression_level(Some(config::COMPRESS_LEVEL_HIGH))
                };

                // Open the file before starting the zip entry to avoid a partial entry on error.
                let mut f = match File::open(entry.path()) {
                    Ok(f) => f,
                    Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                        skipped.push((entry.path().to_path_buf(), SkipReason::PermissionDenied));
                        continue;
                    }
                    Err(e) => return Err(AppError::Io(e)),
                };

                zip.start_file(&entry_name, options)?;
                io::copy(&mut f, &mut zip)?;
                entry_count += 1;
            }
        }
    }

    zip.finish()?;

    Ok(CompressReport {
        output_path: output_path.to_path_buf(),
        entry_count,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn make_tempdir() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("zip_st_compress_{}", id));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn zip_entry_names(zip_path: &Path) -> Vec<String> {
        let file = File::open(zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(BufReader::new(file)).unwrap();
        (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_owned())
            .collect()
    }

    #[cfg(windows)]
    fn try_create_symlink(src: &Path, dst: &Path) -> bool {
        std::os::windows::fs::symlink_file(src, dst).is_ok()
    }
    #[cfg(unix)]
    fn try_create_symlink(src: &Path, dst: &Path) -> bool {
        std::os::unix::fs::symlink(src, dst).is_ok()
    }

    #[test]
    fn compress_single_file() {
        let tmp = make_tempdir();
        let file = tmp.join("a.txt");
        fs::write(&file, b"hello world").unwrap();
        let output = tmp.join("out.zip");
        let report = compress(&[file], &output).unwrap();
        assert_eq!(report.entry_count, 1);
        let entries = zip_entry_names(&output);
        assert_eq!(entries, vec!["a.txt"]);
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn compress_single_folder() {
        let tmp = make_tempdir();
        let dir = tmp.join("mydir");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"aaa").unwrap();
        fs::write(dir.join("b.txt"), b"bbb").unwrap();
        let output = tmp.join("out.zip");
        let report = compress(&[dir], &output).unwrap();
        assert_eq!(report.entry_count, 2);
        let entries = zip_entry_names(&output);
        assert!(entries.contains(&"mydir/".to_owned()));
        assert!(entries.contains(&"mydir/a.txt".to_owned()));
        assert!(entries.contains(&"mydir/b.txt".to_owned()));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn compress_multi_items() {
        let tmp = make_tempdir();
        let file1 = tmp.join("a.txt");
        let file2 = tmp.join("b.txt");
        let dir = tmp.join("subdir");
        fs::write(&file1, b"a").unwrap();
        fs::write(&file2, b"b").unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("c.txt"), b"c").unwrap();
        let output = tmp.join("out.zip");
        let report = compress(&[file1, file2, dir], &output).unwrap();
        assert_eq!(report.entry_count, 3);
        let entries = zip_entry_names(&output);
        assert!(entries.contains(&"a.txt".to_owned()));
        assert!(entries.contains(&"b.txt".to_owned()));
        assert!(entries.contains(&"subdir/".to_owned()));
        assert!(entries.contains(&"subdir/c.txt".to_owned()));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn compress_empty_folder_preserved() {
        let tmp = make_tempdir();
        let dir = tmp.join("mydir");
        fs::create_dir_all(dir.join("empty_dir")).unwrap();
        fs::write(dir.join("file.txt"), b"content").unwrap();
        let output = tmp.join("out.zip");
        compress(&[dir], &output).unwrap();
        let entries = zip_entry_names(&output);
        assert!(
            entries.iter().any(|e| e == "mydir/empty_dir/" || e == "mydir/empty_dir"),
            "empty_dir 엔트리 없음: {:?}",
            entries
        );
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn compress_unicode_filename() {
        let tmp = make_tempdir();
        let file = tmp.join("한글파일.txt");
        fs::write(&file, b"content").unwrap();
        let output = tmp.join("out.zip");
        compress(&[file], &output).unwrap();
        let entries = zip_entry_names(&output);
        assert!(entries.contains(&"한글파일.txt".to_owned()));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn compress_entry_separator_is_slash() {
        let tmp = make_tempdir();
        let dir = tmp.join("parent");
        let sub = dir.join("child");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("file.txt"), b"content").unwrap();
        let output = tmp.join("out.zip");
        compress(&[dir], &output).unwrap();
        let entries = zip_entry_names(&output);
        for e in &entries {
            assert!(!e.contains('\\'), "엔트리 이름에 백슬래시 포함: '{}'", e);
        }
        assert!(entries.contains(&"parent/child/file.txt".to_owned()));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn compress_skips_symlink() {
        let tmp = make_tempdir();
        let dir = tmp.join("mydir");
        fs::create_dir_all(&dir).unwrap();
        let real_file = dir.join("real.txt");
        fs::write(&real_file, b"content").unwrap();
        let link = dir.join("link.txt");

        if !try_create_symlink(&real_file, &link) {
            // 심볼릭 링크 생성 권한 없음 — 이 환경에서 생략
            fs::remove_dir_all(&tmp).ok();
            return;
        }

        let output = tmp.join("out.zip");
        let report = compress(&[dir], &output).unwrap();
        let entries = zip_entry_names(&output);
        assert!(
            !entries.iter().any(|e| e.contains("link")),
            "링크 엔트리가 zip에 포함되었음: {:?}",
            entries
        );
        assert_eq!(report.skipped.len(), 1);
        assert!(matches!(report.skipped[0].1, SkipReason::SymlinkOrJunction));
        fs::remove_dir_all(&tmp).ok();
    }

    #[cfg(unix)]
    #[test]
    fn compress_skips_permission_denied() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = make_tempdir();
        let dir = tmp.join("mydir");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("good.txt"), b"good").unwrap();
        let bad_file = dir.join("bad.txt");
        fs::write(&bad_file, b"bad").unwrap();
        let mut perms = fs::metadata(&bad_file).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&bad_file, perms).unwrap();

        let output = tmp.join("out.zip");
        let report = compress(&[dir.clone()], &output).unwrap();

        let mut restore = fs::metadata(&bad_file).unwrap().permissions();
        restore.set_mode(0o644);
        fs::set_permissions(&bad_file, restore).ok();

        assert!(
            report.skipped.iter().any(|(_, r)| matches!(r, SkipReason::PermissionDenied)),
            "PermissionDenied 스킵이 기록되지 않음"
        );
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn compress_rollback_on_error() {
        let tmp = make_tempdir();
        let file = tmp.join("a.txt");
        fs::write(&file, b"hello").unwrap();
        // output_path를 기존 디렉토리로 지정 → File::create 실패 → 롤백
        let result = compress(&[file], &tmp);
        assert!(result.is_err(), "디렉토리 경로로 출력 시 오류 반환해야 함");
        assert!(tmp.is_dir(), "롤백 후 디렉토리가 남아 있어야 함");
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn compress_report_output_path() {
        let tmp = make_tempdir();
        let file = tmp.join("a.txt");
        fs::write(&file, b"hello").unwrap();
        let output = tmp.join("out.zip");
        let report = compress(&[file], &output).unwrap();
        assert_eq!(report.output_path, output);
        fs::remove_dir_all(&tmp).ok();
    }
}
