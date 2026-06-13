use std::path::PathBuf;

use crate::error::{AppError, SkipReason};

// ── Trait ────────────────────────────────────────────────────────────────────

pub trait MessageSink {
    fn show_error(&self, msg: &str);
    fn show_info(&self, msg: &str);
}

// ── 메시지 빌더 (순수 함수) ──────────────────────────────────────────────────

pub fn error_message(err: &AppError) -> String {
    err.to_string()
}

pub fn renamed_message(renamed: &[(String, String)]) -> Option<String> {
    if renamed.is_empty() {
        return None;
    }
    let mut msg = "파일 이름 충돌로 다음 항목을 자동으로 이름 변경했습니다:\n".to_owned();
    for (orig, new) in renamed {
        msg.push_str(&format!("  {} → {}\n", orig, new));
    }
    Some(msg)
}

pub fn skipped_message(skipped: &[(PathBuf, SkipReason)]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let mut msg = "다음 항목은 건너뛰었습니다:\n".to_owned();
    for (path, reason) in skipped {
        msg.push_str(&format!("  {} ({})\n", path.display(), reason));
    }
    Some(msg)
}

pub fn multi_failure_message(failures: &[(PathBuf, AppError)]) -> String {
    let mut msg = "다음 파일을 처리하지 못했습니다:\n".to_owned();
    for (path, err) in failures {
        msg.push_str(&format!("  {}: {}\n", path.display(), err));
    }
    msg
}

// ── WindowsMessageBox ────────────────────────────────────────────────────────

#[cfg(windows)]
pub struct WindowsMessageBox;

#[cfg(windows)]
impl MessageSink for WindowsMessageBox {
    fn show_error(&self, msg: &str) {
        use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let title: Vec<u16> = "ZIP 도구".encode_utf16().chain([0]).collect();
        let msg_w: Vec<u16> = msg.encode_utf16().chain([0]).collect();
        unsafe {
            MessageBoxW(
                None,
                windows::core::PCWSTR(msg_w.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONERROR,
            );
        }
    }

    fn show_info(&self, msg: &str) {
        use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
        let title: Vec<u16> = "ZIP 도구".encode_utf16().chain([0]).collect();
        let msg_w: Vec<u16> = msg.encode_utf16().chain([0]).collect();
        unsafe {
            MessageBoxW(
                None,
                windows::core::PCWSTR(msg_w.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
}

// ── CaptureMessageBox (테스트 전용) ──────────────────────────────────────────

#[cfg(test)]
pub struct CaptureMessageBox {
    pub messages: std::cell::RefCell<Vec<String>>,
}

#[cfg(test)]
impl Default for CaptureMessageBox {
    fn default() -> Self {
        Self {
            messages: std::cell::RefCell::new(Vec::new()),
        }
    }
}

#[cfg(test)]
impl CaptureMessageBox {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
impl MessageSink for CaptureMessageBox {
    fn show_error(&self, msg: &str) {
        self.messages.borrow_mut().push(msg.to_owned());
    }
    fn show_info(&self, msg: &str) {
        self.messages.borrow_mut().push(msg.to_owned());
    }
}

// ── 테스트 ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_error_format() {
        let msg = error_message(&AppError::NoInput);
        assert!(msg.contains("선택한 항목이 없습니다"), "메시지: {msg}");
    }

    #[test]
    fn message_renamed_none_when_empty() {
        assert_eq!(renamed_message(&[]), None);
    }

    #[test]
    fn message_renamed_some_when_nonempty() {
        let result = renamed_message(&[("a.txt".into(), "a (1).txt".into())]);
        assert!(result.is_some());
    }

    #[test]
    fn message_renamed_contains_both_names() {
        let msg = renamed_message(&[("a.txt".into(), "a (1).txt".into())]).unwrap();
        assert!(msg.contains("a.txt"), "메시지: {msg}");
        assert!(msg.contains("a (1).txt"), "메시지: {msg}");
    }

    #[test]
    fn message_skipped_none_when_empty() {
        assert_eq!(skipped_message(&[]), None);
    }

    #[test]
    fn message_skipped_some_when_nonempty() {
        let result =
            skipped_message(&[(PathBuf::from("link.txt"), SkipReason::SymlinkOrJunction)]);
        assert!(result.is_some());
    }

    #[test]
    fn message_skipped_contains_path() {
        let msg =
            skipped_message(&[(PathBuf::from("link.txt"), SkipReason::SymlinkOrJunction)])
                .unwrap();
        assert!(msg.contains("link.txt"), "메시지: {msg}");
    }

    #[test]
    fn message_multi_failure_contains_all() {
        let failures = vec![
            (PathBuf::from("a.zip"), AppError::NoInput),
            (PathBuf::from("b.zip"), AppError::NoInput),
        ];
        let msg = multi_failure_message(&failures);
        assert!(msg.contains("a.zip"), "메시지: {msg}");
        assert!(msg.contains("b.zip"), "메시지: {msg}");
    }

    #[test]
    fn capture_sink_records_message() {
        let sink = CaptureMessageBox::new();
        sink.show_error("테스트 오류");
        let msgs = sink.messages.borrow();
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("테스트 오류"), "기록된 메시지: {:?}", msgs[0]);
    }
}
