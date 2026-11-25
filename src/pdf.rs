use crate::model::{PdfExportMode, PdfOptions, Project, Slide};
use crate::project::normalize_transcript;
use ab_glyph::{Font, FontArc, FontVec, PxScale, ScaleFont, VariableFont};
use anyhow::{Context, Result};
use image::{GenericImageView, ImageEncoder, Rgb, RgbImage};
use imageproc::drawing::{draw_hollow_rect_mut, draw_line_segment_mut, draw_text_mut};
use imageproc::rect::Rect;
use printpdf::{
    Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, PdfWarnMsg, RawImage, XObjectTransform,
};
use std::fs;
use std::path::Path;

const FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/Mplus1.ttf");
const PAGE_WIDTH_PT: f64 = 595.2756;
const PAGE_HEIGHT_PT: f64 = 841.8898;
const MARGIN_PT: f64 = 28.0;
const RENDER_SCALE: f64 = 2.0;

struct TranscriptArea {
    x: f64,
    top: f64,
    width: f64,
    bottom: f64,
    minimum_font_size: f64,
}

#[derive(Debug)]
pub struct PdfLayoutError {
    pub slides: Vec<usize>,
    pub minimum_font_size: f64,
}

impl std::fmt::Display for PdfLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "文字量が多く、最小フォントサイズ {:.2}pt でも1ページに収まらないスライドがあります",
            self.minimum_font_size
        )
    }
}

impl std::error::Error for PdfLayoutError {}

pub fn export(project: &Project, output_path: &Path, options: PdfOptions) -> Result<()> {
    let mut font_vec =
        FontVec::try_from_vec(FONT_BYTES.to_vec()).context("同梱フォントを読み込めませんでした")?;
    font_vec.set_variation(b"wght", 650.0);
    let font = FontArc::from(font_vec);
    let mut document = PdfDocument::new(&format!("{} スライドノート", project.video_name));
    let mut overflow = Vec::new();
    for slide in &project.slides {
        if !page_fits(slide, options, &font) {
            overflow.push(slide.number);
            continue;
        }
        let image = render_page(slide, options, &font);
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 92).write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgb8,
        )?;
        let mut warnings = Vec::new();
        let raw = RawImage::decode_from_bytes(&jpeg, &mut warnings).map_err(anyhow::Error::msg)?;
        let id = document.add_image(&raw);
        document.pages.push(PdfPage::new(
            Mm(210.0),
            Mm(297.0),
            vec![Op::UseXobject {
                id,
                transform: XObjectTransform {
                    dpi: Some((72.0 * RENDER_SCALE) as f32),
                    ..Default::default()
                },
            }],
        ));
    }
    if !overflow.is_empty() {
        return Err(PdfLayoutError {
            slides: overflow,
            minimum_font_size: options.minimum_font_size,
        }
        .into());
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut warnings = Vec::<PdfWarnMsg>::new();
    let bytes = document.save(&PdfSaveOptions::default(), &mut warnings);
    fs::write(output_path, bytes)?;
    Ok(())
}

fn page_fits(slide: &Slide, options: PdfOptions, font: &FontArc) -> bool {
    if options.mode == PdfExportMode::SlidesOnly {
        return true;
    }
    let (top, bottom) = match options.mode {
        PdfExportMode::Combined => (PAGE_HEIGHT_PT / 2.0 + 36.0, PAGE_HEIGHT_PT - MARGIN_PT),
        PdfExportMode::TranscriptsOnly => (MARGIN_PT + 42.0, PAGE_HEIGHT_PT - MARGIN_PT),
        PdfExportMode::SlidesOnly => return true,
    };
    let text = transcript_or_empty(slide);
    let mut size = 10.5;
    while size >= options.minimum_font_size - 0.001 {
        let lines = wrap_text(&text, font, size, PAGE_WIDTH_PT - MARGIN_PT * 2.0);
        if lines.len() as f64 * size * 1.48 <= bottom - top {
            return true;
        }
        size -= 0.25;
    }
    false
}

