use crate::model::PdfExportMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct AppSettings {
    pub dark_mode: bool,
    pub use_gpu: bool,
    pub output_directory: PathBuf,
    pub model_directory: PathBuf,
    pub scene_threshold: f64,
    pub minimum_slide_seconds: f64,
    pub initial_prompt: String,
    pub model_index: usize,
    pub pdf_minimum_font_size: f64,
    pub pdf_export_mode: PdfExportMode,
}

impl Default for AppSettings {
    fn default() -> Self {
        let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            dark_mode: true,
            use_gpu: true,
            output_directory: dirs::video_dir()
                .unwrap_or_else(|| local.clone())
                .join("SlideVideoDivider"),
            model_directory: local.join("SlideVideoDivider").join("models"),
            scene_threshold: 0.12,
            minimum_slide_seconds: 1.5,
            initial_prompt: "工学、数学、物理学、制御工学、専門用語。".to_string(),
            model_index: 0,
            pdf_minimum_font_size: 6.5,
            pdf_export_mode: PdfExportMode::Combined,
        }
    }
}

impl AppSettings {
    pub fn directory() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("SlideVideoDivider")
    }

    pub fn path() -> PathBuf {
        Self::directory().join("settings.json")
    }

    fn legacy_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("SlideTranscriber")
            .join("settings.json")
    }

    pub fn load() -> Self {
        let use_legacy = !Self::path().exists() && Self::legacy_path().exists();
        let path = if Self::path().exists() {
            Self::path()
        } else {
            Self::legacy_path()
        };
        let mut settings = fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Self>(&text).ok())
            .unwrap_or_default();
        settings.model_index = settings
            .model_index
            .min(crate::model::MODEL_CHOICES.len().saturating_sub(1));
        settings.pdf_minimum_font_size = settings.pdf_minimum_font_size.clamp(5.0, 10.0);
        if use_legacy {
            let _ = settings.save();
        }
        settings
    }

    pub fn save(&self) -> anyhow::Result<()> {
        fs::create_dir_all(Self::directory())?;
        fs::write(Self::path(), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
