use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::error::AppError;
use crate::path::{next_available, zip_slip_check};

pub struct ExtractReport {
    pub target_dir: PathBuf,
    pub renamed: Vec<(String, String)>,
}

pub fn extract(zip_path: &Path, target_dir: &Path) -> Result<ExtractReport, AppError> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(BufReader::new(file))?;
    let mut renamed: Vec<(String, String)> = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_name = entry.name().to_owned();
        let is_dir = entry.is_dir();

        let dest = zip_slip_check(&entry_name, target_dir)?;

        if is_dir {
            drop(entry);
            fs::create_dir_all(&dest)?;
        } else {
            let actual_dest = next_available(&dest)?;

            if actual_dest != dest {
                renamed.push((entry_name, path_to_zip_str(&actual_dest, target_dir)));
            }

            if let Some(parent) = actual_dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut writer = BufWriter::new(File::create(&actual_dest)?);
            io::copy(&mut entry, &mut writer)?;
        }
    }

    Ok(ExtractReport {
        target_dir: target_dir.to_path_buf(),
        renamed,
    })
}

fn path_to_zip_str(path: &Path, base: &Path) -> String {
    match path.strip_prefix(base) {
        Ok(rel) => rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn make_tempdir() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("zip_st_extract_{}", id));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_zip_with_entries(zip_path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        for (name, content) in entries {
            if name.ends_with('/') {
                zip.add_directory(*name, SimpleFileOptions::default()).unwrap();
            } else {
                zip.start_file(*name, SimpleFileOptions::default()).unwrap();
                zip.write_all(content).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    fn make_malicious_zip(zip_path: &Path, entry_name: &str) {
        let file = File::create(zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file(entry_name, SimpleFileOptions::default()).unwrap();
        zip.write_all(b"evil").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn extract_single_file() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        make_zip_with_entries(&zip_path, &[("file.txt", b"hello world")]);
        extract(&zip_path, &target).unwrap();
        assert_eq!(fs::read(target.join("file.txt")).unwrap(), b"hello world");
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_nested_dirs() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        make_zip_with_entries(&zip_path, &[("a/b/c.txt", b"nested")]);
        extract(&zip_path, &target).unwrap();
        assert!(target.join("a").join("b").join("c.txt").exists());
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_empty_dir_entry() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        make_zip_with_entries(&zip_path, &[("empty/", b"")]);
        extract(&zip_path, &target).unwrap();
        assert!(target.join("empty").is_dir());
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_unicode_filename() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        make_zip_with_entries(&zip_path, &[("한글.txt", b"content")]);
        extract(&zip_path, &target).unwrap();
        assert!(target.join("한글.txt").exists());
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_rename_on_conflict() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("file.txt"), b"existing").unwrap();
        make_zip_with_entries(&zip_path, &[("file.txt", b"new")]);
        let report = extract(&zip_path, &target).unwrap();
        assert!(target.join("file (1).txt").exists());
        assert_eq!(
            report.renamed,
            vec![("file.txt".to_owned(), "file (1).txt".to_owned())]
        );
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_rename_sequence() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("f.txt"), b"0").unwrap();
        fs::write(target.join("f (1).txt"), b"1").unwrap();
        make_zip_with_entries(&zip_path, &[("f.txt", b"new")]);
        extract(&zip_path, &target).unwrap();
        assert!(target.join("f (2).txt").exists());
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_rename_records_full_path() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        let dir_path = target.join("dir");
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join("file.txt"), b"existing").unwrap();
        make_zip_with_entries(&zip_path, &[("dir/", b""), ("dir/file.txt", b"new")]);
        let report = extract(&zip_path, &target).unwrap();
        assert_eq!(
            report.renamed,
            vec![("dir/file.txt".to_owned(), "dir/file (1).txt".to_owned())]
        );
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_rename_limit_error() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("a.txt"), b"").unwrap();
        for n in 1u32..=999 {
            fs::write(target.join(format!("a ({}).txt", n)), b"").unwrap();
        }
        make_zip_with_entries(&zip_path, &[("a.txt", b"new")]);
        assert!(matches!(
            extract(&zip_path, &target),
            Err(AppError::RenameLimit(_))
        ));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_zip_slip_dotdot() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("evil.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        make_malicious_zip(&zip_path, "../evil.txt");
        assert!(matches!(
            extract(&zip_path, &target),
            Err(AppError::UnsafePath(_))
        ));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_zip_slip_absolute() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("evil.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        make_malicious_zip(&zip_path, "/etc/passwd");
        assert!(matches!(
            extract(&zip_path, &target),
            Err(AppError::UnsafePath(_))
        ));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_non_zip_file() {
        let tmp = make_tempdir();
        let not_a_zip = tmp.join("notazip.txt");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        fs::write(&not_a_zip, b"this is not a zip file at all").unwrap();
        assert!(matches!(
            extract(&not_a_zip, &target),
            Err(AppError::Zip(_))
        ));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn extract_report_target_dir() {
        let tmp = make_tempdir();
        let zip_path = tmp.join("test.zip");
        let target = tmp.join("out");
        fs::create_dir_all(&target).unwrap();
        make_zip_with_entries(&zip_path, &[("file.txt", b"content")]);
        let report = extract(&zip_path, &target).unwrap();
        assert_eq!(report.target_dir, target);
        fs::remove_dir_all(&tmp).ok();
    }
}