fn render_page(slide: &Slide, options: PdfOptions, font: &FontArc) -> RgbImage {
    let width = (PAGE_WIDTH_PT * RENDER_SCALE).round() as u32;
    let height = (PAGE_HEIGHT_PT * RENDER_SCALE).round() as u32;
    let mut canvas = RgbImage::from_pixel(width, height, Rgb([255, 255, 255]));
    let content_width = PAGE_WIDTH_PT - MARGIN_PT * 2.0;
    match options.mode {
        PdfExportMode::Combined => {
            let half = PAGE_HEIGHT_PT / 2.0;
            draw_slide(
                &mut canvas,
                slide,
                MARGIN_PT,
                18.0,
                content_width,
                half - 36.0,
            );
            draw_line(
                &mut canvas,
                MARGIN_PT,
                half,
                PAGE_WIDTH_PT - MARGIN_PT,
                half,
                Rgb([220, 224, 230]),
            );
            draw_header(&mut canvas, slide, font, MARGIN_PT, half + 14.0);
            draw_transcript(
                &mut canvas,
                slide,
                font,
                TranscriptArea {
                    x: MARGIN_PT,
                    top: half + 36.0,
                    width: content_width,
                    bottom: PAGE_HEIGHT_PT - MARGIN_PT,
                    minimum_font_size: options.minimum_font_size,
                },
            );
        }
        PdfExportMode::SlidesOnly => {
            draw_slide(
                &mut canvas,
                slide,
                MARGIN_PT,
                30.0,
                content_width,
                PAGE_HEIGHT_PT - 100.0,
            );
            draw_footer(&mut canvas, slide, font);
        }
        PdfExportMode::TranscriptsOnly => {
            draw_header(&mut canvas, slide, font, MARGIN_PT, MARGIN_PT);
            draw_transcript(
                &mut canvas,
                slide,
                font,
                TranscriptArea {
                    x: MARGIN_PT,
                    top: MARGIN_PT + 42.0,
                    width: content_width,
                    bottom: PAGE_HEIGHT_PT - MARGIN_PT,
                    minimum_font_size: options.minimum_font_size,
                },
            );
        }
    }
    canvas
}

