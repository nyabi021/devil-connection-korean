use std::path::{Path, PathBuf};

const GAME_DIR: &str = "でびるコネクショん";

pub fn find_app_asar(game_path: &Path) -> Option<PathBuf> {
    let candidates: [PathBuf; 2] = [
        game_path.join("DevilConnection.app/Contents/Resources/app.asar"),
        game_path.join("resources/app.asar"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

pub fn auto_detect() -> Option<PathBuf> {
    for root in steam_roots() {
        let candidate = root.join("steamapps/common").join(GAME_DIR);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn steam_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ];
    for drive in other_drives() {
        roots.push(PathBuf::from(format!(r"{drive}:\Steam")));
        roots.push(PathBuf::from(format!(r"{drive}:\Program Files (x86)\Steam")));
        roots.push(PathBuf::from(format!(r"{drive}:\Program Files\Steam")));
        roots.push(PathBuf::from(format!(r"{drive}:\SteamLibrary")));
    }
    roots
}

#[cfg(target_os = "windows")]
fn other_drives() -> Vec<char> {
    use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
    let bitmask = unsafe { GetLogicalDrives() };
    (0..26u32)
        .filter(|i| bitmask & (1 << i) != 0)
        .map(|i| (b'A' + i as u8) as char)
        .filter(|&c| c != 'C')
        .collect()
}

#[cfg(target_os = "macos")]
fn steam_roots() -> Vec<PathBuf> {
    let home = dirs_home();
    vec![home.join("Library/Application Support/Steam")]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn steam_roots() -> Vec<PathBuf> {
    let home = dirs_home();
    vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
    ]
}

#[cfg(unix)]
fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}
