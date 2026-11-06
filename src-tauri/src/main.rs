#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::{fs, path::PathBuf, process::Command};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
struct MediaInfo {
    path: String,
    name: String,
    duration: f64,
    has_audio: bool,
}

#[derive(Serialize)]
struct ProjectDocument {
    path: String,
    contents: String,
}

#[tauri::command]
fn import_media(app: tauri::AppHandle) -> Result<Option<MediaInfo>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("Media", &["mp4", "mov", "mkv", "avi", "webm", "wav", "mp3", "m4a"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };

    let path: PathBuf = path.into_path().map_err(|error| error.to_string())?;
    let path_string = path.to_string_lossy().to_string();
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_type",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(&path)
        .output();
    let (duration, has_audio) = match probe {
        Ok(result) if result.status.success() => {
            let output = String::from_utf8_lossy(&result.stdout);
            let mut duration = 8.0;
            let mut has_audio = false;
            for value in output.lines().map(str::trim) {
                has_audio |= value == "audio";
                if let Ok(parsed) = value.parse::<f64>() {
                    duration = parsed;
                }
            }
            (duration.max(0.25), has_audio)
        }
        _ => (8.0, false),
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Media")
        .to_string();

    Ok(Some(MediaInfo {
        path: path_string,
        name,
        duration,
        has_audio,
    }))
}

#[tauri::command]
fn save_project(
    app: tauri::AppHandle,
    contents: String,
    path: Option<String>,
) -> Result<Option<String>, String> {
    let path = match path {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => {
            let Some(path) = app
                .dialog()
                .file()
                .add_filter("Dango project", &["dango"])
                .set_file_name("Untitled.dango")
                .blocking_save_file()
            else {
                return Ok(None);
            };
            path.into_path().map_err(|error| error.to_string())?
        }
    };

    fs::write(&path, contents).map_err(|error| error.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
fn open_project(app: tauri::AppHandle) -> Result<Option<ProjectDocument>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("Dango project", &["dango"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };

    let path = path.into_path().map_err(|error| error.to_string())?;
    let contents = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    Ok(Some(ProjectDocument {
        path: path.to_string_lossy().to_string(),
        contents,
    }))
}

#[tauri::command]
fn window_action(app: tauri::AppHandle, action: String) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window is missing")?;
    let result = match action.as_str() {
        "minimize" => window.minimize(),
        "toggle-maximize" => {
            if window.is_maximized().map_err(|error| error.to_string())? {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        "close" => window.close(),
        _ => return Err("Unknown window action".into()),
    };

    result.map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            import_media,
            save_project,
            open_project,
            window_action
        ])
        .run(tauri::generate_context!())
        .expect("Dango failed to start");
}
