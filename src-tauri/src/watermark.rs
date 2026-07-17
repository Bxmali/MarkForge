use crate::WatermarkOptions;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};

const POSITIONS: &[&str] = &[
    "top-left",
    "top-center",
    "top-right",
    "center-left",
    "center",
    "center-right",
    "bottom-left",
    "bottom-center",
    "bottom-right",
];

pub fn candidate_fonts() -> Vec<String> {
    vec![
        // Prefer single-face TTF/OTF (TTC face index is awkward with ab_glyph)
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf".into(),
        "/Library/Fonts/Arial Unicode.ttf".into(),
        "/System/Library/Fonts/Supplemental/Arial.ttf".into(),
        "/System/Library/Fonts/Helvetica.ttc".into(),
        "/System/Library/Fonts/Supplemental/Songti.ttc".into(),
        "/System/Library/Fonts/Hiragino Sans GB.ttc".into(),
        "/System/Library/Fonts/STHeiti Light.ttc".into(),
        "/usr/share/fonts/truetype/noto/NotoSansSC-Regular.otf".into(),
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc".into(),
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".into(),
        "C:/Windows/Fonts/msyh.ttf".into(),
        "C:/Windows/Fonts/msyh.ttc".into(),
        "C:/Windows/Fonts/arial.ttf".into(),
    ]
}

/// Fonts that ab_glyph can open and that can outline at least one sample char.
pub fn usable_fonts() -> Vec<String> {
    candidate_fonts()
        .into_iter()
        .filter(|p| font_usable(Path::new(p), "Aa水印"))
        .collect()
}

fn font_usable(path: &Path, sample: &str) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let Ok(font) = FontRef::try_from_slice(&bytes) else {
        return false;
    };
    font_can_draw(&font, sample)
}

fn font_can_draw(font: &FontRef, text: &str) -> bool {
    let scale = PxScale::from(32.0);
    let scaled = font.as_scaled(scale);
    text.chars().any(|ch| {
        let id = scaled.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(0.0, scaled.ascent()));
        font.outline_glyph(glyph).is_some()
    })
}

fn resolve_font_bytes(options: &WatermarkOptions, text: &str) -> Result<(Vec<u8>, String), String> {
    let sample = if text.chars().any(|c| !c.is_ascii()) {
        text
    } else {
        "AaWwMm"
    };

    if let Some(p) = options.font_path.as_ref().filter(|s| !s.trim().is_empty()) {
        let path = PathBuf::from(p);
        if path.is_file() {
            let bytes = fs::read(&path).map_err(|e| format!("读取字体失败：{e}"))?;
            if let Ok(font) = FontRef::try_from_slice(&bytes) {
                if font_can_draw(&font, sample) {
                    return Ok((bytes, p.clone()));
                }
            }
            // fall through to auto-pick if user font can't draw this text
        }
    }

    for p in candidate_fonts() {
        let path = PathBuf::from(&p);
        if !path.is_file() {
            continue;
        }
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(font) = FontRef::try_from_slice(&bytes) {
                if font_can_draw(&font, sample) {
                    return Ok((bytes, p));
                }
            }
        }
    }
    Err("未找到可用字体（当前文案无法渲染）。请安装 Arial Unicode 或指定 .ttf/.otf。".into())
}

fn parse_hex_color(raw: &str, fallback: [u8; 3]) -> [u8; 3] {
    let s = raw.trim().trim_start_matches('#');
    if s.len() == 6 {
        if let Ok(bytes) = hex::decode(s) {
            if bytes.len() == 3 {
                return [bytes[0], bytes[1], bytes[2]];
            }
        }
    }
    fallback
}

fn blend_rgb(base: [u8; 3], overlay: [u8; 3], opacity: f32) -> Rgb<u8> {
    let o = opacity.clamp(0.05, 1.0);
    Rgb([
        ((base[0] as f32) * (1.0 - o) + (overlay[0] as f32) * o).round() as u8,
        ((base[1] as f32) * (1.0 - o) + (overlay[1] as f32) * o).round() as u8,
        ((base[2] as f32) * (1.0 - o) + (overlay[2] as f32) * o).round() as u8,
    ])
}

