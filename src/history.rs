use crate::model::Project;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum EditCommand {
    MoveTranscript {
        source: usize,
        target: usize,
        source_text: String,
        target_text: String,
    },
    ReplaceImage {
        index: usize,
        old_path: PathBuf,
        new_path: PathBuf,
    },
    ReassignTranscripts {
        old_texts: Vec<String>,
    },
}

impl EditCommand {
    pub fn description(&self) -> &'static str {
        match self {
            Self::MoveTranscript { .. } => "文字の移動",
            Self::ReplaceImage { .. } => "画像差し替え",
            Self::ReassignTranscripts { .. } => "再関連付け",
        }
    }

    pub fn apply(&self, project: &mut Project) {
        match self {
            Self::MoveTranscript {
                source,
                target,
                source_text,
                target_text,
            } => {
                let text = source_text.clone();
                project.slides[*target].transcript = if target_text.is_empty() {
                    text
                } else {
                    format!("{target_text}\n{text}")
                };
                project.slides[*source].transcript.clear();
            }
            Self::ReplaceImage {
                index, new_path, ..
            } => project.slides[*index].image_path = new_path.clone(),
            Self::ReassignTranscripts { .. } => crate::project::assign_transcripts(project),
        }
    }

    pub fn revert(&self, project: &mut Project) {
        match self {
            Self::MoveTranscript {
                source,
                target,
                source_text,
                target_text,
            } => {
                project.slides[*source].transcript = source_text.clone();
                project.slides[*target].transcript = target_text.clone();
            }
            Self::ReplaceImage {
                index, old_path, ..
            } => project.slides[*index].image_path = old_path.clone(),
            Self::ReassignTranscripts { old_texts } => {
                for (slide, old_text) in project.slides.iter_mut().zip(old_texts) {
                    slide.transcript = old_text.clone();
                }
            }
        }
    }
}

pub struct EditHistory {
    undo: Vec<EditCommand>,
    redo: Vec<EditCommand>,
    capacity: usize,
}

impl EditHistory {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            capacity: 100,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_description(&self) -> Option<&'static str> {
        self.undo.last().map(EditCommand::description)
    }

    pub fn redo_description(&self) -> Option<&'static str> {
        self.redo.last().map(EditCommand::description)
    }

    pub fn push(&mut self, command: EditCommand) {
        self.undo.push(command);
        if self.undo.len() > self.capacity {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, project: &mut Project) -> bool {
        let Some(command) = self.undo.pop() else {
            return false;
        };
        command.revert(project);
        self.redo.push(command);
        true
    }

    pub fn redo(&mut self, project: &mut Project) -> bool {
        let Some(command) = self.redo.pop() else {
            return false;
        };
        command.apply(project);
        self.undo.push(command);
        true
    }

    pub fn clear_redo(&mut self) {
        self.redo.clear();
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, Slide};

    #[test]
    fn undo_and_redo_replace_image() {
        let mut project = Project {
            slides: vec![Slide {
                image_path: std::path::PathBuf::from("old.jpg"),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut history = EditHistory::new();
        let command = EditCommand::ReplaceImage {
            index: 0,
            old_path: std::path::PathBuf::from("old.jpg"),
            new_path: std::path::PathBuf::from("new.jpg"),
        };
        command.apply(&mut project);
        history.push(command);
        assert_eq!(
            project.slides[0].image_path,
            std::path::PathBuf::from("new.jpg")
        );
        assert!(history.undo(&mut project));
        assert_eq!(
            project.slides[0].image_path,
            std::path::PathBuf::from("old.jpg")
        );
        assert!(history.redo(&mut project));
        assert_eq!(
            project.slides[0].image_path,
            std::path::PathBuf::from("new.jpg")
        );
    }

    #[test]
    fn undo_and_redo_reassign_transcripts() {
        let mut project = Project {
            slides: vec![
                Slide {
                    start: 0.0,
                    end: 5.0,
                    transcript: "old1".to_string(),
                    ..Default::default()
                },
                Slide {
                    start: 5.0,
                    end: 10.0,
                    transcript: "old2".to_string(),
                    ..Default::default()
                },
            ],
            segments: vec![crate::model::TranscriptSegment {
                start: 0.0,
                end: 10.0,
                text: "0123456789".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut history = EditHistory::new();
        let command = EditCommand::ReassignTranscripts {
            old_texts: vec!["old1".to_string(), "old2".to_string()],
        };
        command.apply(&mut project);
        history.push(command);
        assert_eq!(project.slides[0].transcript, "01234");
        assert_eq!(project.slides[1].transcript, "56789");
        assert!(history.undo(&mut project));
        assert_eq!(project.slides[0].transcript, "old1");
        assert_eq!(project.slides[1].transcript, "old2");
        assert!(history.redo(&mut project));
        assert_eq!(project.slides[0].transcript, "01234");
        assert_eq!(project.slides[1].transcript, "56789");
    }

    #[test]
    fn undo_and_redo_move_transcript() {
        let mut project = Project {
            slides: vec![
                Slide {
                    transcript: "A".to_string(),
                    ..Default::default()
                },
                Slide {
                    transcript: "B".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut history = EditHistory::new();
        let command = EditCommand::MoveTranscript {
            source: 0,
            target: 1,
            source_text: "A".to_string(),
            target_text: "B".to_string(),
        };
        command.apply(&mut project);
        history.push(command);
        assert_eq!(project.slides[1].transcript, "B\nA");
        assert!(history.undo(&mut project));
        assert_eq!(project.slides[0].transcript, "A");
        assert_eq!(project.slides[1].transcript, "B");
        assert!(history.redo(&mut project));
        assert_eq!(project.slides[1].transcript, "B\nA");
    }
}
