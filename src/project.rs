use crate::model::{Project, Slide, TranscriptSegment};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn normalize_transcript(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn assign_transcripts(project: &mut Project) {
    let mut parts = vec![Vec::<String>::new(); project.slides.len()];
    let mut segments = project.segments.clone();
    segments.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for segment in segments {
        let text = normalize_transcript(&segment.text);
        if text.is_empty() {
            continue;
        }

        if assign_by_token_timestamps(&project.slides, &segment, &text, &mut parts) {
            continue;
        }
        assign_proportionally(&project.slides, &segment, &text, &mut parts);
    }

    for (index, slide) in project.slides.iter_mut().enumerate() {
        slide.transcript = normalize_transcript(&parts[index].join("\n"));
    }
}

fn assign_by_token_timestamps(
    slides: &[Slide],
    segment: &TranscriptSegment,
    text: &str,
    parts: &mut [Vec<String>],
) -> bool {
    let mut tokens: Vec<_> = segment
        .tokens
        .iter()
        .filter(|token| !token.text.is_empty() && token.end > token.start && token.start >= 0.0)
        .collect();
    tokens.sort_by(|left, right| {
        left.start
            .partial_cmp(&right.start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if tokens.len() < 2 {
        return false;
    }

    let token_chars: usize = tokens.iter().map(|token| token.text.chars().count()).sum();
    if token_chars * 2 < text.chars().count() {
        return false;
    }
    if tokens.last().map_or(0.0, |token| token.end) - tokens[0].start < 0.05 {
        return false;
    }

    let mut grouped = BTreeMap::<usize, String>::new();
    let mut previous_index = None;
    for token in tokens {
        let midpoint = token.start + (token.end - token.start) / 2.0;
        let Some(mut index) = slide_index_at(slides, midpoint) else {
            continue;
        };
        if is_attached_punctuation(&token.text)
            && let Some(previous) = previous_index
        {
            index = previous;
        }
        grouped.entry(index).or_default().push_str(&token.text);
        previous_index = Some(index);
    }

    if grouped.is_empty() {
        return false;
    }

    for (index, token_text) in grouped {
        let piece = normalize_transcript(&token_text);
        if !piece.is_empty() {
            parts[index].push(piece);
        }
    }
    true
}

fn assign_proportionally(
    slides: &[Slide],
    segment: &TranscriptSegment,
    text: &str,
    parts: &mut [Vec<String>],
) {
    let overlaps: Vec<(usize, f64)> = slides
        .iter()
        .enumerate()
        .filter_map(|(index, slide)| {
            let overlap = segment.end.min(slide.end) - segment.start.max(slide.start);
            (overlap > 0.0).then_some((index, overlap))
        })
        .collect();

    if overlaps.is_empty() {
        if let Some(index) = slide_index_at(slides, segment.start) {
            parts[index].push(text.to_string());
        }
        return;
    }

    let total_overlap: f64 = overlaps.iter().map(|(_, overlap)| overlap).sum();
    let overlap_count = overlaps.len();
    let characters: Vec<char> = text.chars().collect();
    let mut consumed = 0usize;
    for (position, (index, overlap)) in overlaps.into_iter().enumerate() {
        let remaining = characters.len() - consumed;
        let count = if position + 1 == overlap_count {
            remaining
        } else {
            ((characters.len() as f64 * overlap / total_overlap).round() as usize).min(remaining)
        };
        let piece: String = characters[consumed..consumed + count].iter().collect();
        let piece = normalize_transcript(&piece);
        if !piece.is_empty() {
            parts[index].push(piece);
        }
        consumed += count;
    }
}

fn is_attached_punctuation(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|character| {
            character.is_whitespace()
                || matches!(
                    character,
                    '。' | '！'
                        | '？'
                        | '、'
                        | '，'
                        | '．'
                        | '!'
                        | '?'
                        | '.'
                        | ','
                        | '」'
                        | '』'
                        | '）'
                        | ')'
                        | '】'
                        | ']'
                        | '”'
                        | '’'
                )
        })
}

fn slide_index_at(slides: &[Slide], time: f64) -> Option<usize> {
    if let Some(index) = slides
        .iter()
        .position(|slide| time >= slide.start && time < slide.end)
    {
        return Some(index);
    }

    slides
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            let left_distance = if time < left.start {
                left.start - time
            } else {
                time - left.end
            };
            let right_distance = if time < right.start {
                right.start - time
            } else {
                time - right.end
            };
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
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

    #[test]
    fn removes_blank_lines() {
        assert_eq!(
            normalize_transcript("一行目\n\n二行目\n \n三行目"),
            "一行目\n二行目\n三行目"
        );
    }

    #[test]
    fn token_timestamps_split_across_slides() {
        let mut project = Project {
            slides: vec![
                Slide {
                    start: 0.0,
                    end: 5.0,
                    ..Default::default()
                },
                Slide {
                    start: 5.0,
                    end: 10.0,
                    ..Default::default()
                },
            ],
            segments: vec![TranscriptSegment {
                start: 0.0,
                end: 10.0,
                text: "ABCD".to_string(),
                tokens: vec![
                    crate::model::TranscriptToken {
                        start: 0.0,
                        end: 2.0,
                        text: "A".to_string(),
                    },
                    crate::model::TranscriptToken {
                        start: 2.0,
                        end: 4.0,
                        text: "B".to_string(),
                    },
                    crate::model::TranscriptToken {
                        start: 6.0,
                        end: 8.0,
                        text: "C".to_string(),
                    },
                    crate::model::TranscriptToken {
                        start: 8.0,
                        end: 10.0,
                        text: "D".to_string(),
                    },
                ],
            }],
            ..Default::default()
        };
        assign_transcripts(&mut project);
        assert_eq!(project.slides[0].transcript, "AB");
        assert_eq!(project.slides[1].transcript, "CD");
    }

    #[test]
    fn punctuation_stays_with_preceding_slide() {
        let mut project = Project {
            slides: vec![
                Slide {
                    start: 0.0,
                    end: 5.0,
                    ..Default::default()
                },
                Slide {
                    start: 5.0,
                    end: 10.0,
                    ..Default::default()
                },
            ],
            segments: vec![TranscriptSegment {
                start: 0.0,
                end: 6.0,
                text: "ABC。".to_string(),
                tokens: vec![
                    crate::model::TranscriptToken {
                        start: 0.0,
                        end: 2.0,
                        text: "A".to_string(),
                    },
                    crate::model::TranscriptToken {
                        start: 3.0,
                        end: 4.0,
                        text: "BC".to_string(),
                    },
                    crate::model::TranscriptToken {
                        start: 5.2,
                        end: 5.8,
                        text: "。".to_string(),
                    },
                ],
            }],
            ..Default::default()
        };
        assign_transcripts(&mut project);
        assert_eq!(project.slides[0].transcript, "ABC。");
        assert_eq!(project.slides[1].transcript, "");
    }

    #[test]
    fn missing_token_timestamps_fall_back_to_proportional_split() {
        let mut project = Project {
            slides: vec![
                Slide {
                    start: 0.0,
                    end: 4.0,
                    ..Default::default()
                },
                Slide {
                    start: 4.0,
                    end: 10.0,
                    ..Default::default()
                },
            ],
            segments: vec![TranscriptSegment {
                start: 0.0,
                end: 10.0,
                text: "0123456789".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assign_transcripts(&mut project);
        assert_eq!(project.slides[0].transcript, "0123");
        assert_eq!(project.slides[1].transcript, "456789");
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
