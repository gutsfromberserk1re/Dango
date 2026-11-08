use serde_json::Value;
use std::f64::consts::FRAC_PI_2;

const MAX_SEGMENTS: usize = 48;

#[derive(Clone, Copy)]
pub struct RenderCfg {
    pub fps: f64,
    /// Output height in px; used to scale blur radii.
    pub height: f64,
}

impl Default for RenderCfg {
    fn default() -> Self {
        Self { fps: 30.0, height: 1080.0 }
    }
}

/// One stage of a clip's filter graph. Most effects are plain chain filters;
/// glow-style effects need a second branch that is blended back in.
pub enum Step {
    Chain(String),
    /// `prep` runs, the stream is split, `branch` runs on one copy, and `merge`
    /// (a two-input filter such as `blend`) combines [original][branch].
    Branch { prep: Vec<String>, branch: Vec<String>, merge: String },
}

/// Turns steps into a filtergraph fragment `[input] ... [output]`.
/// `uid` must be unique per clip so internal pad labels never collide.
pub fn render_filtergraph(steps: &[Step], input: &str, output: &str, uid: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut current = input.to_string();
    let mut n = 0usize;
    for step in steps {
        match step {
            Step::Chain(filter) => pending.push(filter.clone()),
            Step::Branch { prep, branch, merge } => {
                n += 1;
                let (a, b, c, out) = (
                    format!("{uid}_a{n}"),
                    format!("{uid}_b{n}"),
                    format!("{uid}_c{n}"),
                    format!("{uid}_o{n}"),
                );
                pending.extend(prep.iter().cloned());
                pending.push("split".into());
                parts.push(format!("[{current}]{}[{a}][{b}]", pending.join(",")));
                pending.clear();
                parts.push(format!("[{b}]{}[{c}]", branch.join(",")));
                parts.push(format!("[{a}][{c}]{merge}[{out}]"));
                current = out;
            }
        }
    }
    if pending.is_empty() {
        parts.push(format!("[{current}]null[{output}]"));
    } else {
        parts.push(format!("[{current}]{}[{output}]", pending.join(",")));
    }
    parts.join(";")
}

fn filter_enable(start: f64, end: f64) -> String {
    format!("between(t,{start:.4},{end:.4})")
}

fn default_value(effect: &Value, name: &str) -> f64 {
    match name {
        "offset" => 8.0,
        "intensity" if effect["effectName"] == "Fade" || effect["effectName"] == "Cross dissolve" => 100.0,
        _ => 50.0,
    }
}

fn keyframes(effect: &Value, name: &str) -> Vec<(f64, f64)> {
    let Some(keys) = effect["keys"].as_array() else {
        return Vec::new();
    };
    let mut keyed: Vec<(f64, f64)> = keys
        .iter()
        .filter_map(|key| Some((key["time"].as_f64()?.max(0.0), key["controls"][name].as_f64()?)))
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    keyed
}

fn effect_value_at(effect: &Value, name: &str, time: f64, fps: f64) -> f64 {
    let fallback = effect[name].as_f64().unwrap_or_else(|| default_value(effect, name));
    let mut previous = (0.0, fallback);
    for current in keyframes(effect, name) {
        if time < current.0 {
            let span = (current.0 - previous.0).max(1.0 / fps);
            let amount = ((time - previous.0) / span).clamp(0.0, 1.0);
            return previous.1 + (current.1 - previous.1) * amount;
        }
        previous = current;
    }
    previous.1
}

/// Piecewise-linear FFmpeg expression for a control. `tvar` is `T` (geq, blend)
/// or `t` (eq). Effect-local time = tvar + shift.
fn value_expr(effect: &Value, name: &str, tvar: &str, shift: f64, fps: f64) -> String {
    let fallback = effect[name].as_f64().unwrap_or_else(|| default_value(effect, name));
    let keys = keyframes(effect, name);
    if keys.is_empty() {
        return format!("({fallback:.4})");
    }
    let mut points = Vec::with_capacity(keys.len() + 1);
    if keys[0].0 > 0.0 {
        points.push((0.0, fallback));
    }
    points.extend(keys);
    let time = format!("({tvar}+({shift:.4}))");
    let mut expr = format!("({:.4})", points[points.len() - 1].1);
    for pair in points.windows(2).rev() {
        let ((t0, v0), (t1, v1)) = (pair[0], pair[1]);
        let span = (t1 - t0).max(1.0 / fps);
        expr = format!(
            "if(lt({time},{t1:.4}),({v0:.4})+(({v1:.4})-({v0:.4}))*clip(({time}-{t0:.4})/{span:.4},0,1),{expr})"
        );
    }
    expr
}

