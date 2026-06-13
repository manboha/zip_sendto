use std::path::PathBuf;

use zip_sendto::args::{parse_args, Mode};
use zip_sendto::compress::compress;
use zip_sendto::error::AppError;
use zip_sendto::extract::extract;
use zip_sendto::path::{next_available, output_zip_name};
use zip_sendto::ui::{
    error_message, multi_failure_message, renamed_message, skipped_message, MessageSink,
};

#[cfg(windows)]
use zip_sendto::ui::WindowsMessageBox;

fn main() {
    #[cfg(windows)]
    let sink = WindowsMessageBox;
    #[cfg(not(windows))]
    let sink = NoopSink;

    run(&sink);
}

fn run(sink: &impl MessageSink) {
    let raw: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();

    let (mode, items) = match parse_args(&raw) {
        Ok(v) => v,
        Err(e) => {
            sink.show_error(&error_message(&e));
            return;
        }
    };

    match mode {
        Mode::CompressByParent | Mode::CompressByName => {
            run_compress(sink, &mode, &items);
        }
        Mode::ExtractHere => {
            run_extract_here(sink, &items);
        }
        Mode::ExtractSubdir => {
            run_extract_subdir(sink, &items);
        }
    }
}

fn run_compress(sink: &impl MessageSink, mode: &Mode, items: &[PathBuf]) {
    let initial = match output_zip_name(mode, items) {
        Ok(p) => p,
        Err(e) => {
            sink.show_error(&error_message(&e));
            return;
        }
    };
    let output = match next_available(&initial) {
        Ok(p) => p,
        Err(e) => {
            sink.show_error(&error_message(&e));
            return;
        }
    };
    let report = match compress(items, &output) {
        Ok(r) => r,
        Err(e) => {
            sink.show_error(&error_message(&e));
            return;
        }
    };
    if let Some(msg) = skipped_message(&report.skipped) {
        sink.show_info(&msg);
    }
}

fn run_extract_here(sink: &impl MessageSink, items: &[PathBuf]) {
    let mut failures: Vec<(PathBuf, AppError)> = Vec::new();

    for zip_path in items {
        let target = match zip_path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => {
                failures.push((zip_path.clone(), AppError::InvalidPath(zip_path.clone())));
                continue;
            }
        };
        match extract(zip_path, target) {
            Ok(report) => {
                if let Some(msg) = renamed_message(&report.renamed) {
                    sink.show_info(&msg);
                }
            }
            Err(e) => failures.push((zip_path.clone(), e)),
        }
    }

    if !failures.is_empty() {
        sink.show_error(&multi_failure_message(&failures));
    }
}

fn run_extract_subdir(sink: &impl MessageSink, items: &[PathBuf]) {
    let mut failures: Vec<(PathBuf, AppError)> = Vec::new();

    for zip_path in items {
        let parent = match zip_path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => {
                failures.push((zip_path.clone(), AppError::InvalidPath(zip_path.clone())));
                continue;
            }
        };
        let stem = match zip_path.file_stem() {
            Some(s) => s,
            None => {
                failures.push((zip_path.clone(), AppError::InvalidPath(zip_path.clone())));
                continue;
            }
        };
        let initial_dir = parent.join(stem);
        let target = match next_available(&initial_dir) {
            Ok(p) => p,
            Err(e) => {
                failures.push((zip_path.clone(), e));
                continue;
            }
        };
        if let Err(e) = std::fs::create_dir_all(&target) {
            failures.push((zip_path.clone(), AppError::Io(e)));
            continue;
        }
        match extract(zip_path, &target) {
            Ok(report) => {
                if let Some(msg) = renamed_message(&report.renamed) {
                    sink.show_info(&msg);
                }
            }
            Err(e) => failures.push((zip_path.clone(), e)),
        }
    }

    if !failures.is_empty() {
        sink.show_error(&multi_failure_message(&failures));
    }
}

// ── 비-Windows 더미 (컴파일 유지용, 실제 사용 안 됨) ─────────────────────────

#[cfg(not(windows))]
struct NoopSink;

#[cfg(not(windows))]
impl MessageSink for NoopSink {
    fn show_error(&self, _msg: &str) {}
    fn show_info(&self, _msg: &str) {}
}
