use serde_json::Value;

fn filter_enable(start: f64, end: f64) -> String {
    format!("between(t,{start:.4},{end:.4})")
}

fn effect_value_at(effect: &Value, name: &str, time: f64) -> f64 {
    let default = match name {
        "offset" => 8.0,
        "intensity" if effect["effectName"] == "Fade" || effect["effectName"] == "Cross dissolve" => 100.0,
        _ => 50.0,
    };
    let fallback = effect[name].as_f64().unwrap_or(default);
    let Some(keys) = effect["keys"].as_array() else {
        return fallback;
    };
    let mut keyed: Vec<(f64, f64)> = keys
        .iter()
        .filter_map(|key| {
            Some((
                key["time"].as_f64()?.max(0.0),
                key["controls"][name].as_f64()?,
            ))
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut previous = (0.0, fallback);
    for current in keyed {
        if time < current.0 {
            let span = (current.0 - previous.0).max(1.0 / 30.0);
            let amount = ((time - previous.0) / span).clamp(0.0, 1.0);
            return previous.1 + (current.1 - previous.1) * amount;
        }
        previous = current;
    }
    previous.1
}

fn hex_rgb(value: &str, fallback: [f64; 3]) -> [f64; 3] {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) { return fallback; }
    let mut color = [0.0; 3];
    for (index, component) in color.iter_mut().enumerate() {
        let Ok(value) = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16) else { return fallback; };
        *component = value as f64 / 255.0;
    }
    color
}

fn effect_filters(
    effect: &Value,
    name: &str,
    intensity: f64,
    enable: &str,
    local_start: f64,
    local_end: f64,
    fade_start: f64,
    fade_end: f64,
    offset: f64,
) -> Vec<String> {
    let amount = intensity.clamp(0.0, 100.0) / 100.0;
    match name {
        "Fade" => {
            let direction = effect["direction"].as_str().unwrap_or("in");
            let fade_duration = (fade_end - fade_start).max(0.001);
            let progress = format!("min(1\\,max(0\\,(T-{fade_start:.4})/{fade_duration:.4}))");
            let factor = if direction == "out" {
                format!("(1-{amount:.4}*{progress})")
            } else {
                format!("(1-{amount:.4}*(1-{progress}))")
            };
            vec![
                "format=rgba".into(),
                format!("geq=r='r(X,Y)*{factor}':g='g(X,Y)*{factor}':b='b(X,Y)*{factor}':a='alpha(X,Y)':enable='{enable}'"),
            ]
        }
        "Blur" => vec![format!("gblur=sigma={:.3}:enable='{enable}'", 1.0 + amount * 9.0)],
        "Vignette" => vec![format!("vignette=angle={:.4}:enable='{enable}'", 0.18 + amount * 0.72)],
        "Warm film" => vec![format!(
            "eq=saturation={:.3}:gamma_r=1.05:gamma_b=0.92:enable='{enable}'",
            1.0 + amount * 0.35
        )],
        "Sharpen" => vec![format!("unsharp=5:5:{:.3}:5:5:0:enable='{enable}'", amount * 1.5)],
        "Soft glow" => vec![
            format!("gblur=sigma={:.3}:enable='{enable}'", amount * 2.2),
            format!("eq=brightness={:.3}:saturation={:.3}:enable='{enable}'", amount * 0.08, 1.0 + amount * 0.2),
        ],
        "Swirl" => {
            let radius = "sqrt((X-W/2)*(X-W/2)+(Y-H/2)*(Y-H/2))";
            let angle = format!("(atan2(Y-H/2,X-W/2)+{:.4}*exp(-({radius})/(min(W,H)*0.45)))", amount * 2.4);
            let sample_x = format!("(W/2+({radius})*cos({angle}))");
            let sample_y = format!("(H/2+({radius})*sin({angle}))");
            vec!["format=rgba".into(), format!("geq=r='r({sample_x}\\,{sample_y})':g='g({sample_x}\\,{sample_y})':b='b({sample_x}\\,{sample_y})':a='alpha(X,Y)':enable='{enable}'")]
        }
        "Wave" => {
            let wavelength = effect["wavelength"].as_f64().unwrap_or(80.0).clamp(8.0, 240.0);
            let speed = effect["waveSpeed"].as_f64().unwrap_or(4.0).clamp(0.0, 10.0);
            let displacement = format!("({:.3}*sin((Y/{wavelength:.3})*2*PI+T*{speed:.3}))", amount * 18.0);
            vec!["format=rgba".into(), format!("geq=r='r(X+{displacement}\\,Y)':g='g(X+{displacement}\\,Y)':b='b(X+{displacement}\\,Y)':a='alpha(X,Y)':enable='{enable}'")]
        }
        "Grain" => vec![format!("noise=alls={:.2}:allf=t+u:enable='{enable}'", amount * 18.0)],
        "VHS" => vec![
            format!("chromashift=cbh={}:crh={}:enable='{enable}'", (amount * 4.0).round() as i32, (-amount * 4.0).round() as i32),
            format!("eq=contrast={:.3}:saturation={:.3}:gamma_r=1.04:gamma_b=0.96:enable='{enable}'", 1.0 + amount * 0.2, 1.0 - amount * 0.22),
            format!("noise=alls={:.2}:allf=t+u:enable='{enable}'", amount * 12.0),
        ],
        "Chromatic Aberration" => {
            let colors = match effect["palette"].as_str().unwrap_or("RGB") {
                "GBY" => [[0.0, 1.0, 0.4], [0.09, 0.55, 1.0], [1.0, 0.9, 0.0]],
                "Pink / Blue / Purple" => [[1.0, 0.31, 0.60], [0.22, 0.84, 1.0], [0.57, 0.30, 1.0]],
                "Custom" => [
                    hex_rgb(effect["chromaticColors"]["red"].as_str().unwrap_or("#ff0000"), [1.0, 0.0, 0.0]),
                    hex_rgb(effect["chromaticColors"]["green"].as_str().unwrap_or("#00ff00"), [0.0, 1.0, 0.0]),
                    hex_rgb(effect["chromaticColors"]["blue"].as_str().unwrap_or("#0000ff"), [0.0, 0.0, 1.0]),
                ],
                _ => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            };
            let separation = offset.clamp(0.0, 40.0) * amount;
            let samples = [format!("(X-{separation:.3})"), "X".into(), format!("(X+{separation:.3})")];
            let channel_expression = |channel: usize| {
                format!(
                    "({:.4}*r({}\\,Y)+{:.4}*g(X\\,Y)+{:.4}*b({}\\,Y))",
                    colors[0][channel], samples[0], colors[1][channel], colors[2][channel], samples[2]
                )
            };
            vec![
                "format=rgba".into(),
                format!("geq=r='{}':g='{}':b='{}':a='alpha(X,Y)':enable='{enable}'", channel_expression(0), channel_expression(1), channel_expression(2)),
            ]
        }
        _ => Vec::new(),
    }
}