fn draw_text_rgb(
    img: &mut RgbImage,
    color: [u8; 3],
    opacity: f32,
    x: i32,
    y: i32,
    scale: PxScale,
    font: &FontRef,
    text: &str,
) {
    use ab_glyph::point;
    let opacity = opacity.clamp(0.05, 1.0);
    let scaled = font.as_scaled(scale);
    let mut caret = 0.0f32;
    let image_w = img.width() as i32;
    let image_h = img.height() as i32;

    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        let glyph = glyph_id.with_scale_and_position(scale, point(caret, scaled.ascent()));
        caret += scaled.h_advance(glyph_id);
        let Some(outlined) = font.outline_glyph(glyph) else {
            continue;
        };
        let bb = outlined.px_bounds();
        let x_shift = x + bb.min.x.round() as i32;
        let y_shift = y + bb.min.y.round() as i32;
        outlined.draw(|gx, gy, gv| {
            let ix = gx as i32 + x_shift;
            let iy = gy as i32 + y_shift;
            if (0..image_w).contains(&ix) && (0..image_h).contains(&iy) {
                let px = img.get_pixel(ix as u32, iy as u32).0;
                let covered = (gv.clamp(0.0, 1.0) * opacity).clamp(0.0, 1.0);
                img.put_pixel(ix as u32, iy as u32, blend_rgb(px, color, covered));
            }
        });
    }
}

fn xy_for_position(
    position: &str,
    media_w: u32,
    media_h: u32,
    box_w: i32,
    box_h: i32,
    margin: i32,
) -> (i32, i32) {
    let mw = media_w as i32;
    let mh = media_h as i32;
    match position {
        "top-left" => (margin, margin),
        "top-center" => ((mw - box_w).max(margin) / 2 + margin / 2, margin),
        "top-right" => ((mw - box_w - margin).max(margin), margin),
        "center-left" => (margin, ((mh - box_h) / 2).max(margin)),
        "center" => (
            ((mw - box_w) / 2).max(margin),
            ((mh - box_h) / 2).max(margin),
        ),
        "center-right" => (
            (mw - box_w - margin).max(margin),
            ((mh - box_h) / 2).max(margin),
        ),
        "bottom-left" => (margin, (mh - box_h - margin).max(margin)),
        "bottom-center" => (
            ((mw - box_w) / 2).max(margin),
            (mh - box_h - margin).max(margin),
        ),
        _ => (
            (mw - box_w - margin).max(margin),
            (mh - box_h - margin).max(margin),
        ),
    }
}

fn approx_text_size(font: &FontRef, scale: PxScale, text: &str) -> (i32, i32) {
    let scaled = font.as_scaled(scale);
    let mut width = 0.0f32;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        width += scaled.h_advance(id);
    }
    let height = scaled.height();
    (width.ceil() as i32, height.ceil() as i32)
}

fn apply_crop(img: RgbaImage, crop: Option<&crate::CropInsets>) -> Result<RgbaImage, String> {
    let Some(c) = crop else {
        return Ok(img);
    };
    let (w, h) = img.dimensions();
    let left = (c.left.clamp(0.0, 0.45) * w as f32).round() as u32;
    let top = (c.top.clamp(0.0, 0.45) * h as f32).round() as u32;
    let right = (c.right.clamp(0.0, 0.45) * w as f32).round() as u32;
    let bottom = (c.bottom.clamp(0.0, 0.45) * h as f32).round() as u32;
    if left + right >= w || top + bottom >= h {
        return Err("裁剪范围过大，请缩小边距".into());
    }
    let cw = w - left - right;
    let ch = h - top - bottom;
    Ok(image::imageops::crop_imm(&img, left, top, cw, ch).to_image())
}

