//! The 7-step install flow, driven by a background thread.

use include_dir::{Dir, include_dir};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc::Sender;

use crate::config::PATCH_DIRS;
use crate::detect::find_app_asar;

/// Bundled patch assets. The directory must exist at build time; an empty
/// folder is acceptable for debug builds, the installer just copies whatever
/// is inside whose top-level names match `config.patch.dirs`.
static PATCHES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/patches");

#[derive(Debug, Clone)]
pub enum Message {
    Log { text: String, level: Level },
    Progress(u8),
    Finished { success: bool, message: String },
}

#[derive(Debug, Clone, Copy)]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

pub struct Installer {
    pub game_path: PathBuf,
    pub tx: Sender<Message>,
    pub cancel: Arc<AtomicBool>,
}

impl Installer {
    pub fn run(self) {
        let Self {
            game_path,
            tx,
            cancel,
        } = self;

        let mut asar_path: Option<PathBuf> = None;
        let mut backup_path: Option<PathBuf> = None;
        let mut app_folder: Option<PathBuf> = None;

        let result =
            run_inner(&game_path, &tx, &cancel, &mut asar_path, &mut backup_path, &mut app_folder);

        match result {
            Ok(()) => {
                let _ = tx.send(Message::Finished {
                    success: true,
                    message: "한글패치가 완료되었습니다!\n\nSteam에서 게임을 실행하면 한글로 플레이하실 수 있습니다."
                        .into(),
                });
            }
            Err(InstallError::Cancelled) => {
                log(&tx, "설치가 취소되었습니다. 원본 파일을 복원 중...", Level::Warning);
                restore_backup(asar_path.as_deref(), backup_path.as_deref(), app_folder.as_deref(), &tx);
                let _ = tx.send(Message::Finished {
                    success: false,
                    message: "설치가 취소되었습니다.\n원본 파일이 복원되었습니다.".into(),
                });
            }
            Err(InstallError::Other(e)) => {
                log(&tx, "=".repeat(60), Level::Error);
                log(&tx, format!("설치 중 오류 발생: {e}"), Level::Error);
                log(&tx, "=".repeat(60), Level::Error);
                let _ = tx.send(Message::Finished {
                    success: false,
                    message: format!("설치 중 오류가 발생했습니다:\n\n{e}"),
                });
            }
        }
    }
}

