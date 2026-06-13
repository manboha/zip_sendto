use std::path::PathBuf;

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

#[derive(Debug)]
pub enum SkipReason {
    SymlinkOrJunction,
    PermissionDenied,
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SymlinkOrJunction => write!(f, "심볼릭 링크/정션"),
            Self::PermissionDenied => write!(f, "접근 권한 없음"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn app_error_display_no_input() {
        assert!(AppError::NoInput.to_string().contains("선택한 항목이 없습니다"));
    }

    #[test]
    fn app_error_display_io() {
        let err = AppError::Io(io::Error::other("x"));
        assert!(err.to_string().contains("입출력 오류:"));
    }

    #[test]
    fn app_error_display_zip() {
        let err = AppError::Zip(zip::result::ZipError::FileNotFound);
        assert!(err.to_string().contains("ZIP 처리 오류:"));
    }

    #[test]
    fn app_error_display_invalid_path() {
        let err = AppError::InvalidPath(PathBuf::from("test_path"));
        assert!(err.to_string().contains("test_path"));
    }

    #[test]
    fn app_error_display_rename_limit() {
        let err = AppError::RenameLimit(PathBuf::from("file.txt"));
        let msg = err.to_string();
        assert!(msg.contains("999"), "메시지에 '999' 포함 필요: {msg}");
        assert!(msg.contains("file.txt"), "메시지에 경로 포함 필요: {msg}");
    }

    #[test]
    fn app_error_display_unsafe_path() {
        let err = AppError::UnsafePath("../evil".into());
        assert!(err.to_string().contains("../evil"));
    }

    #[test]
    fn app_error_from_io_error() {
        let app_err: AppError = io::Error::other("e").into();
        assert!(matches!(app_err, AppError::Io(_)));
    }

    #[test]
    fn app_error_from_zip_error() {
        let app_err: AppError = zip::result::ZipError::FileNotFound.into();
        assert!(matches!(app_err, AppError::Zip(_)));
    }

    #[test]
    fn skip_reason_display_symlink() {
        assert!(SkipReason::SymlinkOrJunction.to_string().contains("링크"));
    }

    #[test]
    fn skip_reason_display_permission() {
        assert!(SkipReason::PermissionDenied.to_string().contains("권한"));
    }
}