fn draw_text_with_stroke(
    img: &mut RgbImage,
    font: &FontRef,
    scale: PxScale,
    x: i32,
    y: i32,
    text: &str,
    fill: [u8; 3],
    stroke: [u8; 3],
    opacity: f32,
) {
    let offsets = [
        (-2, 0),
        (2, 0),
        (0, -2),
        (0, 2),
        (-1, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
        (-2, -1),
        (2, -1),
        (-2, 1),
        (2, 1),
        (-1, -2),
        (1, -2),
        (-1, 2),
        (1, 2),
    ];
    let stroke_op = (opacity + 0.15).min(1.0);
    for (dx, dy) in offsets {
        draw_text_rgb(img, stroke, stroke_op, x + dx, y + dy, scale, font, text);
    }
    draw_text_rgb(img, fill, opacity, x, y, scale, font, text);
}

fn draw_tiled_watermark(
    img: &mut RgbImage,
    font: &FontRef,
    scale: PxScale,
    text: &str,
    tw: i32,
    th: i32,
    fill: [u8; 3],
    stroke: [u8; 3],
    opacity: f32,
) {
    let (w, h) = img.dimensions();
    let step_x = (tw + (tw / 2).max(48)).max(96);
    let step_y = (th * 3).max(72);
    let mut row = 0i32;
    let mut y = -(th / 2);
    while y < h as i32 + th {
        let x_off = if row % 2 == 0 { 0 } else { step_x / 2 };
        let mut x = -tw + x_off;
        while x < w as i32 + tw {
            if x + tw > 0 && y + th > 0 && x < w as i32 && y < h as i32 {
                draw_text_with_stroke(img, font, scale, x, y, text, fill, stroke, opacity);
            }
            x += step_x;
        }
        y += step_y;
        row += 1;
    }
}

fn count_diff(a: &RgbImage, b: &RgbImage) -> u32 {
    a.pixels()
        .zip(b.pixels())
        .filter(|(p, q)| p != q)
        .count() as u32
}

fn bounce_xy(
    frame: u32,
    media_w: u32,
    media_h: u32,
    box_w: i32,
    box_h: i32,
    margin: i32,
) -> (i32, i32) {
    let usable_w = (media_w as i32 - box_w - margin * 2).max(1) as u32;
    let usable_h = (media_h as i32 - box_h - margin * 2).max(1) as u32;
    // DVD-style bounce: triangle waves with different periods
    let speed = 18u32;
    let period_x = (usable_w * 2).max(2);
    let period_y = (usable_h * 2).max(2);
    let tx = (frame.saturating_mul(speed)) % period_x;
    let ty = (frame.saturating_mul(speed.saturating_mul(11) / 17)) % period_y;
    let x_off = if tx <= usable_w {
        tx
    } else {
        period_x - tx
    };
    let y_off = if ty <= usable_h {
        ty
    } else {
        period_y - ty
    };
    (margin + x_off as i32, margin + y_off as i32)
}

fn output_path(input: &Path, out_dir: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let out_ext = if matches!(ext.as_str(), "jpg" | "jpeg") {
        "jpg"
    } else {
        "png"
    };
    let mut name = format!("{stem}_wm.{out_ext}");
    let mut dest = out_dir.join(&name);
    let mut i = 2;
    while dest.exists() {
        name = format!("{stem}_wm_{i}.{out_ext}");
        dest = out_dir.join(&name);
        i += 1;
    }
    dest
}

pub fn apply_image_watermark(
    input: PathBuf,
    out_dir: &Path,
    options: &WatermarkOptions,
) -> Result<PathBuf, String> {
    let dest = output_path(&input, out_dir);
    apply_image_watermark_to(input, dest.clone(), options)?;
    Ok(dest)
}

/// Watermark a still image into an explicit destination path (used by video frame pipeline).
pub fn apply_image_watermark_to(
    input: PathBuf,
    dest: PathBuf,
    options: &WatermarkOptions,
) -> Result<(), String> {
    if !input.is_file() {
        return Err(format!("文件不存在：{}", input.display()));
    }
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tif" | "tiff"
    ) {
        return Err(format!("不是图片文件：{}", input.display()));
    }

    let text = {
        let t = options.text.trim();
        if t.is_empty() {
            "red.aiplanet.me/"
        } else {
            t
        }
    };
    let position = if POSITIONS.contains(&options.position.as_str()) {
        options.position.as_str()
    } else {
        "bottom-right"
    };
    let opacity = options.opacity.clamp(0.28, 0.95);
    let font_scale = options.font_scale.clamp(0.022, 0.08);
    let margin_ratio = options.margin_ratio.clamp(0.015, 0.12);

    let (font_bytes, _font_used) = resolve_font_bytes(options, text)?;
    let font =
        FontRef::try_from_slice(&font_bytes).map_err(|e| format!("字体解析失败：{e}"))?;

    let img = image::open(&input).map_err(|e| format!("打开图片失败：{e}"))?;
    let rgba = apply_crop(img.to_rgba8(), options.crop.as_ref())?;
    let mut rgb: RgbImage = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    let before = rgb.clone();
    let (w, h) = rgb.dimensions();
    let short = w.min(h) as f32;
    let font_size = (short * font_scale).max(18.0);
    let scale = PxScale::from(font_size);
    let (tw, th) = approx_text_size(&font, scale, text);
    if tw < 2 || th < 2 {
        return Err(format!(
            "水印文案无法测量尺寸（字体可能不支持这些字符）：{text}"
        ));
    }
    let margin = (short * margin_ratio).round() as i32;

    let fill_rgb = parse_hex_color(&options.text_color, [255, 255, 255]);
    let stroke_rgb = parse_hex_color(&options.stroke_color, [0, 0, 0]);

    let style = options
        .style
        .as_deref()
        .unwrap_or("single")
        .trim()
        .to_ascii_lowercase();
    if style == "tiled" || style == "diagonal" {
        draw_tiled_watermark(
            &mut rgb, &font, scale, text, tw, th, fill_rgb, stroke_rgb, opacity,
        );
    } else if style == "moving" || style == "bounce" || style == "dynamic" {
        let frame = options.frame_index.unwrap_or(0);
        let (x, y) = bounce_xy(frame, w, h, tw, th, margin);
        draw_text_with_stroke(
            &mut rgb, &font, scale, x, y, text, fill_rgb, stroke_rgb, opacity,
        );
    } else {
        let (x, y) = xy_for_position(position, w, h, tw, th, margin);
        draw_text_with_stroke(
            &mut rgb, &font, scale, x, y, text, fill_rgb, stroke_rgb, opacity,
        );
    }

    let changed = count_diff(&before, &rgb);
    if changed < 30 {
        return Err(
            "水印没有画到图上（字体/字符可能不支持）。请换字体或改成英文/数字文案后重试。"
                .into(),
        );
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败：{e}"))?;
    }
    match dest
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => {
            rgb.save(&dest).map_err(|e| format!("保存失败：{e}"))?;
        }
        _ => {
            image::DynamicImage::ImageRgb8(rgb)
                .to_rgba8()
                .save(&dest)
                .map_err(|e| format!("保存失败：{e}"))?;
        }
    }
    Ok(())
}

