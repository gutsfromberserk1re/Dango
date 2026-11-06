#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
struct MediaInfo {
    path: String,
    name: String,
    duration: f64,
    has_audio: bool,
    kind: String,
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
        .add_filter("Media", &["mp4", "mov", "mkv", "avi", "webm", "wav", "mp3", "m4a", "png", "jpg", "jpeg", "gif", "bmp", "webp"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };

    let path: PathBuf = path.into_path().map_err(|error| error.to_string())?;
    let path_string = path.to_string_lossy().to_string();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kind = if ["png", "jpg", "jpeg", "gif", "bmp", "webp"].contains(&extension.as_str()) {
        "image"
    } else if ["wav", "mp3", "m4a"].contains(&extension.as_str()) {
        "audio"
    } else {
        "video"
    };
    if kind == "image" {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Image")
            .to_string();
        return Ok(Some(MediaInfo {
            path: path_string,
            name,
            duration: 5.0,
            has_audio: false,
            kind: kind.to_string(),
        }));
    }
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
        kind: kind.to_string(),
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

#[tauri::command]
fn export_video(app: tauri::AppHandle, contents: String, quality: String) -> Result<Option<String>, String> {
    let Some(path) = app.dialog().file().add_filter("MP4 video", &["mp4"])
        .set_file_name("Dango Export.mp4").blocking_save_file() else { return Ok(None); };
    let output = path.into_path().map_err(|e| e.to_string())?;
    let project: Value = serde_json::from_str(&contents).map_err(|e| e.to_string())?;
    let clips = project.get("clips").and_then(Value::as_array).ok_or("Project has no timeline")?;
    let mut visual: Vec<&Value> = clips.iter().filter(|c| matches!(c["kind"].as_str(), Some("video" | "image")) && c["titleText"].is_null() && c["path"].as_str().is_some()).collect();
    visual.sort_by(|a,b| a["track"].as_str().cmp(&b["track"].as_str()).then_with(|| a["start"].as_f64().partial_cmp(&b["start"].as_f64()).unwrap_or(std::cmp::Ordering::Equal)));
    if visual.is_empty() { return Err("Add a video or image clip before exporting".into()); }
    let duration = clips.iter().filter_map(|c| c["start"].as_f64().zip(c["duration"].as_f64()).map(|(s,d)|s+d)).fold(0.0_f64,f64::max);
    let mut cmd = Command::new("ffmpeg"); cmd.arg("-y");
    let mut audio_indices = Vec::new();
    for (i,c) in visual.iter().enumerate() {
        let d=c["duration"].as_f64().unwrap_or(1.0).max(0.05); let source=c["path"].as_str().unwrap();
        if c["kind"]=="image" { cmd.args(["-loop","1","-t"]); cmd.arg(d.to_string()); }
        else { let offset=c["sourceOffset"].as_f64().unwrap_or(0.0); if offset>0.0 {cmd.arg("-ss").arg(offset.to_string());} cmd.arg("-t").arg(d.to_string()); }
        cmd.arg("-i").arg(source);
        if c["kind"]=="video" && c["hasAudio"].as_bool().unwrap_or(false) { audio_indices.push((i,c["start"].as_f64().unwrap_or(0.0),d)); }
    }
    for (n,c) in clips.iter().filter(|c| c["kind"]=="audio" && c["path"].as_str().is_some()).enumerate() { let idx=visual.len()+n; let offset=c["sourceOffset"].as_f64().unwrap_or(0.0); let d=c["duration"].as_f64().unwrap_or(1.0).max(0.05); if offset>0.0 { cmd.arg("-ss").arg(offset.to_string()); } cmd.arg("-t").arg(d.to_string()).arg("-i").arg(c["path"].as_str().unwrap()); audio_indices.push((idx,c["start"].as_f64().unwrap_or(0.0),d)); }
    let mut graph=format!("color=c=black:s=1920x1080:r=30:d={duration}[base0]"); let mut last="base0".to_string();
    for (i,c) in visual.iter().enumerate() { let start=c["start"].as_f64().unwrap_or(0.0); let end=start+c["duration"].as_f64().unwrap_or(1.0); let next=format!("base{}",i+1); graph.push_str(&format!(";[{i}:v]scale=1920:1080:force_original_aspect_ratio=decrease,format=rgba,setsar=1,setpts=PTS-STARTPTS+{start}/TB[v{i}];[{last}][v{i}]overlay=(W-w)/2:(H-h)/2:enable='between(t,{start},{end})'[{next}]")); last=next; }
    let mut audio_labels=Vec::new();
    for (i,start,d) in audio_indices { let label=format!("a{i}"); graph.push_str(&format!(";[{i}:a]atrim=duration={d},asetpts=PTS-STARTPTS,adelay={}:all=1[{label}]",(start*1000.0) as u64)); audio_labels.push(format!("[{label}]")); }
    graph.push_str(&format!(";[{last}]format=yuv420p[vout]"));
    if !audio_labels.is_empty() { graph.push_str(&format!(";{}amix=inputs={}:duration=longest[aout]",audio_labels.concat(),audio_labels.len())); }
    cmd.arg("-filter_complex").arg(&graph).args(["-map","[vout]"]); if !audio_labels.is_empty(){cmd.args(["-map","[aout]"]);}else{cmd.arg("-an");}
    let crf=if quality=="high" {"18"} else if quality=="small" {"27"} else {"23"};
    cmd.args(["-c:v","libx264","-preset","veryfast","-crf",crf,"-pix_fmt","yuv420p","-movflags","+faststart"]);
    if !audio_labels.is_empty(){cmd.args(["-c:a","aac","-b:a","192k"]);}
    let result=cmd.arg("-t").arg(duration.to_string()).arg(&output).output().map_err(|e|format!("FFmpeg could not start: {e}. Install FFmpeg and add it to PATH."))?;
    if !result.status.success(){return Err(String::from_utf8_lossy(&result.stderr).lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"));}
    Ok(Some(output.to_string_lossy().into_owned()))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            import_media,
            save_project,
            open_project,
            export_video,
            window_action
        ])
        .run(tauri::generate_context!())
        .expect("Dango failed to start");
}
