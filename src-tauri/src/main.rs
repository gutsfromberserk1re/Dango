#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaInfo {
    path: String,
    name: String,
    duration: f64,
    has_audio: bool,
    kind: String,
    thumbnail: Option<String>,
}

#[derive(Serialize)]
struct ProjectDocument {
    path: String,
    contents: String,
}

#[tauri::command]
async fn import_media(app: tauri::AppHandle) -> Result<Option<MediaInfo>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("Media", &["mp4", "mov", "mkv", "avi", "webm", "wav", "mp3", "m4a", "png", "jpg", "jpeg", "gif", "bmp", "webp"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };

    let path: PathBuf = path.into_path().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
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
            thumbnail: make_thumbnail_data(&app, &path).ok().flatten(),
        }));
    }
    let probe = Command::new(multimedia_tool(&app, "ffprobe"))
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
        thumbnail: make_thumbnail_data(&app, &path).ok().flatten(),
    }))
    }).await.map_err(|error| error.to_string())?
}

fn multimedia_tool(app: &tauri::AppHandle, tool: &str) -> PathBuf {
    let filename = if cfg!(windows) { format!("{tool}.exe") } else { tool.to_string() };
    if let Ok(directory) = std::env::var("DANGO_FFMPEG_DIR") {
        let candidate = PathBuf::from(directory).join(&filename);
        if candidate.is_file() { return candidate; }
    }
    let mut folders = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(folder) = executable.parent() {
            folders.push(folder.to_path_buf());
            folders.push(folder.join("ffmpeg"));
        }
    }
    if let Ok(resources) = app.path().resource_dir() {
        folders.push(resources.clone());
        folders.push(resources.join("ffmpeg"));
    }
    for folder in folders {
        let candidate = folder.join(&filename);
        if candidate.is_file() { return candidate; }
    }
    PathBuf::from(filename)
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 { TABLE[(((b & 15) << 2) | (c >> 6)) as usize] as char } else { '=' });
        output.push(if chunk.len() > 2 { TABLE[(c & 63) as usize] as char } else { '=' });
    }
    output
}

fn make_thumbnail_data(app: &tauri::AppHandle, path: &PathBuf) -> Result<Option<String>, String> {
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase();
    let mut command = Command::new(multimedia_tool(app, "ffmpeg"));
    command.args(["-hide_banner", "-loglevel", "error"]);
    if !["png", "jpg", "jpeg", "gif", "bmp", "webp"].contains(&extension.as_str()) {
        command.args(["-ss", "0.1"]);
    }
    let result = command.arg("-i").arg(path)
        .args(["-an", "-frames:v", "1", "-vf", "scale=160:90:force_original_aspect_ratio=increase,crop=160:90", "-f", "image2pipe", "-vcodec", "mjpeg", "pipe:1"])
        .output()
        .map_err(|error| error.to_string())?;
    if !result.status.success() || result.stdout.is_empty() { return Ok(None); }
    Ok(Some(format!("data:image/jpeg;base64,{}", encode_base64(&result.stdout))))
}

#[tauri::command]
async fn generate_thumbnail(app: tauri::AppHandle, path: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(path);
        if !path.is_file() { return Ok(None); }
        make_thumbnail_data(&app, &path)
    }).await.map_err(|error| error.to_string())?
}

struct ExportTempFiles(PathBuf);

impl Drop for ExportTempFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ffmpeg_filter_path(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

fn bundled_font(app: &tauri::AppHandle) -> Option<PathBuf> {
    let name = "ChunkypufflyRegular.otf";
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("fonts").join(name));
        }
    }
    if let Ok(resources) = app.path().resource_dir() {
        candidates.push(resources.join("fonts").join(name));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("frontend")
            .join("fonts")
            .join(name),
    );
    candidates.into_iter().find(|path| path.is_file())
}

fn filter_enable(start: f64, end: f64) -> String {
    format!("between(t,{start:.4},{end:.4})")
}