/// Transparent full-frame PNG for ffmpeg overlay (no drawtext required).
pub fn render_video_watermark_png(
    width: u32,
    height: u32,
    options: &WatermarkOptions,
    dest: &Path,
) -> Result<(), String> {
    if width < 16 || height < 16 {
        return Err("视频分辨率过小".into());
    }
    let text = {
        let t = options.text.trim();
        if t.is_empty() {
            "red.aiplanet.me/"
        } else {
            t
        }
    };
    let position = if POSITIONS.contains(&options.position.as_str()) {
        options.position.as_str()
    } else {
        "bottom-right"
    };
    let opacity = options.opacity.clamp(0.28, 0.95);
    let font_scale = options.font_scale.clamp(0.022, 0.08);
    let margin_ratio = options.margin_ratio.clamp(0.015, 0.12);

    let (font_bytes, _) = resolve_font_bytes(options, text)?;
    let font = FontRef::try_from_slice(&font_bytes).map_err(|e| format!("字体解析失败：{e}"))?;

    let short = width.min(height) as f32;
    let font_size = (short * font_scale).max(18.0);
    let scale = PxScale::from(font_size);
    let (tw, th) = approx_text_size(&font, scale, text);
    if tw < 2 || th < 2 {
        return Err(format!("水印文案无法测量尺寸：{text}"));
    }
    let margin = (short * margin_ratio).round() as i32;
    let fill_rgb = parse_hex_color(&options.text_color, [255, 255, 255]);
    let stroke_rgb = parse_hex_color(&options.stroke_color, [0, 0, 0]);

    let style = options
        .style
        .as_deref()
        .unwrap_or("single")
        .trim()
        .to_ascii_lowercase();

    // Moving: tight glyph PNG; ffmpeg animates overlay position.
    if style == "moving" || style == "bounce" || style == "dynamic" {
        let pad = 8u32;
        let gw = (tw as u32 + pad * 2).max(8);
        let gh = (th as u32 + pad * 2).max(8);
        let mut layer = RgbaImage::from_pixel(gw, gh, Rgba([0, 0, 0, 0]));
        draw_text_with_stroke_rgba(
            &mut layer,
            &font,
            scale,
            pad as i32,
            pad as i32,
            text,
            fill_rgb,
            stroke_rgb,
            opacity,
        );
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建临时目录失败：{e}"))?;
        }
        layer
            .save(dest)
            .map_err(|e| format!("保存水印层失败：{e}"))?;
        return Ok(());
    }

    let mut layer = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    if style == "tiled" || style == "diagonal" {
        draw_tiled_watermark_rgba(
            &mut layer, &font, scale, text, tw, th, fill_rgb, stroke_rgb, opacity,
        );
    } else {
        let (x, y) = xy_for_position(position, width, height, tw, th, margin);
        draw_text_with_stroke_rgba(
            &mut layer, &font, scale, x, y, text, fill_rgb, stroke_rgb, opacity,
        );
    }

    let opaque = layer.pixels().filter(|p| p.0[3] > 8).count();
    if opaque < 30 {
        return Err("水印层为空（字体/字符可能不支持）".into());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建临时目录失败：{e}"))?;
    }
    layer
        .save(dest)
        .map_err(|e| format!("保存水印层失败：{e}"))?;
    Ok(())
}