pub fn media_effect_filters(clips: &[Value], clip: &Value) -> Vec<String> {
    let clip_start = clip["start"].as_f64().unwrap_or(0.0);
    let clip_end = clip_start + clip["duration"].as_f64().unwrap_or(0.0);
    let clip_track = clip["track"].as_str().and_then(|track| track.strip_prefix('V'))
        .and_then(|rank| rank.parse::<u32>().ok()).unwrap_or(1);
    let mut filters = Vec::new();

    let color = &clip["color"];
    if !color.is_null() {
        let hue = color["hue"].as_f64().unwrap_or(0.0).clamp(-180.0, 180.0);
        let saturation = (color["saturation"].as_f64().unwrap_or(100.0) / 100.0).clamp(0.0, 2.0);
        let brightness = (color["brightness"].as_f64().unwrap_or(0.0) / 100.0).clamp(-1.0, 1.0);
        let contrast = (color["contrast"].as_f64().unwrap_or(100.0) / 100.0).clamp(0.0, 2.0);
        let sharpness = (color["sharpness"].as_f64().unwrap_or(0.0) / 100.0).clamp(0.0, 1.0) * 1.5;
        if hue.abs() > 0.001 || (saturation - 1.0).abs() > 0.001 {
            filters.push(format!("hue=h={hue:.3}:s={saturation:.3}"));
        }
        if brightness.abs() > 0.001 || (contrast - 1.0).abs() > 0.001 {
            filters.push(format!("eq=brightness={brightness:.3}:contrast={contrast:.3}"));
        }
        if sharpness > 0.001 {
            filters.push(format!("unsharp=5:5:{sharpness:.3}:5:5:0"));
        }
    }

    for effect in clips.iter().filter(|item| item["kind"] == "effect") {
        let effect_track = effect["track"].as_str().and_then(|track| track.strip_prefix('V'))
            .and_then(|rank| rank.parse::<u32>().ok()).unwrap_or(2);
        if effect_track <= clip_track || effect["effectName"] == "Cross dissolve" {
            continue;
        }
        let effect_start = effect["start"].as_f64().unwrap_or(0.0);
        let effect_end = effect_start + effect["duration"].as_f64().unwrap_or(0.0);
        let active_start = effect_start.max(clip_start);
        let active_end = effect_end.min(clip_end);
        if active_end <= active_start { continue; }
        let name = effect["effectName"].as_str().unwrap_or("");
        let local_start = active_start - clip_start;
        let local_end = active_end - clip_start;
        let fade_start = (effect_start - clip_start).max(0.0);
        let fade_end = (effect_end - clip_start).min(clip_end - clip_start);
        let has_animated_keys = effect["keys"].as_array().is_some_and(|keys| {
            keys.iter().any(|key| key["controls"]["intensity"].as_f64().is_some() || key["controls"]["offset"].as_f64().is_some())
        });
        if has_animated_keys {
            let first_frame = (local_start * 30.0).floor() as i64;
            let last_frame = (local_end * 30.0).ceil() as i64;
            for frame in first_frame..last_frame {
                let segment_start = (frame as f64 / 30.0).max(local_start);
                let segment_end = ((frame + 1) as f64 / 30.0).min(local_end);
                if segment_end <= segment_start { continue; }
                let effect_time = (segment_start + segment_end) * 0.5 + clip_start - effect_start;
                let intensity = effect_value_at(effect, "intensity", effect_time);
                let offset = effect_value_at(effect, "offset", effect_time);
                let enable = filter_enable(segment_start, (segment_end - 0.0005).max(segment_start));
                filters.extend(effect_filters(effect, name, intensity, &enable, segment_start, segment_end, fade_start, fade_end, offset));
            }
        } else {
            let enable = filter_enable(local_start, local_end);
            let intensity = effect["intensity"].as_f64().unwrap_or(if name == "Fade" { 100.0 } else { 50.0 });
            let offset = effect["offset"].as_f64().unwrap_or(8.0);
            filters.extend(effect_filters(effect, name, intensity, &enable, local_start, local_end, fade_start, fade_end, offset));
        }
    }
    filters
}