fn draw_slide(canvas: &mut RgbImage, slide: &Slide, x: f64, y: f64, width: f64, height: f64) {
    if let Ok(image) = image::open(&slide.image_path) {
        let (image_width, image_height) = image.dimensions();
        let ratio = (width / image_width as f64).min(height / image_height as f64);
        let draw_width = (image_width as f64 * ratio).max(1.0) as u32;
        let draw_height = (image_height as f64 * ratio).max(1.0) as u32;
        let resized = image
            .resize_exact(
                draw_width,
                draw_height,
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgb8();
        let draw_x = ((x + (width - draw_width as f64) / 2.0) * RENDER_SCALE).round() as i32;
        let draw_y = ((y + (height - draw_height as f64) / 2.0) * RENDER_SCALE).round() as i32;
        image::imageops::replace(canvas, &resized, draw_x as i64, draw_y as i64);
        draw_hollow_rect_mut(
            canvas,
            Rect::at(draw_x, draw_y).of_size(draw_width, draw_height),
            Rgb([210, 214, 220]),
        );
    } else {
        let missing_font = font();
        draw_text(
            canvas,
            &missing_font,
            "スライド画像が見つかりません",
            x,
            y + height / 2.0,
            10.0,
            Rgb([150, 150, 150]),
        );
    }
}

fn draw_header(canvas: &mut RgbImage, slide: &Slide, font: &FontArc, x: f64, y: f64) {
    let color = Rgb([80, 86, 96]);
    draw_text(canvas, font, "文字起こし", x, y, 9.0, color);
    let time = format!(
        "No. {:02}    {} – {}",
        slide.number,
        format_time(slide.start),
        format_time(slide.end)
    );
    let width = text_width(font, &time, 8.0);
    draw_text(
        canvas,
        font,
        &time,
        PAGE_WIDTH_PT - MARGIN_PT - width,
        y + 1.0,
        8.0,
        color,
    );
}

fn draw_footer(canvas: &mut RgbImage, slide: &Slide, font: &FontArc) {
    let text = format!(
        "No. {:02}    {} – {}",
        slide.number,
        format_time(slide.start),
        format_time(slide.end)
    );
    let width = text_width(font, &text, 8.0);
    draw_text(
        canvas,
        font,
        &text,
        (PAGE_WIDTH_PT - width) / 2.0,
        PAGE_HEIGHT_PT - MARGIN_PT - 4.0,
        8.0,
        Rgb([110, 116, 126]),
    );
}

fn draw_transcript(canvas: &mut RgbImage, slide: &Slide, font: &FontArc, area: TranscriptArea) {
    let text = transcript_or_empty(slide);
    let mut size = 10.5;
    loop {
        let lines = wrap_text(&text, font, size, area.width);
        if lines.len() as f64 * size * 1.48 <= area.bottom - area.top
            || size <= area.minimum_font_size + 0.001
        {
            let mut y = area.top;
            for line in lines {
                draw_text(canvas, font, &line, area.x, y, size, Rgb([32, 36, 43]));
                y += size * 1.48;
            }
            break;
        }
        size -= 0.25;
    }
}

fn transcript_or_empty(slide: &Slide) -> String {
    let text = normalize_transcript(&slide.transcript);
    if text.is_empty() {
        "（文字起こしなし）".to_string()
    } else {
        text
    }
}

fn wrap_text(text: &str, font: &FontArc, size: f64, max_width: f64) -> Vec<String> {
    let scaled = font.clone().into_scaled(size as f32);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut width = 0.0f32;
    for character in text.chars() {
        if character == '\n' {
            lines.push(std::mem::take(&mut line));
            width = 0.0;
            continue;
        }
        let advance = scaled.h_advance(scaled.glyph_id(character));
        if !line.is_empty() && width + advance > max_width as f32 {
            lines.push(std::mem::take(&mut line));
            width = 0.0;
        }
        line.push(character);
        width += advance;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn text_width(font: &FontArc, text: &str, size: f64) -> f64 {
    let scaled = font.clone().into_scaled(size as f32);
    text.chars()
        .map(|c| scaled.h_advance(scaled.glyph_id(c)) as f64)
        .sum()
}

fn draw_text(
    canvas: &mut RgbImage,
    font: &FontArc,
    text: &str,
    x: f64,
    y: f64,
    size: f64,
    color: Rgb<u8>,
) {
    draw_text_mut(
        canvas,
        color,
        (x * RENDER_SCALE) as i32,
        (y * RENDER_SCALE) as i32,
        PxScale::from(size as f32 * RENDER_SCALE as f32),
        font,
        text,
    );
}

fn draw_line(canvas: &mut RgbImage, x1: f64, y1: f64, x2: f64, y2: f64, color: Rgb<u8>) {
    draw_line_segment_mut(
        canvas,
        ((x1 * RENDER_SCALE) as f32, (y1 * RENDER_SCALE) as f32),
        ((x2 * RENDER_SCALE) as f32, (y2 * RENDER_SCALE) as f32),
        color,
    );
}

fn font() -> FontArc {
    let mut font = FontVec::try_from_vec(FONT_BYTES.to_vec()).expect("bundled font must load");
    font.set_variation(b"wght", 650.0);
    FontArc::from(font)
}

fn format_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    let hours = seconds / 3600;
    let minutes = seconds % 3600 / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Slide;

    #[test]
    fn exports_all_pdf_modes() {
        let directory = tempfile::tempdir().unwrap();
        let image_path = directory.path().join("slide.jpg");
        let image = RgbImage::from_pixel(640, 360, Rgb([230, 230, 230]));
        image.save(&image_path).unwrap();
        let project = Project {
            video_name: "test.mp4".to_string(),
            slides: vec![Slide {
                number: 1,
                start: 0.0,
                end: 10.0,
                image_path,
                transcript: "一行目\n\n二行目".to_string(),
            }],
            ..Default::default()
        };
        for mode in [
            PdfExportMode::Combined,
            PdfExportMode::SlidesOnly,
            PdfExportMode::TranscriptsOnly,
        ] {
            let path = directory.path().join(format!("{mode:?}.pdf"));
            export(
                &project,
                &path,
                PdfOptions {
                    mode,
                    minimum_font_size: 6.5,
                },
            )
            .unwrap();
            assert!(path.metadata().unwrap().len() > 1000);
        }
    }

    #[test]
    fn reports_layout_overflow() {
        let directory = tempfile::tempdir().unwrap();
        let image_path = directory.path().join("slide.jpg");
        RgbImage::from_pixel(640, 360, Rgb([255, 255, 255]))
            .save(&image_path)
            .unwrap();
        let project = Project {
            slides: vec![Slide {
                number: 1,
                image_path,
                transcript: "あ".repeat(60000),
                ..Default::default()
            }],
            ..Default::default()
        };
        let error = export(
            &project,
            &directory.path().join("overflow.pdf"),
            PdfOptions {
                mode: PdfExportMode::TranscriptsOnly,
                minimum_font_size: 5.0,
            },
        )
        .unwrap_err();
        assert!(error.downcast_ref::<PdfLayoutError>().is_some());
    }
}
