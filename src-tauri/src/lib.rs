mod ai;
mod video;
mod watermark;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropInsets {
    /// Fraction of width/height to trim from each edge (0..=0.45)
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkOptions {
    pub text: String,
    pub position: String,
    pub opacity: f32,
    pub font_scale: f32,
    pub text_color: String,
    pub stroke_color: String,
    pub margin_ratio: f32,
    /// Optional absolute path to a .ttf / .otf font
    pub font_path: Option<String>,
    /// "single" | "tiled" | "moving"（单点水印按帧在画面里来回跑）
    #[serde(default)]
    pub style: Option<String>,
    /// Optional edge crop before watermark
    #[serde(default)]
    pub crop: Option<CropInsets>,
    /// Frame index for moving watermark (batch sequence)
    #[serde(default)]
    pub frame_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobResult {
    pub input: String,
    pub output: Option<String>,
    pub ok: bool,
    pub error: Option<String>,
}

#[tauri::command]
fn default_watermark_text() -> String {
    "red.aiplanet.me/".to_string()
}

#[tauri::command]
fn list_system_fonts() -> Vec<String> {
    watermark::usable_fonts()
}

#[tauri::command]
fn apply_watermark_one(
    input: String,
    output_dir: String,
    options: WatermarkOptions,
) -> Result<JobResult, String> {
    let out_dir = PathBuf::from(&output_dir);
    if !out_dir.is_dir() {
        return Err(format!("输出目录不存在：{output_dir}"));
    }
    match watermark::apply_media_watermark(PathBuf::from(&input), &out_dir, &options) {
        Ok(path) => Ok(JobResult {
            input,
            output: Some(path.to_string_lossy().to_string()),
            ok: true,
            error: None,
        }),
        Err(e) => Ok(JobResult {
            input,
            output: None,
            ok: false,
            error: Some(e),
        }),
    }
}

#[tauri::command]
fn default_output_dir() -> String {
    if let Some(dir) = dirs::download_dir() {
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
        if dir.is_dir() {
            return dir.to_string_lossy().to_string();
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    for name in ["Downloads", "下载"] {
        let downloads = PathBuf::from(&home).join(name);
        if downloads.is_dir() {
            return downloads.to_string_lossy().to_string();
        }
    }
    let downloads = PathBuf::from(&home).join("Downloads");
    let _ = std::fs::create_dir_all(&downloads);
    if downloads.is_dir() {
        downloads.to_string_lossy().to_string()
    } else {
        home
    }
}

#[tauri::command]
async fn apply_watermark_batch(
    inputs: Vec<String>,
    output_dir: String,
    options: WatermarkOptions,
) -> Result<Vec<JobResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let out_dir = PathBuf::from(&output_dir);
        if !out_dir.is_dir() {
            return Err(format!("输出目录不存在：{output_dir}"));
        }
        if inputs.is_empty() {
            return Err("请先选择图片或视频".into());
        }
        let mut results = Vec::with_capacity(inputs.len());
        for (i, input) in inputs.into_iter().enumerate() {
            let mut opts = options.clone();
            // For still images in a batch, moving style uses queue index as frame.
            // For videos, ffmpeg uses its own frame clock.
            opts.frame_index = Some(i as u32);
            let res = match watermark::apply_media_watermark(PathBuf::from(&input), &out_dir, &opts)
            {
                Ok(path) => JobResult {
                    input,
                    output: Some(path.to_string_lossy().to_string()),
                    ok: true,
                    error: None,
                },
                Err(e) => JobResult {
                    input,
                    output: None,
                    ok: false,
                    error: Some(e),
                },
            };
            results.push(res);
        }
        Ok(results)
    })
    .await
    .map_err(|e| format!("任务中断：{e}"))?
}

#[tauri::command]
fn list_media_files(dir: String) -> Result<Vec<String>, String> {
    let root = PathBuf::from(&dir);
    if !root.is_dir() {
        return Err(format!("不是有效文件夹：{dir}"));
    }
    let mut out = Vec::new();
    collect_media(&root, &mut out, 0)?;
    out.sort();
    Ok(out)
}

fn collect_media(dir: &PathBuf, out: &mut Vec<String>, depth: u32) -> Result<(), String> {
    if depth > 6 {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("读取目录失败：{e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_media(&path, out, depth + 1)?;
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let ext = ext.to_ascii_lowercase();
        if matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tif" | "tiff"
                | "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm"
        ) {
            out.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
}

#[tauri::command]
async fn ai_chat(request: ai::AiChatRequest) -> Result<ai::AiChatResponse, String> {
    ai::chat(request).await
}

#[cfg(target_os = "macos")]
fn set_macos_dock_icon() {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    // Transparent-corner PNG (not the full-bleed .icns Tauri embeds for Ready).
    const ICON_PNG: &[u8] = include_bytes!("../icons/icon.png");
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let ns_app = NSApplication::sharedApplication(mtm);
    let data = NSData::with_bytes(ICON_PNG);
    if let Some(img) = NSImage::initWithData(NSImage::alloc(), &data) {
        unsafe { ns_app.setApplicationIconImage(Some(&img)) };
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            default_watermark_text,
            list_system_fonts,
            apply_watermark_one,
            apply_watermark_batch,
            list_media_files,
            default_output_dir,
            ai_chat,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // Tauri itself sets Dock icon from .icns on Ready (full-bleed square).
            // Re-apply our transparent PNG after that so corners stay cut out.
            if let tauri::RunEvent::Ready = event {
                #[cfg(target_os = "macos")]
                set_macos_dock_icon();
            }
        });
}