fn blend_rgba(dst: &mut [u8; 4], color: [u8; 3], covered: f32) {
    let src_a = covered.clamp(0.0, 1.0);
    if src_a <= 0.001 {
        return;
    }
    let dst_a = dst[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.001 {
        *dst = [0, 0, 0, 0];
        return;
    }
    for i in 0..3 {
        let c = color[i] as f32;
        let d = dst[i] as f32;
        dst[i] = ((c * src_a + d * dst_a * (1.0 - src_a)) / out_a).round() as u8;
    }
    dst[3] = (out_a * 255.0).round() as u8;
}

fn draw_text_rgba(
    img: &mut RgbaImage,
    color: [u8; 3],
    opacity: f32,
    x: i32,
    y: i32,
    scale: PxScale,
    font: &FontRef,
    text: &str,
) {
    use ab_glyph::point;
    let opacity = opacity.clamp(0.05, 1.0);
    let scaled = font.as_scaled(scale);
    let mut caret = 0.0f32;
    let image_w = img.width() as i32;
    let image_h = img.height() as i32;

    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        let glyph = glyph_id.with_scale_and_position(scale, point(caret, scaled.ascent()));
        caret += scaled.h_advance(glyph_id);
        let Some(outlined) = font.outline_glyph(glyph) else {
            continue;
        };
        let bb = outlined.px_bounds();
        let x_shift = x + bb.min.x.round() as i32;
        let y_shift = y + bb.min.y.round() as i32;
        outlined.draw(|gx, gy, gv| {
            let ix = gx as i32 + x_shift;
            let iy = gy as i32 + y_shift;
            if (0..image_w).contains(&ix) && (0..image_h).contains(&iy) {
                let covered = (gv.clamp(0.0, 1.0) * opacity).clamp(0.0, 1.0);
                let px = img.get_pixel_mut(ix as u32, iy as u32);
                blend_rgba(&mut px.0, color, covered);
            }
        });
    }
}

fn draw_text_with_stroke_rgba(
    img: &mut RgbaImage,
    font: &FontRef,
    scale: PxScale,
    x: i32,
    y: i32,
    text: &str,
    fill: [u8; 3],
    stroke: [u8; 3],
    opacity: f32,
) {
    let offsets = [
        (-2, 0),
        (2, 0),
        (0, -2),
        (0, 2),
        (-1, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
        (-2, -1),
        (2, -1),
        (-2, 1),
        (2, 1),
        (-1, -2),
        (1, -2),
        (-1, 2),
        (1, 2),
    ];
    let stroke_op = (opacity + 0.15).min(1.0);
    for (dx, dy) in offsets {
        draw_text_rgba(img, stroke, stroke_op, x + dx, y + dy, scale, font, text);
    }
    draw_text_rgba(img, fill, opacity, x, y, scale, font, text);
}

fn draw_tiled_watermark_rgba(
    img: &mut RgbaImage,
    font: &FontRef,
    scale: PxScale,
    text: &str,
    tw: i32,
    th: i32,
    fill: [u8; 3],
    stroke: [u8; 3],
    opacity: f32,
) {
    let (w, h) = img.dimensions();
    let step_x = (tw + (tw / 2).max(48)).max(96);
    let step_y = (th * 3).max(72);
    let mut row = 0i32;
    let mut y = -(th / 2);
    while y < h as i32 + th {
        let x_off = if row % 2 == 0 { 0 } else { step_x / 2 };
        let mut x = -tw + x_off;
        while x < w as i32 + tw {
            if x + tw > 0 && y + th > 0 && x < w as i32 && y < h as i32 {
                draw_text_with_stroke_rgba(img, font, scale, x, y, text, fill, stroke, opacity);
            }
            x += step_x;
        }
        y += step_y;
        row += 1;
    }
}

pub fn apply_media_watermark(
    input: PathBuf,
    out_dir: &Path,
    options: &WatermarkOptions,
) -> Result<PathBuf, String> {
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(
        ext.as_str(),
        "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm"
    ) {
        return crate::video::apply_video_watermark(input, out_dir, options);
    }
    apply_image_watermark(input, out_dir, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WatermarkOptions;

    #[test]
    fn watermark_changes_pixels() {
        let dir = std::env::temp_dir().join("markforge_wm_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.png");
        let img = image::RgbImage::from_pixel(640, 480, image::Rgb([20, 90, 160]));
        img.save(&input).unwrap();

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
        let out = apply_image_watermark(input.clone(), &dir, &opts).expect("wm");
        assert!(out.exists());
        let before = image::open(&input).unwrap().to_rgb8();
        let after = image::open(&out).unwrap().to_rgb8();
        let diff = count_diff(&before, &after);
        assert!(diff > 100, "expected visible watermark pixels, diff={diff}");
    }
}