fn hex_rgb(value: &str, fallback: [f64; 3]) -> [f64; 3] {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return fallback;
    }
    let mut color = [0.0; 3];
    for (index, component) in color.iter_mut().enumerate() {
        let Ok(value) = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16) else {
            return fallback;
        };
        *component = value as f64 / 255.0;
    }
    color
}

struct EffectCtx<'a> {
    effect: &'a Value,
    shift: f64,
    cfg: RenderCfg,
    enable: String,
    local_start: f64,
    local_end: f64,
}

impl EffectCtx<'_> {
    fn param(&self, name: &str, tvar: &str) -> String {
        value_expr(self.effect, name, tvar, self.shift, self.cfg.fps)
    }
    fn amount(&self, tvar: &str) -> String {
        format!("(clip({},0,100)/100)", self.param("intensity", tvar))
    }
    fn num(&self, key: &str, default: f64, lo: f64, hi: f64) -> f64 {
        self.effect[key].as_f64().unwrap_or(default).clamp(lo, hi)
    }
}

fn geq_filter(exprs: &[String; 4], interpolation: &str, enable: &str) -> String {
    format!(
        "geq=r='{}':g='{}':b='{}':a='{}':interpolation={interpolation}:enable='{enable}'",
        exprs[0], exprs[1], exprs[2], exprs[3]
    )
}

fn remap_exprs(prelude: &str, x: &str, y: &str) -> [String; 4] {
    ["r", "g", "b", "alpha"].map(|plane| format!("{prelude}{plane}({x},{y})"))
}

// ---------------------------------------------------------------- expression effects

fn fade_filter(ctx: &EffectCtx, fade_start: f64, fade_end: f64) -> String {
    let amount = ctx.amount("t");
    let duration = (fade_end - fade_start).max(0.001);
    let progress = format!("clip((t-{fade_start:.4})/{duration:.4},0,1)");
    let factor = if ctx.effect["direction"].as_str().unwrap_or("in") == "out" {
        format!("(1-{amount}*{progress})")
    } else {
        format!("(1-{amount}*(1-{progress}))")
    };
    format!(
        "eq=contrast='{factor}':brightness='-(1-{factor})/2':saturation='{factor}':eval=frame:enable='{}'",
        ctx.enable
    )
}

fn swirl_filter(ctx: &EffectCtx) -> String {
    let twist = format!("({}*4.0)", ctx.amount("T"));
    let radius = ctx.num("swirlRadius", 0.6, 0.1, 1.5);
    let prelude = format!(
        "st(0,X-W/2);st(1,Y-H/2);st(2,hypot(ld(0),ld(1)));\
         st(3,clip(1-ld(2)/(min(W,H)*{radius:.4}),0,1));\
         st(4,atan2(ld(1),ld(0))-{twist}*ld(3)*ld(3));\
         st(5,W/2+ld(2)*cos(ld(4)));st(6,H/2+ld(2)*sin(ld(4)));"
    );
    geq_filter(&remap_exprs(&prelude, "ld(5)", "ld(6)"), "bilinear", &ctx.enable)
}

fn wave_filter(ctx: &EffectCtx) -> String {
    let amount = ctx.amount("T");
    let wavelength = ctx.num("wavelength", 80.0, 8.0, 240.0);
    let speed = ctx.num("waveSpeed", 4.0, 0.0, 10.0);
    let vertical = ctx.effect["waveDirection"].as_str() == Some("vertical");
    let shift = ctx.shift;
    let (major, x, y) = if vertical { ("X", "X", "(Y+ld(1))") } else { ("Y", "(X+ld(1))", "Y") };
    let prelude = format!(
        "st(0,H/1080);\
         st(1,{amount}*18*ld(0)*sin(({major}/({wavelength:.3}*ld(0)))*2*PI+(T+({shift:.4}))*{speed:.3}));"
    );
    geq_filter(&remap_exprs(&prelude, x, y), "bilinear", &ctx.enable)
}

