use std::sync::Mutex;

use tauri::{generate_context, generate_handler, State};

use shufflekeys::engine::config::AppConfig;

struct AppState {
    engine_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[tauri::command]
fn get_status() -> bool {
    shufflekeys::is_running()
}

#[tauri::command]
fn check_permissions() -> bool {
    shufflekeys::platform::is_event_tap_enabled()
}

#[tauri::command]
fn start_engine(state: State<AppState>) -> Result<String, String> {
    if shufflekeys::is_running() {
        return Ok("Already running".into());
    }

    shufflekeys::restart();
    let mut handle = state
        .engine_handle
        .lock()
        .map_err(|e| format!("Failed to acquire lock: {e}"))?;

    // Use a channel to catch immediate startup errors (like permission issues)
    let (tx, rx) = std::sync::mpsc::channel();

    let join_handle = std::thread::spawn(move || {
        let cfg = AppConfig::load().unwrap_or_else(|e| {
            log::error!("Failed to load config, using defaults: {e}");
            AppConfig::default()
        });
        
        // Try to start the engine. If it fails immediately, send the error back.
        match shufflekeys::run_engine(cfg, true, false) {
            Ok(_) => {
                let _ = tx.send(Ok(()));
            }
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
            }
        }
    });

    // Wait briefly for startup success/failure
    match rx.recv_timeout(std::time::Duration::from_millis(500)) {
        Ok(Err(e)) => {
            shufflekeys::stop();
            return Err(e);
        }
        Ok(Ok(())) => {}
        Err(_) => {
            // Timeout: assume it started or will fail later asynchronously
        }
    }

    *handle = Some(join_handle);
    Ok("Engine started".into())
}

#[tauri::command]
fn stop_engine() -> Result<String, String> {
    shufflekeys::stop();
    Ok("Engine stopped".into())
}

#[tauri::command]
fn get_config() -> Result<AppConfig, String> {
    AppConfig::load().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_config(config: AppConfig) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            engine_handle: Mutex::new(None),
        })
        .invoke_handler(generate_handler![
            get_status,
            check_permissions,
            start_engine,
            stop_engine,
            get_config,
            save_config
        ])
        .run(generate_context!())
        .expect("error while running tauri application");
}
