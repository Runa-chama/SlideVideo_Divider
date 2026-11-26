use crate::model::Project;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn normalize_transcript(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn default_project_path(output_directory: &Path, video_path: &Path) -> PathBuf {
    let name = video_path.file_stem().unwrap_or_default().to_string_lossy();
    output_directory.join(format!("{name}_project.json"))
}

pub fn save(path: &Path, project: &mut Project) -> Result<()> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory)?;
    let full_path = std::path::absolute(directory)?;
    project.renumber();
    let mut document = project.clone();
    document.project_path = None;
    for slide in &mut document.slides {
        if slide.image_path.is_absolute() {
            slide.image_path = pathdiff(&slide.image_path, &full_path);
        }
    }
    fs::write(path, serde_json::to_string_pretty(&document)?)?;
    project.project_path = Some(path.to_path_buf());
    Ok(())
}

pub fn load(path: &Path) -> Result<Project> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("プロジェクトを読み込めません: {}", path.display()))?;
    let mut project: Project =
        serde_json::from_str(&text).context("プロジェクトJSONの解析に失敗しました")?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    for slide in &mut project.slides {
        if slide.image_path.is_relative() {
            slide.image_path = directory.join(&slide.image_path);
        }
        slide.transcript = normalize_transcript(&slide.transcript);
    }
    project.renumber();
    project.project_path = Some(path.to_path_buf());
    Ok(project)
}

fn pathdiff(path: &Path, base: &Path) -> PathBuf {
    if path.components().next() != base.components().next() {
        return path.to_path_buf();
    }
    let path_components: Vec<_> = path.components().collect();
    let base_components: Vec<_> = base.components().collect();
    let mut common = 0;
    while common < path_components.len()
        && common < base_components.len()
        && path_components[common] == base_components[common]
    {
        common += 1;
    }
    let mut result = PathBuf::new();
    for _ in common..base_components.len() {
        result.push("..");
    }
    for component in &path_components[common..] {
        result.push(component.as_os_str());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Slide;

    #[test]
    fn removes_blank_lines() {
        assert_eq!(
            normalize_transcript("一行目\n\n二行目\n \n三行目"),
            "一行目\n二行目\n三行目"
        );
    }

    #[test]
    fn project_round_trip_keeps_relative_images() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("project.json");
        let image = directory.path().join("slides").join("slide_001.jpg");
        fs::create_dir_all(image.parent().unwrap()).unwrap();
        fs::write(&image, b"test").unwrap();
        let mut project = Project {
            video_name: "video.mp4".to_string(),
            slides: vec![Slide {
                number: 1,
                image_path: image,
                transcript: "A\n\nB".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        save(&path, &mut project).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.slides[0].transcript, "A\nB");
        assert!(loaded.slides[0].image_path.is_absolute());
    }
}
