#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::{path::PathBuf, process::Command};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
struct MediaInfo { path: String, name: String, duration: f64 }

#[tauri::command]
async fn import_media(app: tauri::AppHandle) -> Result<Option<MediaInfo>, String> {
    let Some(path) = app.dialog().file().add_filter("Media", &["mp4", "mov", "mkv", "avi", "webm", "wav", "mp3", "m4a"]).blocking_pick_file() else { return Ok(None); };
    let path: PathBuf = path.into_path().map_err(|e| e.to_string())?;
    let path_string = path.to_string_lossy().to_string();
    let output = Command::new("ffprobe").args(["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1"]).arg(&path).output();
    let duration = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(8.0),
        _ => 8.0,
    };
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Media").to_string();
    Ok(Some(MediaInfo { path: path_string, name, duration: duration.max(0.25) }))
}

#[tauri::command]
fn set_preview_speed(speed: f32) -> Result<f32, String> {
    if [0.25_f32, 0.5, 1.0].contains(&speed) { Ok(speed) } else { Err("Unsupported preview speed".into()) }
}

#[tauri::command]
fn window_action(app: tauri::AppHandle, action: String) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Main window is missing")?;
    match action.as_str() { "minimize" => window.minimize(), "toggle-maximize" => { if window.is_maximized().map_err(|e| e.to_string())? { window.unmaximize() } else { window.maximize() } }, "close" => window.close(), _ => return Err("Unknown window action".into()) }.map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default().plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![import_media, set_preview_speed, window_action])
        .run(tauri::generate_context!()).expect("Dango failed to start");
}
