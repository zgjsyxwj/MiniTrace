#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod backend;
pub mod commands;
pub mod diagnostics;
pub mod disk;
pub mod macos_backend;
pub mod model;
pub mod recovery;
pub mod storage;
pub mod windows_backend;

use backend::TauriEventSink;
use commands::AppState;
use std::sync::Arc;
use tauri::{Manager, Wry};

pub fn run() {
    tauri::Builder::<Wry>::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|error| format!("解析应用配置目录失败：{error}"))?;
            app.state::<AppState>().configure(
                config_dir.join("workspace.json"),
                Arc::new(TauriEventSink {
                    app: app.handle().clone(),
                }),
            )?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::set_workspace,
            commands::pick_workspace,
            commands::diagnose_environment,
            commands::list_candidates,
            commands::start_capture,
            commands::stop_capture,
            commands::report_target_process_exit,
            commands::refresh_recovery_candidates,
            commands::select_recovery_candidate,
            commands::get_capture_recovery,
            commands::list_sessions,
            commands::get_session,
            commands::read_raw_bytes,
            commands::rebuild_session_index,
            commands::open_session_directory,
        ])
        .run(tauri::generate_context!())
        .expect("运行 MiniTrace Tauri 应用失败");
}
