mod asar;
mod installer;
mod steam;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Emitter;
use tauri::Manager;
use tauri::ipc::Channel;

use installer::InstallEvent;

#[derive(Default)]
struct AppState {
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    install_in_progress: AtomicBool,
    close_after_install: AtomicBool,
}

#[tauri::command]
async fn auto_detect_game_path() -> Option<String> {
    tokio::task::spawn_blocking(|| steam::auto_detect().map(|p| p.display().to_string()))
        .await
        .unwrap_or(None)
}

#[tauri::command]
fn validate_game_path(path: String) -> bool {
    steam::is_valid_game_dir(&PathBuf::from(path))
}

#[tauri::command]
async fn start_install(
    game_path: String,
    on_event: Channel<InstallEvent>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let path = PathBuf::from(&game_path);
    if !steam::is_valid_game_dir(&path) {
        return Err("올바른 Devil Connection 게임 폴더를 찾을 수 없습니다.".to_string());
    }
    if steam::is_game_running(&path) {
        return Err(
            "게임이 현재 실행 중입니다.\n게임을 완전히 종료한 뒤 다시 시도해주세요.".to_string(),
        );
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state.cancel.lock().map_err(|e| e.to_string())?;
        if state.install_in_progress.load(Ordering::SeqCst) {
            return Err("이미 설치가 진행 중입니다.".to_string());
        }
        *guard = Some(Arc::clone(&cancel));
    }
    state.install_in_progress.store(true, Ordering::SeqCst);
    state.close_after_install.store(false, Ordering::SeqCst);

    let channel = on_event;
    let result = tokio::task::spawn_blocking(move || {
        installer::run_install(path, channel, cancel);
    })
    .await
    .map_err(|e| format!("설치 작업 실패: {}", e));

    {
        let mut guard = state.cancel.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }
    state.install_in_progress.store(false, Ordering::SeqCst);
    let should_close = state.close_after_install.swap(false, Ordering::SeqCst);

    if should_close {
        app.exit(0);
    }

    result
}

#[tauri::command]
fn cancel_install(close_after: Option<bool>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    if close_after.unwrap_or(false) {
        state.close_after_install.store(true, Ordering::SeqCst);
    }
    let guard = state.cancel.lock().map_err(|e| e.to_string())?;
    if let Some(flag) = guard.as_ref() {
        flag.store(true, Ordering::SeqCst);
    }
    Ok(())
}

fn notify_close_requested(window: &tauri::Window) {
    let _ = window.emit("install-close-requested", ());
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState::default());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let state = window.state::<AppState>();
                if state.install_in_progress.load(Ordering::SeqCst) {
                    api.prevent_close();
                    notify_close_requested(window);
                }
            }
            tauri::WindowEvent::Destroyed => {
                window.app_handle().exit(0);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            auto_detect_game_path,
            validate_game_path,
            start_install,
            cancel_install,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
