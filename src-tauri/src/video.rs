use crate::watermark;
use crate::WatermarkOptions;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_bin(names: &[&str]) -> Result<PathBuf, String> {
    for candidate in names {
        let status = Command::new(candidate)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if matches!(status, Ok(s) if s.success()) {
            return Ok(PathBuf::from(candidate));
        }
    }
    Err("未找到 ffmpeg / ffprobe。请先安装：brew install ffmpeg".into())
}

fn find_ffmpeg() -> Result<PathBuf, String> {
    find_bin(&[
        "ffmpeg",
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
    ])
}

fn find_ffprobe() -> Result<PathBuf, String> {
    find_bin(&[
        "ffprobe",
        "/opt/homebrew/bin/ffprobe",
        "/usr/local/bin/ffprobe",
        "/usr/bin/ffprobe",
    ])
}

fn run_cmd(bin: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("无法启动 ffmpeg：{e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let msg: String = stderr.chars().rev().take(800).collect::<String>().chars().rev().collect();
    Err(format!("ffmpeg 失败：{msg}"))
}

fn probe_size(input: &Path) -> Result<(u32, u32), String> {
    let ffprobe = find_ffprobe()?;
    let out = Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=s=x:p=0",
            input.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("无法启动 ffprobe：{e}"))?;
    if !out.status.success() {
        return Err("读取视频分辨率失败".into());
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let line = raw.lines().next().unwrap_or("").trim();
    let mut parts = line.split('x');
    let w: u32 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| format!("无法解析分辨率：{line}"))?;
    let h: u32 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| format!("无法解析分辨率：{line}"))?;
    Ok((w.max(2), h.max(2)))
}

fn output_video_path(input: &Path, out_dir: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let mut name = format!("{stem}_wm.mp4");
    let mut dest = out_dir.join(&name);
    let mut i = 2;
    while dest.exists() {
        name = format!("{stem}_wm_{i}.mp4");
        dest = out_dir.join(&name);
        i += 1;
    }
    dest
}

fn crop_geometry(
    w: u32,
    h: u32,
    options: &WatermarkOptions,
) -> Option<(u32, u32, u32, u32)> {
    let c = options.crop.as_ref()?;
    let left = (c.left.clamp(0.0, 0.45) * w as f32).round() as u32;
    let top = (c.top.clamp(0.0, 0.45) * h as f32).round() as u32;
    let right = (c.right.clamp(0.0, 0.45) * w as f32).round() as u32;
    let bottom = (c.bottom.clamp(0.0, 0.45) * h as f32).round() as u32;
    if left + right >= w || top + bottom >= h {
        return None;
    }
    Some((w - left - right, h - top - bottom, left, top))
}

/// Fast path: render one transparent PNG watermark + ffmpeg overlay.
/// Avoids drawtext (missing on many Homebrew builds) and per-frame Rust loops.
pub fn apply_video_watermark(
    input: PathBuf,
    out_dir: &Path,
    options: &WatermarkOptions,
) -> Result<PathBuf, String> {
    if !input.is_file() {
        return Err(format!("文件不存在：{}", input.display()));
    }
    let ffmpeg = find_ffmpeg()?;
    let (vw, vh) = probe_size(&input)?;
    let (layer_w, layer_h, crop) = match crop_geometry(vw, vh, options) {
        Some((cw, ch, x, y)) => (cw, ch, Some((cw, ch, x, y))),
        None => (vw, vh, None),
    };

    let work = std::env::temp_dir().join(format!(
        "markforge_vid_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&work).map_err(|e| format!("创建临时目录失败：{e}"))?;
    let layer = work.join("wm.png");
    let cleanup = |work: &PathBuf| {
        let _ = fs::remove_dir_all(work);
    };

    if let Err(e) = watermark::render_video_watermark_png(layer_w, layer_h, options, &layer) {
        cleanup(&work);
        return Err(e);
    }

    let style = options
        .style
        .as_deref()
        .unwrap_or("single")
        .trim()
        .to_ascii_lowercase();
    let moving = matches!(style.as_str(), "moving" | "bounce" | "dynamic");

    // Moving: bounce-like path via triangle waves on overlay x/y.
    let overlay_xy = if moving {
        "overlay=x='if(lt(mod(t*80\\,2*(W-w))\\,(W-w))\\,mod(t*80\\,2*(W-w))\\,2*(W-w)-mod(t*80\\,2*(W-w)))':y='if(lt(mod(t*52\\,2*(H-h))\\,(H-h))\\,mod(t*52\\,2*(H-h))\\,2*(H-h)-mod(t*52\\,2*(H-h)))'"
            .to_string()
    } else {
        "overlay=0:0".to_string()
    };

    let filter = if let Some((cw, ch, x, y)) = crop {
        format!("[0:v]crop={cw}:{ch}:{x}:{y}[base];[base][1:v]{overlay_xy}[v]")
    } else {
        format!("[0:v][1:v]{overlay_xy}[v]")
    };

    let dest = output_video_path(&input, out_dir);
    let dest_str = dest.to_string_lossy().to_string();
    let input_str = input.to_string_lossy().to_string();
    let layer_str = layer.to_string_lossy().to_string();

    let with_audio = run_cmd(
        &ffmpeg,
        &[
            "-y",
            "-i",
            &input_str,
            "-i",
            &layer_str,
            "-filter_complex",
            &filter,
            "-map",
            "[v]",
            "-map",
            "0:a?",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "copy",
            "-shortest",
            &dest_str,
        ],
    );

    if with_audio.is_err() {
        if let Err(e) = run_cmd(
            &ffmpeg,
            &[
                "-y",
                "-i",
                &input_str,
                "-i",
                &layer_str,
                "-filter_complex",
                &filter,
                "-map",
                "[v]",
                "-an",
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "20",
                "-pix_fmt",
                "yuv420p",
                &dest_str,
            ],
        ) {
            cleanup(&work);
            return Err(e);
        }
    }

    cleanup(&work);
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WatermarkOptions;

    #[test]
    fn video_overlay_smoke() {
        let dir = std::env::temp_dir().join("markforge_video_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.mp4");
        let ffmpeg = find_ffmpeg().expect("ffmpeg");
        run_cmd(
            &ffmpeg,
            &[
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=640x360:d=1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                input.to_str().unwrap(),
            ],
        )
        .expect("gen video");

        let opts = WatermarkOptions {
            text: "red.aiplanet.me/".into(),
            position: "bottom-right".into(),
            opacity: 0.7,
            font_scale: 0.05,
            text_color: "#FFFFFF".into(),
            stroke_color: "#000000".into(),
            margin_ratio: 0.04,
            font_path: Some(
                "/System/Library/Fonts/Supplemental/Arial Unicode.ttf".into(),
            ),
            style: Some("single".into()),
            crop: None,
            frame_index: None,
        };
        let out = apply_video_watermark(input, &dir, &opts).expect("wm video");
        assert!(out.exists());
        assert!(out.metadata().unwrap().len() > 1000);
    }
}
