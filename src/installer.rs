use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crossbeam_channel::Sender;
use include_dir::{Dir, DirEntry, include_dir};

use crate::asar;
use crate::config::PATCH_DIRS;
use crate::steam::find_app_asar;

static PATCHES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/patches");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub enum InstallEvent {
    Log(LogLevel, String),
    Progress(u32),
    Finished { success: bool, message: String },
}

pub struct InstallHandle {
    cancel: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl InstallHandle {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn join(&mut self) {
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }
}

pub fn spawn(game_path: PathBuf, tx: Sender<InstallEvent>) -> InstallHandle {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_thread = Arc::clone(&cancel);
    let join = thread::spawn(move || {
        run(game_path, tx, cancel_thread);
    });
    InstallHandle {
        cancel,
        join: Some(join),
    }
}

fn run(game_path: PathBuf, tx: Sender<InstallEvent>, cancel: Arc<AtomicBool>) {
    let log = |level: LogLevel, msg: &str| {
        let _ = tx.send(InstallEvent::Log(level, msg.to_string()));
    };
    let progress = |p: u32| {
        let _ = tx.send(InstallEvent::Progress(p));
    };

    let mut state = RestoreState::default();
    let cancel_fn: Box<dyn Fn() -> bool + Send + Sync> = {
        let cancel = Arc::clone(&cancel);
        Box::new(move || cancel.load(Ordering::SeqCst))
    };

    match run_inner(&game_path, &log, &progress, &*cancel_fn, &mut state) {
        Ok(msg) => {
            let _ = tx.send(InstallEvent::Finished {
                success: true,
                message: msg,
            });
        }
        Err(err) => {
            let cancelled = cancel.load(Ordering::SeqCst);
            if cancelled {
                log(LogLevel::Warning, "설치가 취소되었습니다. 원본 파일을 복원 중...");
            } else {
                log(LogLevel::Error, "============================================================");
                log(LogLevel::Error, &format!("설치 중 오류 발생: {}", err));
                log(LogLevel::Error, "============================================================");
            }
            state.restore(&log);
            let msg = if cancelled {
                "설치가 취소되었습니다.\n원본 파일이 복원되었습니다.".to_string()
            } else {
                format!("설치 중 오류가 발생했습니다:\n\n{}", err)
            };
            let _ = tx.send(InstallEvent::Finished {
                success: false,
                message: msg,
            });
        }
    }
}

#[derive(Default)]
struct RestoreState {
    asar_path: Option<PathBuf>,
    backup_path: Option<PathBuf>,
    app_folder: Option<PathBuf>,
}

impl RestoreState {
    fn restore(&self, log: &dyn Fn(LogLevel, &str)) {
        if let Some(app) = &self.app_folder {
            if app.exists() {
                let _ = fs::remove_dir_all(app);
            }
        }
        if let (Some(backup), Some(asar)) = (&self.backup_path, &self.asar_path) {
            if backup.exists() {
                match fs::copy(backup, asar) {
                    Ok(_) => log(LogLevel::Success, "원본 파일 복원 완료"),
                    Err(e) => log(LogLevel::Error, &format!("복원 중 오류: {}", e)),
                }
            }
        }
    }
}

fn run_inner(
    game_path: &Path,
    log: &dyn Fn(LogLevel, &str),
    progress: &dyn Fn(u32),
    cancel: &(dyn Fn() -> bool + Send + Sync),
    state: &mut RestoreState,
) -> Result<String, String> {
    log(LogLevel::Info, "============================================================");
    log(LogLevel::Info, "설치를 시작합니다...");
    log(LogLevel::Info, "1단계: app.asar 파일 찾기...");

    let asar_path = find_app_asar(game_path)
        .ok_or_else(|| "app.asar 파일을 찾을 수 없습니다. 게임 경로를 확인해주세요.".to_string())?;
    state.asar_path = Some(asar_path.clone());
    log(LogLevel::Success, &format!("app.asar 파일 위치: {}", asar_path.display()));
    progress(5);
    check_cancel(cancel)?;

    let resources_dir = asar_path.parent().ok_or("invalid asar path")?.to_path_buf();
    let app_folder = resources_dir.join("app");
    let backup_path = resources_dir.join("app.asar.backup");
    state.app_folder = Some(app_folder.clone());
    state.backup_path = Some(backup_path.clone());

    log(LogLevel::Info, "2단계: 원본 파일 백업...");
    if backup_path.exists() {
        log(LogLevel::Info, "백업 파일이 이미 존재합니다. 기존 백업을 유지합니다.");
    } else {
        fs::copy(&asar_path, &backup_path).map_err(|e| format!("백업 실패: {}", e))?;
        log(LogLevel::Success, "백업 완료");
    }
    progress(15);
    check_cancel(cancel)?;

    log(LogLevel::Info, "3단계: 기존 패치 파일 정리...");
    if app_folder.exists() {
        log(LogLevel::Info, "기존 app 폴더를 삭제합니다...");
        fs::remove_dir_all(&app_folder).map_err(|e| format!("삭제 실패: {}", e))?;
        log(LogLevel::Success, "삭제 완료");
    }
    progress(20);
    check_cancel(cancel)?;

    log(LogLevel::Info, "4단계: app.asar 압축 해제 중... (시간이 걸릴 수 있습니다)");
    asar::extract_archive(&asar_path, &app_folder, &cancel)
        .map_err(|e| format!("압축 해제 실패: {}", e))?;
    log(LogLevel::Success, "압축 해제 완료");
    progress(40);
    check_cancel(cancel)?;

    log(LogLevel::Info, "5단계: 번역 파일 복사 중...");
    let valid_dirs: Vec<&str> = PATCH_DIRS
        .iter()
        .copied()
        .filter(|d| PATCHES.get_dir(*d).is_some())
        .collect();
    let total = valid_dirs.len().max(1) as u32;
    for (i, dir_name) in valid_dirs.iter().enumerate() {
        check_cancel(cancel)?;
        log(LogLevel::Info, &format!("  - {} 폴더 복사 중...", dir_name));
        let src = PATCHES
            .get_dir(*dir_name)
            .ok_or_else(|| format!("번역 폴더 없음: {}", dir_name))?;
        let dst = app_folder.join(dir_name);
        write_embedded_dir(src, &dst, cancel).map_err(|e| format!("번역 복사 실패: {}", e))?;
        log(LogLevel::Success, &format!("  - {} 복사 완료", dir_name));
        let pct = 40 + ((i as u32 + 1) * 40 / total).min(40);
        progress(pct);
    }
    check_cancel(cancel)?;

    log(LogLevel::Info, "6단계: app 폴더를 app.asar로 재압축 중... (시간이 걸릴 수 있습니다)");
    if asar_path.exists() && asar_path.is_file() {
        fs::remove_file(&asar_path).map_err(|e| format!("원본 삭제 실패: {}", e))?;
        log(LogLevel::Info, "원본 app.asar 파일을 삭제했습니다.");
    }
    let unpack_patterns: &[&str] = &["*.node", "*.dll", "*.so", "*.dylib"];
    asar::create_archive(&app_folder, &asar_path, unpack_patterns, &cancel)
        .map_err(|e| format!("재압축 실패: {}", e))?;
    log(LogLevel::Success, "app.asar 재압축 완료");
    progress(90);

    log(LogLevel::Info, "7단계: 임시 파일 정리 중...");
    if app_folder.exists() {
        fs::remove_dir_all(&app_folder).map_err(|e| format!("임시 폴더 삭제 실패: {}", e))?;
        log(LogLevel::Success, "app 폴더를 삭제했습니다.");
    }
    progress(100);

    log(LogLevel::Info, "============================================================");
    log(LogLevel::Success, "한글패치가 완료되었습니다!");
    log(LogLevel::Success, "Steam에서 게임을 실행하면 한글로 플레이하실 수 있습니다.");

    if cfg!(target_os = "macos") {
        log(LogLevel::Info, "");
        log(LogLevel::Warning, "macOS 사용자 안내:");
        log(LogLevel::Info, "게임 실행 시 '손상되었습니다' 경고가 나타날 수 있습니다.");
        log(LogLevel::Info, "이는 정상적인 macOS 보안 경고이며, 다음과 같이 해결하세요:");
        log(LogLevel::Info, "1. 시스템 설정 > 개인정보 보호 및 보안 열기");
        log(LogLevel::Info, "2. '그래도 열기' 버튼 클릭");
    }

    log(LogLevel::Info, "============================================================");

    state.asar_path = None;
    state.backup_path = None;
    state.app_folder = None;

    Ok(complete_message())
}

fn complete_message() -> String {
    if cfg!(target_os = "macos") {
        "한글패치가 완료되었습니다!\n\n\
         Steam에서 게임을 실행하시면 됩니다.\n\n\
         '손상되었습니다' 경고가 나타나면:\n\
         시스템 설정 > 개인정보 보호 및 보안\n\
         에서 '그래도 열기' 버튼을 클릭하세요."
            .to_string()
    } else {
        "한글패치가 완료되었습니다!\n\n\
         Steam에서 게임을 실행하면 한글로 플레이하실 수 있습니다."
            .to_string()
    }
}

fn check_cancel(cancel: &(dyn Fn() -> bool + Send + Sync)) -> Result<(), String> {
    if cancel() {
        Err("cancelled".to_string())
    } else {
        Ok(())
    }
}

fn write_embedded_dir(
    dir: &Dir<'_>,
    dst: &Path,
    cancel: &(dyn Fn() -> bool + Send + Sync),
) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in dir.entries() {
        if cancel() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }
        let name = entry
            .path()
            .file_name()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no filename"))?;
        let out = dst.join(name);
        match entry {
            DirEntry::Dir(d) => write_embedded_dir(d, &out, cancel)?,
            DirEntry::File(f) => {
                if let Some(parent) = out.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&out, f.contents())?;
            }
        }
    }
    Ok(())
}