fn chromatic_filter(ctx: &EffectCtx) -> String {
    let effect = ctx.effect;
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
    let prelude = format!("st(0,clip({},0,40)*{});", ctx.param("offset", "T"), ctx.amount("T"));
    let samples = ["r(X-ld(0),Y)", "g(X,Y)", "b(X+ld(0),Y)"];
    let mix = |channel: usize| {
        let terms: Vec<String> = (0..3)
            .filter(|&k| colors[k][channel] > 0.0005)
            .map(|k| format!("{:.4}*{}", colors[k][channel], samples[k]))
            .collect();
        let body = if terms.is_empty() { "0".to_string() } else { terms.join("+") };
        format!("{prelude}{body}")
    };
    geq_filter(&[mix(0), mix(1), mix(2), "alpha(X,Y)".to_string()], "bilinear", &ctx.enable)
}

/// Random horizontal row-block displacement with RGB split, re-rolled 12x/sec.
fn glitch_filter(ctx: &EffectCtx) -> String {
    let amount = ctx.amount("T");
    let shift = ctx.shift;
    let prelude = format!(
        "st(5,{amount});\
         st(0,floor(Y/(16*H/1080)));\
         st(1,floor((T+({shift:.4}))*12));\
         st(2,sin(ld(0)*12.9898+ld(1)*78.233)*43758.5453);st(2,ld(2)-floor(ld(2)));\
         st(3,sin(ld(0)*39.346+ld(1)*11.135)*24634.6345);st(3,ld(3)-floor(ld(3)));\
         st(4,if(gt(ld(2),1-ld(5)*0.3),(ld(3)-0.5)*ld(5)*160*H/1080,0));"
    );
    let exprs = [
        format!("{prelude}r(X+ld(4)*1.3,Y)"),
        format!("{prelude}g(X+ld(4),Y)"),
        format!("{prelude}b(X+ld(4)*0.7,Y)"),
        format!("{prelude}alpha(X+ld(4),Y)"),
    ];
    geq_filter(&exprs, "bilinear", &ctx.enable)
}

fn pixelate_filter(ctx: &EffectCtx) -> String {
    let prelude = format!("st(0,max(1,round((1+{}*40)*H/1080)));", ctx.amount("T"));
    let x = "(floor(X/ld(0))*ld(0)+floor(ld(0)/2))";
    let y = "(floor(Y/ld(0))*ld(0)+floor(ld(0)/2))";
    geq_filter(&remap_exprs(&prelude, x, y), "nearest", &ctx.enable)
}

/// Hard mirror: left half reflected onto the right (or top onto bottom). Ignores intensity.
fn mirror_filter(ctx: &EffectCtx) -> String {
    let (x, y) = if ctx.effect["mirrorAxis"].as_str() == Some("vertical") {
        ("X", "if(gt(Y,H/2),H-1-Y,Y)")
    } else {
        ("if(gt(X,W/2),W-1-X,X)", "Y")
    };
    geq_filter(&remap_exprs("", x, y), "bilinear", &ctx.enable)
}

fn shake_filter(ctx: &EffectCtx) -> String {
    let speed = ctx.num("shakeSpeed", 1.0, 0.1, 5.0);
    let shift = ctx.shift;
    let prelude = format!(
        "st(5,{amount});\
         st(0,ld(5)*14*H/1080);\
         st(1,(T+({shift:.4}))*{speed:.3});\
         st(2,1+ld(5)*0.04);\
         st(3,ld(0)*(sin(ld(1)*37)+0.5*sin(ld(1)*91+1.3)));\
         st(4,ld(0)*(sin(ld(1)*43+0.7)+0.5*sin(ld(1)*79+2.1)));",
        amount = ctx.amount("T")
    );
    let x = "(W/2+(X-W/2)/ld(2)+ld(3))";
    let y = "(H/2+(Y-H/2)/ld(2)+ld(4))";
    geq_filter(&remap_exprs(&prelude, x, y), "bilinear", &ctx.enable)
}

