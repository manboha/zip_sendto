pub const COMPRESS_LEVEL_HIGH: i64 = 6;
pub const COMPRESS_LEVEL_MED: i64 = 3;
pub const SIZE_THRESHOLD: u64 = 1_048_576; // 1 MB

pub const STORE_EXTENSIONS: &[&str] = &[
    // 이미지 (손실/무손실 모두)
    "jpg", "jpeg", "png", "gif", "webp", "avif", "heic", "heif",
    // 동영상
    "mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv",
    // 오디오
    "mp3", "aac", "flac", "ogg", "m4a", "wma", "opus",
    // 이미 압축된 아카이브
    "zip", "gz", "bz2", "xz", "zst", "7z", "rar", "br",
    // 문서/폰트 (내부적으로 압축됨)
    "docx", "xlsx", "pptx", "odt", "ods", "odp", "epub",
];
