pub mod ffmpeg;
pub mod transcribe;

use crate::model::{ModelChoice, Project};
use crate::project;
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

#[derive(Debug)]
pub enum PipelineEvent {
    Status(String),
    Overall(f64),
    Finished(Project),
    Failed(String),
    Cancelled,
    ModelReady,
    PdfFinished(PathBuf),
    PdfFailed(String),
    FrameExtracted {
        slide_index: usize,
        time: f64,
        image_path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct PipelineContext {
    pub sender: Sender<PipelineEvent>,
    pub cancel: Arc<AtomicBool>,
}

impl PipelineContext {
    pub fn new(sender: Sender<PipelineEvent>, cancel: Arc<AtomicBool>) -> Self {
        Self { sender, cancel }
    }

    pub fn status(&self, message: impl Into<String>) {
        let _ = self.sender.send(PipelineEvent::Status(message.into()));
    }

    pub fn overall(&self, value: f64) {
        let _ = self
            .sender
            .send(PipelineEvent::Overall(value.clamp(0.0, 100.0)));
    }

    pub fn check_cancel(&self) -> Result<()> {
        ffmpeg::check_cancel(&self.cancel)
    }
}

pub struct PipelineRequest {
    pub video_path: PathBuf,
    pub output_root: PathBuf,
    pub model: ModelChoice,
    pub model_path: PathBuf,
    pub use_gpu: bool,
    pub initial_prompt: String,
    pub scene_threshold: f64,
    pub minimum_slide_seconds: f64,
}

pub fn run_pipeline(mut request: PipelineRequest, ctx: &PipelineContext) -> Result<Project> {
    request.video_path = std::path::absolute(&request.video_path)?;
    request.output_root = std::path::absolute(&request.output_root)?;
    request.model_path = std::path::absolute(&request.model_path)?;
    let video_name = request
        .video_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let folder_name = safe_folder_name(&video_name);
    let base_directory = request.output_root.join(&folder_name);
    let project_file_name = format!("{folder_name}_project.json");
    let output_directory = if base_directory.join(&project_file_name).exists() {
        request.output_root.join(format!(
            "{folder_name}_{}",
            chrono::Local::now().timestamp_millis()
        ))
    } else {
        base_directory
    };
    let slides_directory = output_directory.join("slides");
    let wav_path = output_directory.join("_audio_16k.wav");
    std::fs::create_dir_all(&slides_directory)?;
    ctx.overall(2.0);
    ctx.status("動画情報を確認しています...");
    let duration = ffmpeg::probe_duration(&request.video_path, &ctx.cancel)?;
    let boundaries = ffmpeg::detect_scene_times(
        &request.video_path,
        duration,
        request.scene_threshold,
        request.minimum_slide_seconds,
        &ctx.cancel,
    )?;
    ctx.status("スライド画像を抽出しています...");
    let mut slides = ffmpeg::extract_slides(
        &request.video_path,
        &boundaries,
        duration,
        &slides_directory,
        &ctx.cancel,
        &|percent| {
            ctx.overall(25.0 + percent * 0.15);
        },
    )?;
    ctx.overall(41.0);
    ctx.status("音声を16kHzモノラルへ変換しています...");
    ffmpeg::extract_audio(&request.video_path, &wav_path, &ctx.cancel)?;
    ctx.overall(45.0);
    transcribe::ensure_model(&request.model, &request.model_path, ctx)?;
    ctx.overall(50.0);
    let ranges: Vec<(f64, f64)> = slides
        .iter()
        .map(|slide| (slide.start, slide.end))
        .collect();
    let transcripts = transcribe::transcribe_slides(
        &wav_path,
        &request.model_path,
        request.use_gpu,
        &request.initial_prompt,
        &ranges,
        ctx,
    )?;
    ctx.overall(96.0);
    let mut segments = Vec::new();
    let mut slide_texts = vec![String::new(); slides.len()];
    for transcript in transcripts {
        let text = transcript
            .segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(slot) = slide_texts.get_mut(transcript.index) {
            *slot = project::normalize_transcript(&text);
        }
        segments.extend(transcript.segments);
    }
    if slide_texts.iter().all(|text| text.is_empty()) {
        return Err(anyhow!(
            "文字起こし結果が空でした。音声の有無・言語設定を確認してください。"
        ));
    }
    segments.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (slide, text) in slides.iter_mut().zip(slide_texts) {
        slide.transcript = text;
    }
    ctx.status("プロジェクトを保存しています...");
    let mut project = Project {
        version: 1,
        video_path: request.video_path,
        video_name,
        model_name: request.model.display_name.to_string(),
        model_path: request.model_path,
        created_at: chrono::Local::now().to_rfc3339(),
        slides,
        segments,
        project_path: None,
    };
    let project_path = project::default_project_path(&output_directory, &project.video_path);
    project::save(&project_path, &mut project)?;
    let _ = std::fs::remove_file(&wav_path);
    ctx.overall(100.0);
    ctx.status(format!(
        "完了: {}枚のスライド / {}発話",
        project.slides.len(),
        project.segments.len()
    ));
    Ok(project)
}

fn safe_folder_name(name: &str) -> String {
    let stem = std::path::Path::new(name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let safe: String = stem
        .chars()
        .map(|c| {
            if c.is_control() || "\\/:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    if safe.is_empty() {
        "output".to_string()
    } else {
        safe
    }
}
