use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::AppError;

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    CompressByParent,
    CompressByName,
    ExtractHere,
    ExtractSubdir,
}

pub fn parse_args(args: &[OsString]) -> Result<(Mode, Vec<PathBuf>), AppError> {
    let mut mode: Option<Mode> = None;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        let s = arg.to_string_lossy();
        if s == "--mode" {
            let value = iter
                .next()
                .ok_or_else(|| AppError::InvalidPath(PathBuf::from("--mode")))?;
            mode = Some(parse_mode_str(&value.to_string_lossy())?);
        } else if let Some(value) = s.strip_prefix("--mode=") {
            mode = Some(parse_mode_str(value)?);
        } else {
            files.push(PathBuf::from(arg));
        }
    }

    let mode = mode.ok_or_else(|| AppError::InvalidPath(PathBuf::from("--mode")))?;

    if files.is_empty() {
        return Err(AppError::NoInput);
    }

    Ok((mode, files))
}

fn parse_mode_str(value: &str) -> Result<Mode, AppError> {
    match value {
        "compress-by-parent" => Ok(Mode::CompressByParent),
        "compress-by-name" => Ok(Mode::CompressByName),
        "extract-here" => Ok(Mode::ExtractHere),
        "extract-subdir" => Ok(Mode::ExtractSubdir),
        other => Err(AppError::InvalidPath(PathBuf::from(other))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<OsString> {
        args.iter().copied().map(OsString::from).collect()
    }

    #[test]
    fn parse_compress_by_parent() {
        let (mode, files) = parse_args(&argv(&["--mode", "compress-by-parent", "a.txt"])).unwrap();
        assert_eq!(mode, Mode::CompressByParent);
        assert_eq!(files, vec![PathBuf::from("a.txt")]);
    }

    #[test]
    fn parse_compress_by_name() {
        let (mode, files) = parse_args(&argv(&["--mode", "compress-by-name", "a.txt"])).unwrap();
        assert_eq!(mode, Mode::CompressByName);
        assert_eq!(files, vec![PathBuf::from("a.txt")]);
    }

    #[test]
    fn parse_extract_here() {
        let (mode, files) = parse_args(&argv(&["--mode", "extract-here", "a.zip"])).unwrap();
        assert_eq!(mode, Mode::ExtractHere);
        assert_eq!(files, vec![PathBuf::from("a.zip")]);
    }

    #[test]
    fn parse_extract_subdir() {
        let (mode, files) = parse_args(&argv(&["--mode", "extract-subdir", "a.zip"])).unwrap();
        assert_eq!(mode, Mode::ExtractSubdir);
        assert_eq!(files, vec![PathBuf::from("a.zip")]);
    }

    #[test]
    fn parse_mode_after_files() {
        let (mode, files) =
            parse_args(&argv(&["a.txt", "--mode", "compress-by-parent"])).unwrap();
        assert_eq!(mode, Mode::CompressByParent);
        assert_eq!(files, vec![PathBuf::from("a.txt")]);
    }

    #[test]
    fn parse_mode_equals_syntax() {
        let (mode, _) = parse_args(&argv(&["--mode=extract-here", "a.zip"])).unwrap();
        assert_eq!(mode, Mode::ExtractHere);
    }

    #[test]
    fn parse_missing_mode_returns_err() {
        assert!(parse_args(&argv(&["a.txt"])).is_err());
    }

    #[test]
    fn parse_unknown_mode_returns_err() {
        assert!(parse_args(&argv(&["--mode", "bad-value", "a.txt"])).is_err());
    }

    #[test]
    fn parse_no_files_returns_no_input() {
        let err = parse_args(&argv(&["--mode", "compress-by-parent"])).unwrap_err();
        assert!(matches!(err, AppError::NoInput));
    }

    #[test]
    fn parse_multiple_files() {
        let (mode, files) =
            parse_args(&argv(&["--mode", "extract-here", "a.zip", "b.zip"])).unwrap();
        assert_eq!(mode, Mode::ExtractHere);
        assert_eq!(files, vec![PathBuf::from("a.zip"), PathBuf::from("b.zip")]);
    }
}
