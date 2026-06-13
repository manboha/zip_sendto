use std::path::{Component, Path, PathBuf};

use crate::args::Mode;
use crate::error::AppError;

pub fn common_parent(items: &[PathBuf]) -> Result<PathBuf, AppError> {
    if items.is_empty() {
        return Err(AppError::NoInput);
    }

    let first_parent = items[0]
        .parent()
        .ok_or_else(|| AppError::InvalidPath(items[0].clone()))?
        .to_path_buf();

    items[1..].iter().try_fold(first_parent, |acc, item| {
        let item_parent = item
            .parent()
            .ok_or_else(|| AppError::InvalidPath(item.clone()))?;
        Ok(common_ancestor(&acc, item_parent))
    })
}

fn common_ancestor(a: &Path, b: &Path) -> PathBuf {
    a.components()
        .zip(b.components())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| x)
        .collect()
}

pub fn zip_entry_path(path: &Path, base: &Path, is_dir: bool) -> Result<String, AppError> {
    let rel = path
        .strip_prefix(base)
        .map_err(|_| AppError::InvalidPath(path.to_path_buf()))?;

    let parts: Vec<_> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();

    if parts.is_empty() {
        return Err(AppError::InvalidPath(path.to_path_buf()));
    }

    let mut result = parts.join("/");
    if is_dir {
        result.push('/');
    }
    Ok(result)
}

pub fn output_zip_name(mode: &Mode, items: &[PathBuf]) -> Result<PathBuf, AppError> {
    if items.is_empty() {
        return Err(AppError::NoInput);
    }

    let parent = common_parent(items)?;

    let stem = match mode {
        Mode::CompressByParent => match parent.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => drive_letter(&parent)
                .ok_or_else(|| AppError::InvalidPath(parent.clone()))?,
        },
        Mode::CompressByName => {
            let first = &items[0];
            if first.is_dir() {
                first
                    .file_name()
                    .ok_or_else(|| AppError::InvalidPath(first.clone()))?
                    .to_string_lossy()
                    .into_owned()
            } else {
                first
                    .file_stem()
                    .ok_or_else(|| AppError::InvalidPath(first.clone()))?
                    .to_string_lossy()
                    .into_owned()
            }
        }
        Mode::ExtractHere | Mode::ExtractSubdir => {
            return Err(AppError::InvalidPath(PathBuf::from("잘못된 압축 모드")))
        }
    };

    Ok(parent.join(format!("{}.zip", stem)))
}

fn drive_letter(path: &Path) -> Option<String> {
    for component in path.components() {
        if let Component::Prefix(prefix) = component {
            let s = prefix.as_os_str().to_string_lossy();
            return Some(s.trim_end_matches(':').to_ascii_uppercase());
        }
    }
    None
}

pub fn next_available(path: &Path) -> Result<PathBuf, AppError> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }

    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| Path::new(""));

    for n in 1u32..=999 {
        let name = if ext.is_empty() {
            format!("{} ({})", stem, n)
        } else {
            format!("{} ({}){}", stem, n, ext)
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppError::RenameLimit(path.to_path_buf()))
}