fn media_effect_filters(clips: &[Value], clip: &Value) -> Vec<String> {
    let clip_start = clip["start"].as_f64().unwrap_or(0.0);
    let clip_end = clip_start + clip["duration"].as_f64().unwrap_or(0.0);
    let clip_track = clip["track"]
        .as_str()
        .and_then(|track| track.strip_prefix('V'))
        .and_then(|rank| rank.parse::<u32>().ok())
        .unwrap_or(1);
    let mut filters = Vec::new();
    for effect in clips.iter().filter(|item| item["kind"] == "effect") {
        let effect_track = effect["track"]
            .as_str()
            .and_then(|track| track.strip_prefix('V'))
            .and_then(|rank| rank.parse::<u32>().ok())
            .unwrap_or(2);
        if effect_track <= clip_track || effect["effectName"] == "Cross dissolve" {
            continue;
        }
        let effect_start = effect["start"].as_f64().unwrap_or(0.0);
        let effect_end = effect_start + effect["duration"].as_f64().unwrap_or(0.0);
        let active_start = effect_start.max(clip_start);
        let active_end = effect_end.min(clip_end);
        if active_end <= active_start {
            continue;
        }
        let local_start = active_start - clip_start;
        let local_end = active_end - clip_start;
        let enable = filter_enable(local_start, local_end);
        let intensity = (effect["intensity"].as_f64().unwrap_or(50.0) / 100.0).clamp(0.0, 1.0);
        let name = effect["effectName"].as_str().unwrap_or("");
        let filter = match name {
            "Fade" => {
                let direction = effect["direction"].as_str().unwrap_or("in");
                let mode = if direction == "out" { "out" } else { "in" };
                format!("fade=t={mode}:st={local_start:.4}:d={:.4}", active_end - active_start)
            }
            "Blur" => format!("gblur=sigma={:.3}:enable='{enable}'", 1.0 + intensity * 9.0),
            "Vignette" => format!("vignette=angle={:.4}:enable='{enable}'", 0.18 + intensity * 0.72),
            "Warm film" => format!("eq=saturation={:.3}:gamma_r=1.05:gamma_b=0.92:enable='{enable}'", 1.0 + intensity * 0.35),
            "Sharpen" => format!("unsharp=5:5:{:.3}:5:5:0:enable='{enable}'", intensity * 1.5),
            "Soft glow" => format!("eq=brightness={:.3}:saturation={:.3}:enable='{enable}'", intensity * 0.08, 1.0 + intensity * 0.2),
            "Swirl" => format!("lenscorrection=k1={:.4}:k2={:.4}:enable='{enable}'", intensity * 0.18, intensity * 0.04),
            "Wave" => format!("lenscorrection=k1={:.4}:k2={:.4}:enable='{enable}'", -intensity * 0.12, intensity * 0.03),
            _ => continue,
        };
        filters.push(filter);
    }
    filters
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
async fn export_video(app: tauri::AppHandle, contents: String, quality: String) -> Result<Option<String>, String> {
    let Some(path) = app.dialog().file().add_filter("MP4 video", &["mp4"])
        .set_file_name("Dango Export.mp4").blocking_save_file() else { return Ok(None); };
    let output = path.into_path().map_err(|error| error.to_string())?;
    let result = tauri::async_runtime::spawn_blocking(move || render_export(app, contents, quality, output))
        .await.map_err(|error| format!("Export task failed: {error}"))??;
    Ok(Some(result))
}

fn render_export(app: tauri::AppHandle, contents: String, quality: String, output: PathBuf) -> Result<String, String> {
    let project: Value = serde_json::from_str(&contents).map_err(|error| error.to_string())?;
    let clips = project.get("clips").and_then(Value::as_array).ok_or("Project has no timeline")?;
    let (width, height) = match quality.as_str() {
        "480" => (854, 480),
        "1080" => (1920, 1080),
        _ => (1280, 720),
    };
    let mut visual: Vec<&Value> = clips.iter().filter(|clip| {
        matches!(clip["kind"].as_str(), Some("video" | "image"))
            && clip["titleText"].is_null()
            && clip["path"].as_str().is_some()
    }).collect();
    visual.sort_by(|a, b| {
        let rank = |clip: &&Value| clip["track"].as_str().and_then(|track| track.strip_prefix('V')).and_then(|rank| rank.parse::<u32>().ok()).unwrap_or(1);
        rank(a).cmp(&rank(b)).then_with(|| {
            a["start"].as_f64().partial_cmp(&b["start"].as_f64()).unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    let titles: Vec<&Value> = clips.iter().filter(|clip| clip["titleText"].as_str().is_some()).collect();
    let audio: Vec<&Value> = clips.iter().filter(|clip| clip["kind"] == "audio" && clip["path"].as_str().is_some()).collect();
    if visual.is_empty() && titles.is_empty() && audio.is_empty() {
        return Err("Add video, an image, audio, or text before exporting".into());
    }
    let duration = clips.iter().filter_map(|clip| {
        clip["start"].as_f64().zip(clip["duration"].as_f64()).map(|(start, length)| start + length)
    }).fold(0.0_f64, f64::max);
    if duration <= 0.0 {
        return Err("The timeline has no duration to export".into());
    }

    let mut command = Command::new(multimedia_tool(&app, "ffmpeg"));
    command.arg("-y");
    let mut audio_indices: Vec<(usize, f64, f64, f64, f64, f64)> = Vec::new();
    for (index, clip) in visual.iter().enumerate() {
        let clip_duration = clip["duration"].as_f64().unwrap_or(1.0).max(0.05);
        let source = clip["path"].as_str().ok_or("A video clip is missing its media path")?;
        if !PathBuf::from(source).is_file() {
            return Err(format!("Media file is missing: {source}"));
        }
        if clip["kind"] == "image" {
            command.args(["-loop", "1", "-framerate", "30", "-t"]).arg(clip_duration.to_string());
        } else {
            let offset = clip["sourceOffset"].as_f64().unwrap_or(0.0);
            if offset > 0.0 { command.arg("-ss").arg(offset.to_string()); }
            command.arg("-t").arg(clip_duration.to_string());
        }
        command.arg("-i").arg(source);
        if clip["kind"] == "video" && clip["hasAudio"].as_bool().or_else(|| clip["has_audio"].as_bool()).unwrap_or(false) {
            let id = clip["id"].as_str().unwrap_or("");
            let has_explicit_audio = clips.iter().any(|item| item["kind"] == "audio"
                && (item["sourceVideoId"].as_str() == Some(id) || item["linkedTo"].as_str() == Some(id)));
            if !has_explicit_audio {
                audio_indices.push((index, clip["start"].as_f64().unwrap_or(0.0), clip_duration, 1.0, 0.0, 0.0));
            }
        }
    }
    for (index, clip) in audio.iter().enumerate() {
        let input_index = visual.len() + index;
        let source = clip["path"].as_str().ok_or("An audio clip is missing its media path")?;
        if !PathBuf::from(source).is_file() {
            return Err(format!("Media file is missing: {source}"));
        }
        let source_offset = clip["sourceOffset"].as_f64().unwrap_or(0.0);
        let clip_duration = clip["duration"].as_f64().unwrap_or(1.0).max(0.05);
        if source_offset > 0.0 { command.arg("-ss").arg(source_offset.to_string()); }
        command.arg("-t").arg(clip_duration.to_string()).arg("-i").arg(source);
        let volume = if clip["muted"].as_bool().unwrap_or(false) { 0.0 } else {
            clip["volume"].as_f64().unwrap_or(100.0).clamp(0.0, 100.0) / 100.0
        };
        let fade_in = clip["fadeIn"].as_f64().unwrap_or(0.0).clamp(0.0, clip_duration);
        let fade_out = clip["fadeOut"].as_f64().unwrap_or(0.0).clamp(0.0, clip_duration);
        audio_indices.push((input_index, clip["start"].as_f64().unwrap_or(0.0), clip_duration, volume, fade_in, fade_out));
    }

    let temp_root = std::env::temp_dir().join(format!("dango-export-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()));
    fs::create_dir_all(&temp_root).map_err(|error| format!("Could not prepare export text: {error}"))?;
    let _temp_files = ExportTempFiles(temp_root.clone());
    let font_path = if titles.is_empty() {
        String::new()
    } else {
        ffmpeg_filter_path(&bundled_font(&app).ok_or("The Chunkypuffly font is missing from this Dango build")?)
    };
    let mut graph = format!("color=c=black:s={width}x{height}:r=30:d={duration:.4}[base0]");
    let mut last = "base0".to_string();
    for (index, clip) in visual.iter().enumerate() {
        let start = clip["start"].as_f64().unwrap_or(0.0).max(0.0);
        let clip_duration = clip["duration"].as_f64().unwrap_or(1.0).max(0.05);
        let end = start + clip_duration;
        let controls = &clip["controls"];
        let scale = (controls["size"].as_f64().unwrap_or(100.0) / 100.0).clamp(0.1, 4.0);
        let opacity = (controls["opacity"].as_f64().unwrap_or(100.0) / 100.0).clamp(0.0, 1.0);
        let x = controls["positionX"].as_f64().unwrap_or(0.0).clamp(-100.0, 100.0);
        let y = controls["positionY"].as_f64().unwrap_or(0.0).clamp(-100.0, 100.0);
        let rotation = controls["rotation"].as_f64().unwrap_or(0.0);
        let mut filters = vec![format!("scale={width}:{height}:force_original_aspect_ratio=decrease"), format!("scale=iw*{scale:.4}:ih*{scale:.4}")];
        if rotation.abs() > 0.01 {
            filters.push(format!("rotate={:.6}*PI/180:ow=rotw(iw):oh=roth(ih):c=none", rotation));
        }
        filters.extend(media_effect_filters(clips, clip));
        filters.push("format=rgba".into());
        filters.push(format!("colorchannelmixer=aa={opacity:.4}"));
        filters.push("setsar=1".into());
        filters.push("setpts=PTS-STARTPTS".into());
        let next = format!("base{}", index + 1);
        graph.push_str(&format!(";[{index}:v]{},split=2[ov{index}][src{index}];[ov{index}]setpts=PTS+{start:.4}/TB[v{index}];[src{index}]pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black,format=yuv420p[tsrc{index}];[{last}][v{index}]overlay=x='(W-w)/2+{:.4}*W/100':y='(H-h)/2+{:.4}*H/100':eof_action=pass:enable='between(t,{start:.4},{end:.4})'[{next}]", filters.join(","), x, y));
        last = next;
    }

    let mut transition_sources = vec![false; visual.len()];
    for (effect_index, effect) in clips.iter().filter(|clip| clip["kind"] == "effect" && clip["effectName"] == "Cross dissolve").enumerate() {
        let effect_duration = effect["duration"].as_f64().unwrap_or(1.0).clamp(0.1, 10.0);
        let cut = effect["transitionCut"].as_f64().unwrap_or(effect["start"].as_f64().unwrap_or(0.0) + effect_duration / 2.0);
        let mut pair = None;
        for (out_index, outgoing) in visual.iter().enumerate() {
            for (in_index, incoming) in visual.iter().enumerate() {
                if out_index == in_index { continue; }
                let out_end = outgoing["start"].as_f64().unwrap_or(0.0) + outgoing["duration"].as_f64().unwrap_or(0.0);
                let in_start = incoming["start"].as_f64().unwrap_or(0.0);
                if (out_end - cut).abs() <= 1.0 / 30.0 + 0.002 && (in_start - cut).abs() <= 1.0 / 30.0 + 0.002 {
                    pair = Some((out_index, in_index, out_end));
                }
            }
        }
        if let Some((out_index, in_index, cut_time)) = pair {
            let half = (effect_duration / 2.0)
                .min(visual[out_index]["duration"].as_f64().unwrap_or(0.0))
                .min(visual[in_index]["duration"].as_f64().unwrap_or(0.0));
            if half < 0.03 { continue; }
            transition_sources[out_index] = true;
            transition_sources[in_index] = true;
            let total = half * 2.0;
            let transition_start = cut_time - half;
            let transition_end = cut_time + half;
            let outgoing_offset = (visual[out_index]["duration"].as_f64().unwrap_or(0.0) - half).max(0.0);
            let out_label = format!("xout{effect_index}");
            let in_label = format!("xin{effect_index}");
            let mix_label = format!("xdissolve{effect_index}");
            let next = format!("base_xdissolve{effect_index}");
            graph.push_str(&format!(";[tsrc{out_index}]trim=start={outgoing_offset:.4}:duration={half:.4},setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration={half:.4},trim=duration={total:.4}[{out_label}];[tsrc{in_index}]trim=start=0:duration={half:.4},setpts=PTS-STARTPTS,tpad=start_mode=clone:start_duration={half:.4},trim=duration={total:.4}[{in_label}];[{out_label}][{in_label}]blend=all_expr='A*(1-T/{total:.4})+B*(T/{total:.4})':shortest=1,format=yuv420p,setpts=PTS-STARTPTS+{transition_start:.4}/TB[{mix_label}];[{last}][{mix_label}]overlay=eof_action=pass:enable='between(t,{transition_start:.4},{transition_end:.4})'[{next}]"));
            last = next;
        }
    }
    for (index, used) in transition_sources.iter().enumerate() {
        if !used { graph.push_str(&format!(";[tsrc{index}]nullsink")); }
    }

    for (index, title) in titles.iter().enumerate() {
        let text = title["titleText"].as_str().unwrap_or("");
        let text_path = temp_root.join(format!("title-{index}.txt"));
        fs::write(&text_path, text).map_err(|error| format!("Could not prepare title text: {error}"))?;
        let escaped_text_path = ffmpeg_filter_path(&text_path);
        let start = title["start"].as_f64().unwrap_or(0.0).max(0.0);
        let end = start + title["duration"].as_f64().unwrap_or(3.0).max(0.05);
        let controls = &title["controls"];
        let size = (title["titleSize"].as_f64().unwrap_or(42.0) * (height as f64 / 450.0)
            * (controls["size"].as_f64().unwrap_or(100.0) / 100.0).clamp(0.1, 4.0)).clamp(8.0, 600.0);
        let opacity = (controls["opacity"].as_f64().unwrap_or(100.0) / 100.0).clamp(0.0, 1.0);
        let x = title["titleBox"]["x"].as_f64().unwrap_or(0.0);
        let y = title["titleBox"]["y"].as_f64().unwrap_or(0.0);
        let box_width = title["titleBox"]["width"].as_f64().unwrap_or(100.0);
        let box_height = title["titleBox"]["height"].as_f64().unwrap_or(100.0);
        let x_center = (x + box_width / 2.0 + controls["positionX"].as_f64().unwrap_or(0.0)).clamp(0.0, 100.0) / 100.0;
        let y_center = (y + box_height / 2.0 + controls["positionY"].as_f64().unwrap_or(0.0)).clamp(0.0, 100.0) / 100.0;
        let next = format!("title{index}");
        graph.push_str(&format!(";[{last}]drawtext=fontfile='{font_path}':textfile='{escaped_text_path}':expansion=none:fontsize={size:.2}:fontcolor=white@{opacity:.3}:borderw=2:bordercolor=black@0.55:x='({width}*{x_center:.5})-text_w/2':y='({height}*{y_center:.5})-text_h/2':enable='between(t,{start:.4},{end:.4})'[{next}]"));
        last = next;
    }

    let mut audio_labels = Vec::new();
    for (index, start, clip_duration, volume, fade_in, fade_out) in audio_indices {
        let label = format!("a{index}");
        graph.push_str(&format!(";[{index}:a]atrim=duration={clip_duration:.4},asetpts=PTS-STARTPTS"));
        if volume != 1.0 { graph.push_str(&format!(",volume={volume:.4}")); }
        if fade_in > 0.0 { graph.push_str(&format!(",afade=t=in:st=0:d={fade_in:.4}")); }
        if fade_out > 0.0 { graph.push_str(&format!(",afade=t=out:st={:.4}:d={fade_out:.4}", (clip_duration - fade_out).max(0.0))); }
        graph.push_str(&format!(",adelay={}:all=1[{label}]", (start * 1000.0) as u64));
        audio_labels.push(format!("[{label}]"));
    }
    graph.push_str(&format!(";[{last}]format=yuv420p[vout]"));
    if !audio_labels.is_empty() {
        graph.push_str(&format!(";{}amix=inputs={}:duration=longest:normalize=0,alimiter=limit=0.95[aout]", audio_labels.concat(), audio_labels.len()));
    }
    command.arg("-filter_complex").arg(&graph).args(["-map", "[vout]"]);
    if !audio_labels.is_empty() { command.args(["-map", "[aout]"]); } else { command.arg("-an"); }
    command.args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-r", "30"]);
    if !audio_labels.is_empty() { command.args(["-c:a", "aac", "-b:a", "192k"]); }
    let result = command.arg("-t").arg(format!("{duration:.4}")).arg(&output).output()
        .map_err(|error| format!("FFmpeg could not start: {error}. Put ffmpeg.exe beside Dango or add FFmpeg to PATH."))?;
    if !result.status.success() {
        let _ = fs::remove_file(&output);
        return Err(String::from_utf8_lossy(&result.stderr).lines().rev().take(10).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"));
    }
    Ok(output.to_string_lossy().into_owned())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            import_media,
            generate_thumbnail,
            save_project,
            open_project,
            export_video,
            window_action
        ])
        .run(tauri::generate_context!())
        .expect("Dango failed to start");
}
