//! Static configuration for the patcher. Values are compiled into the binary;
//! to change them, edit this file and rebuild with `cargo build --release`.

pub const APP_TITLE: &str = "でびるコネクショん 한글패치";
pub const WINDOW_WIDTH: f32 = 800.0;
pub const WINDOW_HEIGHT: f32 = 840.0;
pub const CREDITS: &str =
    "메인 시나리오 번역 검수: Ewan | 이미지 번역: 토니, 체퓨 | 영상 번역: 민버드";

/// Top-level directory names under `patches/` to copy into the extracted
/// `app/` folder during install. Missing directories are skipped silently.
pub const PATCH_DIRS: &[&str] = &[
    "data/scenario",
    "data/others",
    "data/system",
    "data/fgimage",
    "data/image",
    "data/video",
    "data/bgimage",
    "tyrano",
];