pub fn zip_slip_check(entry_name: &str, target_dir: &Path) -> Result<PathBuf, AppError> {
    for component in Path::new(entry_name).components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::UnsafePath(entry_name.to_owned()));
            }
            _ => {}
        }
    }
    Ok(target_dir.join(entry_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn make_tempdir() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("zip_st_path_{}", id));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // --- common_parent ---

    #[test]
    fn common_parent_single_file() {
        let items = vec![PathBuf::from("C:/docs/a.txt")];
        assert_eq!(common_parent(&items).unwrap(), PathBuf::from("C:/docs"));
    }

    #[test]
    fn common_parent_same_dir() {
        let items = vec![
            PathBuf::from("C:/docs/a.txt"),
            PathBuf::from("C:/docs/b.txt"),
        ];
        assert_eq!(common_parent(&items).unwrap(), PathBuf::from("C:/docs"));
    }

    #[test]
    fn common_parent_nested() {
        let items = vec![
            PathBuf::from("C:/a/b/c.txt"),
            PathBuf::from("C:/a/d.txt"),
        ];
        assert_eq!(common_parent(&items).unwrap(), PathBuf::from("C:/a"));
    }

    #[test]
    fn common_parent_drive_root() {
        let items = vec![PathBuf::from("C:/a.txt")];
        let result = common_parent(&items).unwrap();
        // parent of "C:/a.txt" is the drive root "C:\"
        assert!(result.to_string_lossy().starts_with("C:"));
        assert!(result.parent().is_none() || result == *"C:\\");
    }

    // --- zip_entry_path ---

    #[test]
    fn zip_entry_path_file() {
        let result =
            zip_entry_path(Path::new("C:/base/b.txt"), Path::new("C:/base"), false).unwrap();
        assert_eq!(result, "b.txt");
    }

    #[test]
    fn zip_entry_path_nested() {
        let result =
            zip_entry_path(Path::new("C:/base/sub/c.txt"), Path::new("C:/base"), false).unwrap();
        assert_eq!(result, "sub/c.txt");
    }

    #[test]
    fn zip_entry_path_directory() {
        let result =
            zip_entry_path(Path::new("C:/base/dir"), Path::new("C:/base"), true).unwrap();
        assert_eq!(result, "dir/");
    }

    #[test]
    fn zip_entry_path_unicode() {
        let result =
            zip_entry_path(Path::new("C:/base/한글.txt"), Path::new("C:/base"), false).unwrap();
        assert_eq!(result, "한글.txt");
    }

    // --- output_zip_name ---

    #[test]
    fn output_name_parent_normal() {
        // common_parent(["C:/Projects/MyApp/a.txt"]) = "C:/Projects/MyApp"
        // file_name = "MyApp" → output = "C:/Projects/MyApp/MyApp.zip"
        let items = vec![PathBuf::from("C:/Projects/MyApp/a.txt")];
        let result = output_zip_name(&Mode::CompressByParent, &items).unwrap();
        assert_eq!(result.file_name().unwrap().to_str().unwrap(), "MyApp.zip");
    }

    #[test]
    fn output_name_parent_drive_root() {
        // common_parent(["C:/a.txt"]) = "C:\" → file_name = None → drive letter "C"
        let items = vec![PathBuf::from("C:/a.txt")];
        let result = output_zip_name(&Mode::CompressByParent, &items).unwrap();
        assert_eq!(result.file_name().unwrap().to_str().unwrap(), "C.zip");
    }

    #[test]
    fn output_name_item_file() {
        let tmp = make_tempdir();
        let file = tmp.join("report.pdf");
        fs::write(&file, b"").unwrap();
        let result = output_zip_name(&Mode::CompressByName, &[file]).unwrap();
        assert_eq!(result.file_name().unwrap().to_str().unwrap(), "report.zip");
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn output_name_item_folder() {
        let tmp = make_tempdir();
        let folder = tmp.join("my.folder");
        fs::create_dir_all(&folder).unwrap();
        let result = output_zip_name(&Mode::CompressByName, &[folder]).unwrap();
        assert_eq!(
            result.file_name().unwrap().to_str().unwrap(),
            "my.folder.zip"
        );
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn output_name_item_no_extension() {
        let tmp = make_tempdir();
        let file = tmp.join("Makefile");
        fs::write(&file, b"").unwrap();
        let result = output_zip_name(&Mode::CompressByName, &[file]).unwrap();
        assert_eq!(result.file_name().unwrap().to_str().unwrap(), "Makefile.zip");
        fs::remove_dir_all(&tmp).ok();
    }

    // --- next_available ---

    #[test]
    fn next_available_no_conflict() {
        let tmp = make_tempdir();
        let path = tmp.join("a.txt");
        assert_eq!(next_available(&path).unwrap(), path);
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn next_available_one_conflict() {
        let tmp = make_tempdir();
        let path = tmp.join("a.txt");
        fs::write(&path, b"").unwrap();
        assert_eq!(next_available(&path).unwrap(), tmp.join("a (1).txt"));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn next_available_sequence() {
        let tmp = make_tempdir();
        let path = tmp.join("a.txt");
        fs::write(&path, b"").unwrap();
        fs::write(tmp.join("a (1).txt"), b"").unwrap();
        fs::write(tmp.join("a (2).txt"), b"").unwrap();
        assert_eq!(next_available(&path).unwrap(), tmp.join("a (3).txt"));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn next_available_folder() {
        let tmp = make_tempdir();
        let folder = tmp.join("MyDir");
        fs::create_dir_all(&folder).unwrap();
        assert_eq!(next_available(&folder).unwrap(), tmp.join("MyDir (1)"));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn next_available_zip_rename() {
        let tmp = make_tempdir();
        let path = tmp.join("out.zip");
        fs::write(&path, b"").unwrap();
        assert_eq!(next_available(&path).unwrap(), tmp.join("out (1).zip"));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn next_available_limit_exceeded() {
        let tmp = make_tempdir();
        let path = tmp.join("a.txt");
        fs::write(&path, b"").unwrap();
        for n in 1u32..=999 {
            fs::write(tmp.join(format!("a ({}).txt", n)), b"").unwrap();
        }
        assert!(matches!(next_available(&path), Err(AppError::RenameLimit(_))));
        fs::remove_dir_all(&tmp).ok();
    }

    // --- zip_slip_check ---

    #[test]
    fn zip_slip_check_safe_path() {
        let result = zip_slip_check("sub/file.txt", Path::new("C:/target")).unwrap();
        assert!(result.starts_with("C:/target"));
        assert_eq!(result.file_name().unwrap(), "file.txt");
    }

    #[test]
    fn zip_slip_check_dotdot() {
        assert!(matches!(
            zip_slip_check("../evil.txt", Path::new("C:/target")),
            Err(AppError::UnsafePath(_))
        ));
    }

    #[test]
    fn zip_slip_check_absolute() {
        assert!(matches!(
            zip_slip_check("/etc/passwd", Path::new("C:/target")),
            Err(AppError::UnsafePath(_))
        ));
    }

    #[test]
    fn zip_slip_check_encoded_traversal() {
        assert!(matches!(
            zip_slip_check("a/../../etc/passwd", Path::new("C:/target")),
            Err(AppError::UnsafePath(_))
        ));
    }
}
