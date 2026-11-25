use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct TranscriptToken {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

impl Default for TranscriptToken {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 0.0,
            text: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct TranscriptSegment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub tokens: Vec<TranscriptToken>,
}

impl Default for TranscriptSegment {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 0.0,
            text: String::new(),
            tokens: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Slide {
    pub number: usize,
    pub start: f64,
    pub end: f64,
    pub image_path: PathBuf,
    pub transcript: String,
}

impl Default for Slide {
    fn default() -> Self {
        Self {
            number: 0,
            start: 0.0,
            end: 0.0,
            image_path: PathBuf::new(),
            transcript: String::new(),
        }
    }
}

impl Slide {
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Project {
    pub version: u32,
    pub video_path: PathBuf,
    pub video_name: String,
    pub model_name: String,
    pub model_path: PathBuf,
    pub created_at: String,
    pub slides: Vec<Slide>,
    pub segments: Vec<TranscriptSegment>,
    #[serde(skip)]
    pub project_path: Option<PathBuf>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            version: 1,
            video_path: PathBuf::new(),
            video_name: String::new(),
            model_name: String::new(),
            model_path: PathBuf::new(),
            created_at: String::new(),
            slides: Vec::new(),
            segments: Vec::new(),
            project_path: None,
        }
    }
}

impl Project {
    pub fn renumber(&mut self) {
        for (index, slide) in self.slides.iter_mut().enumerate() {
            slide.number = index + 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PdfExportMode {
    #[default]
    Combined,
    SlidesOnly,
    TranscriptsOnly,
}

#[derive(Debug, Clone, Copy)]
pub struct PdfOptions {
    pub mode: PdfExportMode,
    pub minimum_font_size: f64,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            mode: PdfExportMode::Combined,
            minimum_font_size: 6.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelChoice {
    pub display_name: &'static str,
    pub file_name: &'static str,
    pub download_name: &'static str,
    pub subdirectory: &'static str,
    pub description: &'static str,
}

pub const MODEL_CHOICES: &[ModelChoice] = &[
    ModelChoice {
        display_name: "large-v3-turbo / Q8 (推奨)",
        file_name: "ggml-large-v3-turbo-q8_0.bin",
        download_name: "ggml-large-v3-turbo.bin",
        subdirectory: "q8_0",
        description: "速度と精度のバランスが良い日本語講義向けモデル",
    },
    ModelChoice {
        display_name: "large-v3 / Q8 (高精度)",
        file_name: "ggml-large-v3-q8_0.bin",
        download_name: "ggml-large-v3.bin",
        subdirectory: "q8_0",
        description: "高精度だが処理時間とVRAM使用量が大きい",
    },
    ModelChoice {
        display_name: "medium / Q5",
        file_name: "ggml-medium-q5_0.bin",
        download_name: "ggml-medium.bin",
        subdirectory: "q5_0",
        description: "中量級。低性能GPU向け",
    },
    ModelChoice {
        display_name: "small / Q5",
        file_name: "ggml-small-q5_0.bin",
        download_name: "ggml-small.bin",
        subdirectory: "q5_0",
        description: "軽量。動作確認や短い動画向け",
    },
    ModelChoice {
        display_name: "tiny / Q5 (テスト)",
        file_name: "ggml-tiny-q5_0.bin",
        download_name: "ggml-tiny.bin",
        subdirectory: "q5_0",
        description: "最小モデル。精度は低い",
    },
];
