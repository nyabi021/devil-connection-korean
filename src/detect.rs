//! Windows-only auto-detection of the Steam install of でびるコネクショん.

use std::path::{Path, PathBuf};

const GAME_DIR: &str = "でびるコネクショん";

pub fn find_game() -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ];

    for drive_letter in 'B'..='Z' {
        let d = format!("{drive_letter}:\\");
        if !Path::new(&d).exists() {
            continue;
        }
        for sub in ["Steam", "Program Files (x86)\\Steam", "Program Files\\Steam", "SteamLibrary"] {
            roots.push(PathBuf::from(format!("{d}{sub}")));
        }
    }

    roots
        .into_iter()
        .map(|r| r.join("steamapps").join("common").join(GAME_DIR))
        .find(|p| p.exists())
}

pub fn find_app_asar(game_path: &Path) -> Option<PathBuf> {
    let candidate = game_path.join("resources").join("app.asar");
    candidate.is_file().then_some(candidate)
}