fn scanlines_filter(ctx: &EffectCtx) -> String {
    let prelude = format!(
        "st(0,max(2,2*H/1080));st(1,1-{}*0.5*(0.5+0.5*sin(PI*Y/ld(0))));",
        ctx.amount("T")
    );
    let exprs = [
        format!("{prelude}r(X,Y)*ld(1)"),
        format!("{prelude}g(X,Y)*ld(1)"),
        format!("{prelude}b(X,Y)*ld(1)"),
        "alpha(X,Y)".to_string(),
    ];
    geq_filter(&exprs, "bilinear", &ctx.enable)
}

fn strobe_filter(ctx: &EffectCtx) -> String {
    let rate = ctx.num("strobeRate", 4.0, 0.5, 15.0);
    let shift = ctx.shift;
    format!(
        "eq=brightness='clip({amount}*0.7*pow(max(0,sin(2*PI*{rate:.3}*(t+({shift:.4})))),6),-1,1)':eval=frame:enable='{enable}'",
        amount = ctx.amount("t"),
        enable = ctx.enable
    )
}

// ---------------------------------------------------------------- glow / bloom

/// Real glow: bright-pass -> gaussian blur -> screen-blended over the original.
/// Done in 8-bit planar RGB so screen is applied to colour, not to YUV chroma.
fn glow_step(ctx: &EffectCtx, name: &str) -> Step {
    let (default_threshold, default_radius) = if name == "Bloom" { (0.6, 3.0) } else { (0.0, 1.5) };
    let threshold = ctx.num("glowThreshold", default_threshold, 0.0, 0.95);
    let radius_pct = ctx.num("glowRadius", default_radius, 0.2, 10.0);
    let sigma = (radius_pct / 100.0 * ctx.cfg.height).clamp(1.0, 300.0);

    let mut branch = Vec::new();
    if threshold > 0.001 {
        let low = threshold * 255.0;
        let gain = 255.0 / (255.0 - low).max(1.0);
        let curve = format!("clip((val-{low:.2})*{gain:.4},0,maxval)");
        branch.push(format!("lutrgb=r='{curve}':g='{curve}':b='{curve}'"));
    }
    branch.push(format!("gblur=sigma={sigma:.2}:steps=2"));

    // A = original (first input), B = blurred branch. screen mixed by k:
    // A + (screen(A,B) - A) * k  ==  A + (255-A)*B/255 * k
    let k = format!("{}*between(T,{:.4},{:.4})", ctx.amount("T"), ctx.local_start, ctx.local_end);
    let colour = format!("st(0,{k});A+(255-A)*B/255*ld(0)");
    Step::Branch {
        prep: vec!["format=gbrap".into()],
        branch,
        merge: format!("blend=c0_expr='{colour}':c1_expr='{colour}':c2_expr='{colour}':c3_expr='A'"),
    }
}

// ---------------------------------------------------------------- static-parameter effects

fn sample_segments(effect: &Value, local_start: f64, local_end: f64, shift: f64, fps: f64) -> Vec<(f64, f64, f64)> {
    if keyframes(effect, "intensity").is_empty() {
        return vec![(local_start, local_end, effect_value_at(effect, "intensity", 0.0, fps))];
    }
    let step = (1.0 / fps).max((local_end - local_start) / MAX_SEGMENTS as f64);
    let mut out: Vec<(f64, f64, f64)> = Vec::new();
    let mut start = local_start;
    while start < local_end - 1e-6 {
        let end = (start + step).min(local_end);
        let value = effect_value_at(effect, "intensity", (start + end) * 0.5 + shift, fps);
        match out.last_mut() {
            Some(last) if (last.2 - value).abs() < 0.25 => last.1 = end,
            _ => out.push((start, end, value)),
        }
        start = end;
    }
    out
}