#[derive(Debug)]
enum InstallError {
    Cancelled,
    Other(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for InstallError {
    fn from(e: E) -> Self {
        InstallError::Other(e.into())
    }
}

fn check_cancel(cancel: &Arc<AtomicBool>) -> Result<(), InstallError> {
    if cancel.load(Ordering::Relaxed) {
        Err(InstallError::Cancelled)
    } else {
        Ok(())
    }
}

fn run_inner(
    game_path: &Path,
    tx: &Sender<Message>,
    cancel: &Arc<AtomicBool>,
    out_asar: &mut Option<PathBuf>,
    out_backup: &mut Option<PathBuf>,
    out_app_folder: &mut Option<PathBuf>,
) -> Result<(), InstallError> {
    log(tx, "=".repeat(60), Level::Info);
    log(tx, "설치를 시작합니다...", Level::Info);
    log(tx, "1단계: app.asar 파일 찾기...", Level::Info);

    let asar_path = find_app_asar(game_path).ok_or_else(|| {
        InstallError::Other(anyhow::anyhow!(
            "app.asar 파일을 찾을 수 없습니다. 게임 경로를 확인해주세요."
        ))
    })?;
    log(tx, format!("app.asar 파일 위치: {}", asar_path.display()), Level::Success);
    let _ = tx.send(Message::Progress(5));
    *out_asar = Some(asar_path.clone());
    check_cancel(cancel)?;

    let resources_dir = asar_path
        .parent()
        .ok_or_else(|| InstallError::Other(anyhow::anyhow!("resources 경로 계산 실패")))?
        .to_path_buf();
    let app_folder = resources_dir.join("app");
    let backup_path = resources_dir.join("app.asar.backup");
    *out_app_folder = Some(app_folder.clone());
    *out_backup = Some(backup_path.clone());

    log(tx, "2단계: 원본 파일 백업...", Level::Info);
    if backup_path.exists() {
        log(tx, "백업 파일이 이미 존재합니다. 기존 백업을 유지합니다.", Level::Info);
    } else {
        fs::copy(&asar_path, &backup_path)?;
        log(tx, "백업 완료", Level::Success);
    }
    let _ = tx.send(Message::Progress(15));
    check_cancel(cancel)?;

    log(tx, "3단계: 기존 패치 파일 정리...", Level::Info);
    if app_folder.exists() {
        log(tx, "기존 app 폴더를 삭제합니다...", Level::Info);
        fs::remove_dir_all(&app_folder)?;
        log(tx, "삭제 완료", Level::Success);
    }
    let _ = tx.send(Message::Progress(20));
    check_cancel(cancel)?;

    log(tx, "4단계: app.asar 압축 해제 중... (시간이 걸릴 수 있습니다)", Level::Info);
    {
        let mut progress = asar_progress(tx.clone(), cancel.clone(), 20, 40);
        crate::asar::extract(&asar_path, &app_folder, &mut progress)
            .map_err(map_asar_err)?;
    }
    log(tx, "압축 해제 완료", Level::Success);
    let _ = tx.send(Message::Progress(40));
    check_cancel(cancel)?;

    log(tx, "5단계: 번역 파일 복사 중...", Level::Info);
    let valid: Vec<&str> = PATCH_DIRS
        .iter()
        .copied()
        .filter(|d| PATCHES.get_dir(d).is_some())
        .collect();
    if valid.is_empty() {
        log(tx, "번들된 번역 파일이 없습니다. 빌드에 patches/ 폴더를 포함했는지 확인하세요.", Level::Warning);
    }
    let total = valid.len().max(1) as u32;
    for (i, dir_name) in valid.iter().enumerate() {
        check_cancel(cancel)?;
        log(tx, format!("  - {dir_name} 폴더 복사 중..."), Level::Info);
        let src = PATCHES
            .get_dir(*dir_name)
            .expect("validated above");
        let dst = app_folder.join(dir_name);
        write_embedded_dir(src, &dst, &app_folder)?;
        log(tx, format!("  - {dir_name} 복사 완료"), Level::Success);
        let pct = 40 + (40 * (i as u32 + 1) / total);
        let _ = tx.send(Message::Progress(pct.min(80) as u8));
    }
    check_cancel(cancel)?;

    log(tx, "6단계: app 폴더를 app.asar로 재압축 중... (시간이 걸릴 수 있습니다)", Level::Info);
    if asar_path.is_file() {
        fs::remove_file(&asar_path)?;
        log(tx, "원본 app.asar 파일을 삭제했습니다.", Level::Info);
    }
    {
        let mut progress = asar_progress(tx.clone(), cancel.clone(), 80, 90);
        crate::asar::pack(
            &app_folder,
            &asar_path,
            &crate::asar::PackOptions {
                unpack: Some("*.node".into()),
            },
            &mut progress,
        )
        .map_err(map_asar_err)?;
    }
    log(tx, "app.asar 재압축 완료", Level::Success);
    let _ = tx.send(Message::Progress(90));

    log(tx, "7단계: 임시 파일 정리 중...", Level::Info);
    if app_folder.exists() {
        fs::remove_dir_all(&app_folder)?;
        log(tx, "app 폴더를 삭제했습니다.", Level::Success);
    }
    let _ = tx.send(Message::Progress(100));

    log(tx, "=".repeat(60), Level::Info);
    log(tx, "한글패치가 완료되었습니다!", Level::Success);
    log(tx, "Steam에서 게임을 실행하면 한글로 플레이하실 수 있습니다.", Level::Success);
    log(tx, "=".repeat(60), Level::Info);
    Ok(())
}

fn map_asar_err(e: crate::asar::AsarError) -> InstallError {
    match e {
        crate::asar::AsarError::Cancelled => InstallError::Cancelled,
        other => InstallError::Other(anyhow::Error::from(other)),
    }
}

fn restore_backup(
    asar_path: Option<&Path>,
    backup_path: Option<&Path>,
    app_folder: Option<&Path>,
    tx: &Sender<Message>,
) {
    let result = (|| -> std::io::Result<()> {
        if let Some(app) = app_folder
            && app.exists()
        {
            fs::remove_dir_all(app)?;
        }
        if let (Some(backup), Some(asar)) = (backup_path, asar_path)
            && backup.exists()
        {
            fs::copy(backup, asar)?;
            log(tx, "원본 파일 복원 완료", Level::Success);
        }
        Ok(())
    })();
    if let Err(e) = result {
        log(tx, format!("복원 중 오류: {e}"), Level::Error);
    }
}

fn write_embedded_dir(
    src: &Dir<'_>,
    dst: &Path,
    _app_root: &Path,
) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for file in src.files() {
        let rel = file.path().strip_prefix(src.path()).unwrap_or(file.path());
        let out_path = dst.join(rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out_path, file.contents())?;
    }
    for subdir in src.dirs() {
        let rel = subdir.path().strip_prefix(src.path()).unwrap_or(subdir.path());
        let out_path = dst.join(rel);
        write_embedded_dir(subdir, &out_path, _app_root)?;
    }
    Ok(())
}

fn asar_progress<'a>(
    tx: Sender<Message>,
    cancel: Arc<AtomicBool>,
    start_pct: u8,
    end_pct: u8,
) -> crate::asar::Progress<'a> {
    let mut total_bytes: u64 = 1;
    let mut done: u64 = 0;
    crate::asar::Progress::new()
        .with_cancel(cancel)
        .with_callback(move |event| match event {
            crate::asar::ProgressEvent::Started { total_bytes: tb, .. } => {
                total_bytes = tb.max(1);
                done = 0;
                let _ = tx.send(Message::Progress(start_pct));
            }
            crate::asar::ProgressEvent::Bytes { delta } => {
                done += delta;
                let span = end_pct.saturating_sub(start_pct) as u64;
                let pct = start_pct as u64 + (span * done / total_bytes);
                let _ = tx.send(Message::Progress(pct.min(end_pct as u64) as u8));
            }
            _ => {}
        })
}

fn log(tx: &Sender<Message>, text: impl Into<String>, level: Level) {
    let _ = tx.send(Message::Log {
        text: text.into(),
        level,
    });
}
