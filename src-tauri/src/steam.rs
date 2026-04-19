use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GAME_DIR: &str = "でびるコネクショん";
const MAC_APP_DIR: &str = "DevilConnection.app";

pub fn is_valid_game_dir(game_path: &Path) -> bool {
    if game_path.file_name().and_then(|n| n.to_str()) != Some(GAME_DIR) {
        return false;
    }

    let has_expected_layout = game_path.join(MAC_APP_DIR).is_dir() || game_path.join("resources").is_dir();
    has_expected_layout && find_app_asar(game_path).is_some()
}

pub fn find_app_asar(game_path: &Path) -> Option<PathBuf> {
    let candidates: [PathBuf; 2] = [
        game_path.join(MAC_APP_DIR).join("Contents/Resources/app.asar"),
        game_path.join("resources/app.asar"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

pub fn auto_detect() -> Option<PathBuf> {
    for library in steam_libraries() {
        let candidate = library.join("steamapps/common").join(GAME_DIR);
        if is_valid_game_dir(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub fn is_game_running(game_path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        return windows_process_output()
            .map(|output| process_output_matches_game(&output, game_path))
            .unwrap_or(false);
    }

    #[cfg(unix)]
    {
        return unix_process_output()
            .map(|output| process_output_matches_game(&output, game_path))
            .unwrap_or(false);
    }

    #[allow(unreachable_code)]
    false
}

fn steam_libraries() -> Vec<PathBuf> {
    let mut libraries = BTreeSet::new();
    for root in steam_roots() {
        libraries.insert(root.clone());
        for library in read_libraryfolders(&root) {
            libraries.insert(library);
        }
    }
    libraries.into_iter().collect()
}

fn read_libraryfolders(root: &Path) -> Vec<PathBuf> {
    let library_file = root.join("steamapps/libraryfolders.vdf");
    let Ok(contents) = fs::read_to_string(library_file) else {
        return Vec::new();
    };

    contents
        .lines()
        .filter_map(|line| {
            let parts = quoted_fields(line);
            if parts.len() >= 2 && parts[0] == "path" {
                Some(PathBuf::from(unescape_vdf_path(&parts[1])))
            } else {
                None
            }
        })
        .collect()
}

fn quoted_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_quote = false;
    let mut current = String::new();

    for ch in line.chars() {
        if ch == '"' {
            if in_quote {
                out.push(current.clone());
                current.clear();
            }
            in_quote = !in_quote;
            continue;
        }
        if in_quote {
            current.push(ch);
        }
    }

    out
}

fn unescape_vdf_path(path: &str) -> String {
    path.replace("\\\\", "\\")
}

#[cfg(target_os = "windows")]
fn steam_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ];
    for drive in other_drives() {
        roots.push(PathBuf::from(format!(r"{drive}:\Steam")));
        roots.push(PathBuf::from(format!(
            r"{drive}:\Program Files (x86)\Steam"
        )));
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
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.steam/steam"),
        home.join("snap/steam/common/.local/share/Steam"),
        home.join("snap/steam/common/.steam/steam"),
    ]
}

#[cfg(unix)]
fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(unix)]
fn unix_process_output() -> Option<String> {
    let output = Command::new("ps")
        .args(["-axo", "command="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "windows")]
fn windows_process_output() -> Option<String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Process | Select-Object -ExpandProperty ExecutablePath",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn process_output_matches_game(output: &str, game_path: &Path) -> bool {
    let game_path = game_path.to_string_lossy().to_lowercase();
    if game_path.is_empty() {
        return false;
    }

    output.lines().any(|line| {
        let line = line.trim().to_lowercase();
        !line.is_empty() && line.contains(&game_path)
    })
}