fn static_effect_filters(name: &str, intensity: f64, enable: &str) -> Vec<String> {
    let amount = intensity.clamp(0.0, 100.0) / 100.0;
    match name {
        "Blur" => vec![format!("gblur=sigma={:.3}:enable='{enable}'", 1.0 + amount * 9.0)],
        "Vignette" => vec![format!(
            "vignette=angle={:.4}:enable='{enable}'",
            FRAC_PI_2 - amount * (FRAC_PI_2 - 0.18)
        )],
        "Warm film" => vec![format!(
            "eq=saturation={:.3}:gamma_r=1.05:gamma_b=0.92:enable='{enable}'",
            1.0 + amount * 0.35
        )],
        "Sharpen" => vec![format!("unsharp=5:5:{:.3}:5:5:0:enable='{enable}'", amount * 1.5)],
        "Grain" => vec![format!("noise=alls={:.2}:allf=t+u:enable='{enable}'", amount * 18.0)],
        "VHS" => vec![
            format!("chromashift=cbh={}:crh={}:enable='{enable}'", (amount * 4.0).round() as i32, (-amount * 4.0).round() as i32),
            format!("eq=contrast={:.3}:saturation={:.3}:gamma_r=1.04:gamma_b=0.96:enable='{enable}'", 1.0 + amount * 0.2, 1.0 - amount * 0.22),
            format!("noise=alls={:.2}:allf=t+u:enable='{enable}'", amount * 12.0),
        ],
        "Black & white" => vec![format!("eq=saturation={:.3}:enable='{enable}'", 1.0 - amount)],
        "Sepia" => {
            let keep = 1.0 - amount;
            vec![format!(
                "colorchannelmixer=rr={:.4}:rg={:.4}:rb={:.4}:gr={:.4}:gg={:.4}:gb={:.4}:br={:.4}:bg={:.4}:bb={:.4}:enable='{enable}'",
                keep + amount * 0.393, amount * 0.769, amount * 0.189,
                amount * 0.349, keep + amount * 0.686, amount * 0.168,
                amount * 0.272, amount * 0.534, keep + amount * 0.131,
            )]
        }
        "Cinematic bars" => {
            let bar = amount * 0.14;
            vec![
                format!("drawbox=x=0:y=0:w=iw:h=ih*{bar:.4}:color=black:t=fill:enable='{enable}'"),
                format!("drawbox=x=0:y=ih-ih*{bar:.4}:w=iw:h=ih*{bar:.4}:color=black:t=fill:enable='{enable}'"),
            ]
        }
        "Posterize" => {
            let step = 1.0 + amount * 63.0;
            let curve = format!("trunc(val/{step:.3})*{step:.3}");
            vec![format!("lutrgb=r='{curve}':g='{curve}':b='{curve}':enable='{enable}'")]
        }
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------- main entry

fn track_rank(item: &Value, default: u32) -> u32 {
    item["track"].as_str().and_then(|track| track.strip_prefix('V'))
        .and_then(|rank| rank.parse::<u32>().ok()).unwrap_or(default)
}

pub fn media_effect_steps(clips: &[Value], clip: &Value, cfg: RenderCfg) -> Vec<Step> {
    let cfg = RenderCfg {
        fps: if cfg.fps > 0.0 { cfg.fps } else { 30.0 },
        height: if cfg.height > 0.0 { cfg.height } else { 1080.0 },
    };
    let clip_start = clip["start"].as_f64().unwrap_or(0.0);
    let clip_end = clip_start + clip["duration"].as_f64().unwrap_or(0.0);
    let clip_track = track_rank(clip, 1);
    let mut steps: Vec<Step> = Vec::new();

    let color = &clip["color"];
    if !color.is_null() {
        let hue = color["hue"].as_f64().unwrap_or(0.0).clamp(-180.0, 180.0);
        let saturation = (color["saturation"].as_f64().unwrap_or(100.0) / 100.0).clamp(0.0, 2.0);
        let brightness = (color["brightness"].as_f64().unwrap_or(0.0) / 100.0).clamp(-1.0, 1.0);
        let contrast = (color["contrast"].as_f64().unwrap_or(100.0) / 100.0).clamp(0.0, 2.0);
        let sharpness = (color["sharpness"].as_f64().unwrap_or(0.0) / 100.0).clamp(0.0, 1.0) * 1.5;
        if hue.abs() > 0.001 || (saturation - 1.0).abs() > 0.001 {
            steps.push(Step::Chain(format!("hue=h={hue:.3}:s={saturation:.3}")));
        }
        if brightness.abs() > 0.001 || (contrast - 1.0).abs() > 0.001 {
            steps.push(Step::Chain(format!("eq=brightness={brightness:.3}:contrast={contrast:.3}")));
        }
        if sharpness > 0.001 {
            steps.push(Step::Chain(format!("unsharp=5:5:{sharpness:.3}:5:5:0")));
        }
    }

    let mut in_rgba = false;

    for effect in clips.iter().filter(|item| item["kind"] == "effect") {
        let name = effect["effectName"].as_str().unwrap_or("");
        if let Some(target_id) = effect["targetClipId"].as_str() {
            let clip_id = clip["id"].as_str().unwrap_or("");
            let applies_to_clip = target_id == clip_id
                || (name == "Cross dissolve"
                    && effect["outgoingClipId"].as_str() == Some(clip_id));
            if !applies_to_clip {
                continue;
            }
        }
        if name == "Cross dissolve" || track_rank(effect, 2) <= clip_track {
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
        let fade_start = (effect_start - clip_start).max(0.0);
        let fade_end = (effect_end - clip_start).min(clip_end - clip_start);
        let shift = clip_start - effect_start;
        let ctx = EffectCtx {
            effect,
            shift,
            cfg,
            enable: filter_enable(local_start, local_end),
            local_start,
            local_end,
        };

        match name {
            "Fade" => {
                in_rgba = false;
                steps.push(Step::Chain(fade_filter(&ctx, fade_start, fade_end)));
            }
            "Soft glow" | "Bloom" => {
                in_rgba = false;
                steps.push(glow_step(&ctx, name));
            }
            "Swirl" | "Wave" | "Chromatic Aberration" | "Glitch" | "Pixelate" | "Mirror" | "Shake" | "Scanlines" => {
                if !in_rgba {
                    steps.push(Step::Chain("format=rgba".into()));
                    in_rgba = true;
                }
                steps.push(Step::Chain(match name {
                    "Swirl" => swirl_filter(&ctx),
                    "Wave" => wave_filter(&ctx),
                    "Glitch" => glitch_filter(&ctx),
                    "Pixelate" => pixelate_filter(&ctx),
                    "Mirror" => mirror_filter(&ctx),
                    "Shake" => shake_filter(&ctx),
                    "Scanlines" => scanlines_filter(&ctx),
                    _ => chromatic_filter(&ctx),
                }));
            }
            "Strobe" => {
                in_rgba = false;
                steps.push(Step::Chain(strobe_filter(&ctx)));
            }
            _ => {
                let segments = sample_segments(effect, local_start, local_end, shift, cfg.fps);
                let last = segments.len().saturating_sub(1);
                for (index, (start, end, intensity)) in segments.into_iter().enumerate() {
                    let end = if index == last { end } else { (end - 0.0005).max(start) };
                    let produced = static_effect_filters(name, intensity, &filter_enable(start, end));
                    if !produced.is_empty() {
                        in_rgba = false;
                    }
                    steps.extend(produced.into_iter().map(Step::Chain));
                }
            }
        }
    }

    // Cross dissolve (export side): the incoming clip fades its alpha in and the
    // outgoing clip fades its alpha out across the dissolve window. This only looks
    // like a dissolve if the two clips actually overlap in time and the outgoing
    // one is composited underneath the incoming one (overlay).
    for effect in clips.iter().filter(|item| item["kind"] == "effect" && item["effectName"] == "Cross dissolve") {
        if let Some(target_id) = effect["targetClipId"].as_str() {
            let clip_id = clip["id"].as_str().unwrap_or("");
            if target_id != clip_id && effect["outgoingClipId"].as_str() != Some(clip_id) {
                continue;
            }
        }
        if track_rank(effect, 2) < clip_track {
            continue;
        }
        let window_start = effect["start"].as_f64().unwrap_or(0.0);
        let window_end = window_start + effect["duration"].as_f64().unwrap_or(0.0);
        if window_end <= clip_start || window_start >= clip_end {
            continue;
        }
        let fade = if clip_start >= window_start - 0.001 {
            let duration = window_end.min(clip_end) - clip_start;
            (duration > 0.001).then(|| format!("fade=t=in:st=0:d={duration:.4}:alpha=1"))
        } else if clip_end <= window_end + 0.001 {
            let from = window_start.max(clip_start) - clip_start;
            let duration = clip_end - clip_start - from;
            (duration > 0.001).then(|| format!("fade=t=out:st={from:.4}:d={duration:.4}:alpha=1"))
        } else {
            None
        };
        if let Some(fade) = fade {
            if !in_rgba {
                steps.push(Step::Chain("format=yuva420p".into()));
            }
            steps.push(Step::Chain(fade));
            break;
        }
    }
    steps
}
