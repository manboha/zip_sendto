mod common;

use std::fs;
use zip_sendto::compress::compress;
use zip_sendto::error::AppError;
use zip_sendto::extract::extract;
use zip_sendto::path::next_available;

// ── 기본 라운드트립 ──────────────────────────────────────────────────────────

#[test]
fn roundtrip_single_file() {
    let tmp = common::make_tempdir();
    let src = tmp.join("hello.txt");
    fs::write(&src, b"hello world content").unwrap();

    let zip_path = tmp.join("out.zip");
    compress(&[src], &zip_path).unwrap();

    let target = tmp.join("target");
    fs::create_dir_all(&target).unwrap();
    extract(&zip_path, &target).unwrap();

    assert_eq!(
        fs::read(target.join("hello.txt")).unwrap(),
        b"hello world content"
    );
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn roundtrip_single_folder() {
    let tmp = common::make_tempdir();
    let src = tmp.join("src_folder");
    fs::create_dir_all(&src).unwrap();
    common::setup_test_tree(&src);

    let zip_path = tmp.join("out.zip");
    compress(std::slice::from_ref(&src), &zip_path).unwrap();

    let target = tmp.join("target");
    fs::create_dir_all(&target).unwrap();
    extract(&zip_path, &target).unwrap();

    common::assert_dir_trees_equal(&src, &target.join("src_folder"));
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn roundtrip_multi_items() {
    let tmp = common::make_tempdir();
    let file1 = tmp.join("file1.txt");
    let file2 = tmp.join("file2.txt");
    let folder = tmp.join("folder");
    fs::write(&file1, b"content1").unwrap();
    fs::write(&file2, b"content2").unwrap();
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("file3.txt"), b"content3").unwrap();

    let zip_path = tmp.join("out.zip");
    compress(&[file1, file2, folder], &zip_path).unwrap();

    let target = tmp.join("target");
    fs::create_dir_all(&target).unwrap();
    extract(&zip_path, &target).unwrap();

    assert_eq!(fs::read(target.join("file1.txt")).unwrap(), b"content1");
    assert_eq!(fs::read(target.join("file2.txt")).unwrap(), b"content2");
    assert_eq!(
        fs::read(target.join("folder").join("file3.txt")).unwrap(),
        b"content3"
    );
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn roundtrip_empty_folder_preserved() {
    let tmp = common::make_tempdir();
    let src = tmp.join("src");
    fs::create_dir_all(src.join("empty_dir")).unwrap();
    fs::write(src.join("file.txt"), b"content").unwrap();

    let zip_path = tmp.join("out.zip");
    compress(&[src], &zip_path).unwrap();

    let target = tmp.join("target");
    fs::create_dir_all(&target).unwrap();
    extract(&zip_path, &target).unwrap();

    assert!(target.join("src").join("empty_dir").is_dir(), "빈 폴더가 보존되지 않음");
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn roundtrip_unicode_paths() {
    let tmp = common::make_tempdir();
    let src = tmp.join("한글폴더");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("한글파일.txt"), "한글 내용".as_bytes()).unwrap();

    let zip_path = tmp.join("out.zip");
    compress(&[src], &zip_path).unwrap();

    let target = tmp.join("target");
    fs::create_dir_all(&target).unwrap();
    extract(&zip_path, &target).unwrap();

    let extracted = target.join("한글폴더").join("한글파일.txt");
    assert!(extracted.exists(), "한글 파일명 보존 실패");
    assert_eq!(fs::read(&extracted).unwrap(), "한글 내용".as_bytes());
    fs::remove_dir_all(&tmp).ok();
}

// ── 스킵 ────────────────────────────────────────────────────────────────────

#[test]
fn roundtrip_skips_symlink() {
    let tmp = common::make_tempdir();
    let src = tmp.join("src");
    fs::create_dir_all(&src).unwrap();
    let real_file = src.join("real.txt");
    fs::write(&real_file, b"content").unwrap();
    let link = src.join("link.txt");

    if !common::try_create_symlink(&real_file, &link) {
        // 심볼릭 링크 생성 권한 없음 — 환경 제약으로 생략
        fs::remove_dir_all(&tmp).ok();
        return;
    }

    let zip_path = tmp.join("out.zip");
    let report = compress(&[src], &zip_path).unwrap();
    assert_eq!(report.skipped.len(), 1, "심볼릭 링크가 skipped에 기록되어야 함");

    let target = tmp.join("target");
    fs::create_dir_all(&target).unwrap();
    extract(&zip_path, &target).unwrap();

    assert!(!target.join("src").join("link.txt").exists(), "링크가 zip에 포함되어서는 안 됨");
    assert!(target.join("src").join("real.txt").exists());
    fs::remove_dir_all(&tmp).ok();
}

// ── 충돌 및 리네임 ──────────────────────────────────────────────────────────

#[test]
fn roundtrip_extract_here_conflict() {
    let tmp = common::make_tempdir();

    // 충돌할 파일을 미리 생성
    fs::write(tmp.join("a.txt"), b"existing").unwrap();

    let zip_path = tmp.join("test.zip");
    common::make_zip_with_entries(&zip_path, &[("a.txt", b"new")]);

    // "여기에 풀기" — zip이 위치한 폴더(tmp)에 그대로 해제
    let report = extract(&zip_path, &tmp).unwrap();

    assert!(tmp.join("a (1).txt").exists(), "충돌 파일이 리네임되어야 함");
    assert_eq!(report.renamed.len(), 1);
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn roundtrip_new_subdir_conflict() {
    let tmp = common::make_tempdir();
    let src_file = tmp.join("mydata.txt");
    fs::write(&src_file, b"content").unwrap();

    let zip_path = tmp.join("mydata.zip");
    compress(&[src_file], &zip_path).unwrap();

    // "새 폴더에 풀기" — 목표 폴더가 이미 존재
    let initial_target = tmp.join("mydata");
    fs::create_dir_all(&initial_target).unwrap();

    // 호출자가 next_available로 빈 폴더명을 확정
    let target = next_available(&initial_target).unwrap();
    assert_ne!(target, initial_target, "충돌 시 다른 이름을 사용해야 함");

    fs::create_dir_all(&target).unwrap();
    let report = extract(&zip_path, &target).unwrap();

    assert!(target.join("mydata.txt").exists());
    assert!(report.renamed.is_empty());
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn roundtrip_output_zip_conflict() {
    let tmp = common::make_tempdir();
    let src = tmp.join("a.txt");
    fs::write(&src, b"hello").unwrap();

    // 출력 zip이 이미 존재
    fs::write(tmp.join("a.zip"), b"dummy").unwrap();

    let initial = tmp.join("a.zip");
    let output = next_available(&initial).unwrap();
    assert_eq!(output, tmp.join("a (1).zip"));

    compress(&[src], &output).unwrap();
    assert!(output.exists());
    fs::remove_dir_all(&tmp).ok();
}

// ── 보안 ────────────────────────────────────────────────────────────────────

#[test]
fn roundtrip_zip_slip() {
    let tmp = common::make_tempdir();
    let zip_path = tmp.join("evil.zip");
    let target = tmp.join("out");
    fs::create_dir_all(&target).unwrap();

    common::make_malicious_zip(&zip_path);

    assert!(
        matches!(extract(&zip_path, &target), Err(AppError::UnsafePath(_))),
        "Zip Slip 공격이 차단되어야 함"
    );
    fs::remove_dir_all(&tmp).ok();
}

// ── RenameLimit ─────────────────────────────────────────────────────────────

#[test]
fn roundtrip_rename_limit_compress() {
    let tmp = common::make_tempdir();

    // "a.zip" ~ "a (999).zip" 1000개를 모두 채움
    common::fill_rename_slots(&tmp, "a", "zip", 1000);

    let initial = tmp.join("a.zip");
    assert!(
        matches!(next_available(&initial), Err(AppError::RenameLimit(_))),
        "999 초과 시 RenameLimit 오류를 반환해야 함"
    );
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn roundtrip_rename_limit_extract() {
    let tmp = common::make_tempdir();
    let target = tmp.join("target");
    fs::create_dir_all(&target).unwrap();

    // "file.txt" ~ "file (999).txt" 1000개를 모두 채움
    common::fill_rename_slots(&target, "file", "txt", 1000);

    let zip_path = tmp.join("test.zip");
    common::make_zip_with_entries(&zip_path, &[("file.txt", b"new")]);

    assert!(
        matches!(extract(&zip_path, &target), Err(AppError::RenameLimit(_))),
        "999 초과 시 RenameLimit 오류를 반환해야 함"
    );
    fs::remove_dir_all(&tmp).ok();
}
