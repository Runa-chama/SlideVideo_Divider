use crate::history::{EditCommand, EditHistory};
use crate::model::{MODEL_CHOICES, PdfExportMode, PdfOptions, Project};
use crate::pdf;
use crate::pipeline::{self, PipelineContext, PipelineEvent, PipelineRequest};
use crate::project;
use crate::settings::AppSettings;
use eframe::{App, CreationContext, Frame};
use egui::{
    Align, Button, Color32, ComboBox, Context, CornerRadius, DragValue, Frame as EguiFrame, Image,
    Key, Layout, Panel, RichText, ScrollArea, Sense, TextEdit, TextureHandle, Visuals, vec2,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/Mplus1.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Convert,
    Notes,
    Settings,
}

struct PdfDialogState {
    mode: PdfExportMode,
    minimum_font_size: f64,
}

struct MessageState {
    title: String,
    text: String,
}

pub struct SlideVideoDividerApp {
    settings: AppSettings,
    tab: Tab,
    project: Option<Project>,
    selected_slide: Option<usize>,
    transcript_buffer: String,
    transcript_has_focus: bool,
    history: EditHistory,
    dirty: bool,
    search: String,

    video_path: String,
    output_directory: String,
    model_index: usize,
    model_path: String,
    use_gpu: bool,
    scene_threshold: f64,
    minimum_slide_seconds: f64,
    initial_prompt: String,

    status: String,
    progress: Option<f32>,
    busy: bool,
    cancelable: bool,
    cancel: Arc<AtomicBool>,
    receiver: Option<Receiver<PipelineEvent>>,
    pdf_dialog: Option<PdfDialogState>,
    message: Option<MessageState>,
    textures: HashMap<PathBuf, TextureHandle>,
}

impl SlideVideoDividerApp {
    pub fn new(cc: &CreationContext<'_>, initial_project: Option<PathBuf>) -> Self {
        install_fonts(&cc.egui_ctx);
        let mut settings = AppSettings::load();
        settings.pdf_minimum_font_size = settings.pdf_minimum_font_size.clamp(5.0, 10.0);
        let model_index = settings
            .model_index
            .min(MODEL_CHOICES.len().saturating_sub(1));
        let use_gpu = settings.use_gpu && cfg!(feature = "cuda");
        let model_path = settings
            .model_directory
            .join(MODEL_CHOICES[model_index].file_name);
        let mut project = initial_project
            .as_ref()
            .and_then(|path| project::load(path).ok());
        let selected_slide = project
            .as_ref()
            .and_then(|project| (!project.slides.is_empty()).then_some(0));
        let transcript_buffer = project
            .as_ref()
            .and_then(|project| project.slides.first())
            .map(|slide| slide.transcript.clone())
            .unwrap_or_default();
        let tab = if project.is_some() {
            Tab::Notes
        } else {
            Tab::Convert
        };
        if let Some(project) = project.as_mut() {
            project.project_path = initial_project.clone();
        }
        Self {
            video_path: project
                .as_ref()
                .map(|p| p.video_path.to_string_lossy().to_string())
                .unwrap_or_default(),
            output_directory: settings.output_directory.to_string_lossy().to_string(),
            tab,
            project,
            selected_slide,
            transcript_buffer,
            transcript_has_focus: false,
            history: EditHistory::new(),
            dirty: false,
            search: String::new(),
            model_index,
            model_path: model_path.to_string_lossy().to_string(),
            use_gpu,
            scene_threshold: settings.scene_threshold,
            minimum_slide_seconds: settings.minimum_slide_seconds,
            initial_prompt: settings.initial_prompt.clone(),
            status: "準備できました".to_string(),
            progress: None,
            busy: false,
            cancelable: false,
            cancel: Arc::new(AtomicBool::new(false)),
            receiver: None,
            pdf_dialog: None,
            message: None,
            textures: HashMap::new(),
            settings,
        }
    }

    fn apply_theme(&self, ctx: &Context) {
        let mut visuals = if self.settings.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        if self.settings.dark_mode {
            visuals.panel_fill = Color32::from_rgb(20, 22, 27);
            visuals.window_fill = Color32::from_rgb(30, 32, 39);
            visuals.extreme_bg_color = Color32::from_rgb(36, 39, 47);
            visuals.faint_bg_color = Color32::from_rgb(40, 43, 52);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 32, 39);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(47, 51, 62);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 64, 78);
            visuals.widgets.active.bg_fill = Color32::from_rgb(35, 67, 92);
            visuals.selection.bg_fill = Color32::from_rgb(35, 67, 92);
            visuals.window_corner_radius = CornerRadius::same(8);
            visuals.menu_corner_radius = CornerRadius::same(6);
        } else {
            visuals.panel_fill = Color32::from_rgb(246, 246, 246);
            visuals.window_fill = Color32::WHITE;
            visuals.extreme_bg_color = Color32::from_rgb(255, 255, 255);
            visuals.faint_bg_color = Color32::from_rgb(237, 239, 242);
            visuals.widgets.noninteractive.bg_fill = Color32::WHITE;
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(233, 236, 241);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(218, 224, 233);
            visuals.widgets.active.bg_fill = Color32::from_rgb(218, 235, 249);
            visuals.selection.bg_fill = Color32::from_rgb(218, 235, 249);
            visuals.window_corner_radius = CornerRadius::same(8);
            visuals.menu_corner_radius = CornerRadius::same(6);
        }
        ctx.set_visuals(visuals);
        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = vec2(7.0, 6.0);
            style.spacing.button_padding = vec2(9.0, 4.0);
        });
    }

    fn render(&mut self, ui: &mut egui::Ui, frame: &mut Frame) {
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);
        apply_title_bar(frame, self.settings.dark_mode);
        self.handle_shortcuts(&ctx);
        self.drain_events(&ctx);
        self.handle_dropped_files(&ctx);
        self.sync_settings();

        Panel::top("header").exact_size(48.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("SlideVideoDivider").strong().size(15.0));
                ui.add_space(24.0);
                ui.selectable_value(&mut self.tab, Tab::Convert, "変換");
                ui.selectable_value(&mut self.tab, Tab::Notes, "スライドノート");
                ui.selectable_value(&mut self.tab, Tab::Settings, "設定");
            });
        });

        Panel::bottom("status").exact_size(28.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                if self.busy {
                    ui.add(egui::Spinner::new());
                }
                ui.label(&self.status);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(project) = &self.project {
                        ui.label(format!("スライド: {}", project.slides.len()));
                    }
                    if let Some(progress) = self.progress {
                        ui.add(egui::ProgressBar::new(progress / 100.0).desired_width(160.0));
                    }
                });
            });
        });

        match self.tab {
            Tab::Convert => self.show_convert(ui),
            Tab::Notes => self.show_notes(ui),
            Tab::Settings => self.show_settings(ui),
        }

        self.show_pdf_dialog(&ctx);
        self.show_message(&ctx);
    }

    fn show_convert(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.heading("音声付きスライド動画をノートPDFへ変換");
                    ui.separator();
                    egui::Grid::new("convert-grid")
                        .num_columns(3)
                        .spacing([8.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("動画ファイル");
                            ui.add(TextEdit::singleline(&mut self.video_path).desired_width(620.0));
                            if ui.button("参照...").clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("動画", &["mp4", "mov", "mkv", "webm", "avi"])
                                    .pick_file()
                            {
                                self.video_path = path.to_string_lossy().to_string();
                            }
                            ui.end_row();

                            ui.label("出力フォルダ");
                            ui.add(
                                TextEdit::singleline(&mut self.output_directory)
                                    .desired_width(620.0),
                            );
                            if ui.button("参照...").clicked()
                                && let Some(path) = rfd::FileDialog::new().pick_folder()
                            {
                                self.output_directory = path.to_string_lossy().to_string();
                            }
                            ui.end_row();
                        });
                    ui.label(RichText::new("MP4 / MOV / MKV / WebM に対応。").weak());

                    ui.separator();
                    ui.heading("文字起こし・スライド検出");
                    egui::Grid::new("options-grid")
                        .num_columns(2)
                        .spacing([8.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("認識モデル");
                            ComboBox::from_id_salt("model-choice")
                                .width(520.0)
                                .selected_text(MODEL_CHOICES[self.model_index].display_name)
                                .show_ui(ui, |ui| {
                                    for (index, choice) in MODEL_CHOICES.iter().enumerate() {
                                        if ui
                                            .selectable_value(
                                                &mut self.model_index,
                                                index,
                                                choice.display_name,
                                            )
                                            .clicked()
                                        {
                                            self.model_path = self
                                                .settings
                                                .model_directory
                                                .join(choice.file_name)
                                                .to_string_lossy()
                                                .to_string();
                                        }
                                    }
                                });
                            ui.end_row();

                            ui.label("モデルファイル");
                            ui.horizontal(|ui| {
                                ui.add(
                                    TextEdit::singleline(&mut self.model_path).desired_width(430.0),
                                );
                                if ui.button("参照...").clicked()
                                    && let Some(path) = rfd::FileDialog::new()
                                        .add_filter("Whisperモデル", &["bin"])
                                        .pick_file()
                                {
                                    self.model_path = path.to_string_lossy().to_string();
                                }
                                if ui
                                    .add_enabled(!self.busy, Button::new("ダウンロード"))
                                    .clicked()
                                {
                                    self.start_model_download();
                                }
                            });
                            ui.end_row();
                            ui.label("");
                            ui.label(
                                RichText::new(MODEL_CHOICES[self.model_index].description).weak(),
                            );
                            ui.end_row();

                            ui.label("実行デバイス");
                            ui.checkbox(&mut self.use_gpu, "NVIDIA GPU（CUDA）を使用");
                            ui.end_row();

                            ui.label("切替検出しきい値");
                            ui.add(
                                DragValue::new(&mut self.scene_threshold)
                                    .speed(0.01)
                                    .range(0.05..=0.50)
                                    .fixed_decimals(2),
                            );
                            ui.end_row();

                            ui.label("最短表示時間");
                            ui.add(
                                DragValue::new(&mut self.minimum_slide_seconds)
                                    .speed(0.1)
                                    .range(0.5..=10.0)
                                    .suffix(" 秒"),
                            );
                            ui.end_row();

                            ui.label("専門用語ヒント");
                            ui.add(
                                TextEdit::multiline(&mut self.initial_prompt)
                                    .desired_rows(3)
                                    .desired_width(620.0),
                            );
                            ui.end_row();
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.busy,
                                Button::new(RichText::new("変換開始").strong())
                                    .fill(Color32::from_rgb(76, 194, 255)),
                            )
                            .clicked()
                        {
                            self.start_pipeline();
                        }
                        if ui
                            .add_enabled(self.busy && self.cancelable, Button::new("キャンセル"))
                            .clicked()
                        {
                            self.cancel.store(true, Ordering::Relaxed);
                            self.status = "キャンセルしています...".to_string();
                        }
                    });
                    if let Some(progress) = self.progress {
                        ui.add(egui::ProgressBar::new(progress / 100.0).desired_width(700.0));
                    }
                });
        });
    }

    fn show_notes(&mut self, ui: &mut egui::Ui) {
        let slides = self
            .project
            .as_ref()
            .map(|project| project.slides.clone())
            .unwrap_or_default();
        Panel::left("slide-list")
            .default_size(220.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("検索");
                    ui.text_edit_singleline(&mut self.search);
                });
                ScrollArea::vertical().show(ui, |ui| {
                    for (index, slide) in slides.iter().enumerate() {
                        let query = self.search.trim().to_lowercase();
                        if !query.is_empty()
                            && !slide.number.to_string().contains(&query)
                            && !slide.transcript.to_lowercase().contains(&query)
                            && !format_time(slide.start).contains(&query)
                        {
                            continue;
                        }
                        let selected = self.selected_slide == Some(index);
                        let clicked =
                            if let Some(texture) = self.texture_for(ui.ctx(), &slide.image_path) {
                                EguiFrame::NONE
                                    .fill(if selected {
                                        if self.settings.dark_mode {
                                            Color32::from_rgb(35, 67, 92)
                                        } else {
                                            Color32::from_rgb(218, 235, 249)
                                        }
                                    } else {
                                        Color32::TRANSPARENT
                                    })
                                    .corner_radius(CornerRadius::same(6))
                                    .show(ui, |ui| {
                                        ui.add(
                                            Image::new(&texture)
                                                .fit_to_exact_size(vec2(180.0, 101.0))
                                                .sense(Sense::click()),
                                        )
                                    })
                                    .inner
                                    .clicked()
                            } else {
                                ui.add(Button::new("").selected(selected)).clicked()
                            };
                        if clicked {
                            self.select_slide(index);
                        }
                        ui.label(format!("{:02}  {}", slide.number, format_time(slide.start)));
                        ui.add_space(4.0);
                    }
                });
            });

        Panel::right("transcript-panel")
            .default_size(390.0)
            .resizable(true)
            .show(ui, |ui| {
                if let Some(index) = self.selected_slide {
                    if let Some(project) = self.project.as_mut()
                        && let Some(slide) = project.slides.get_mut(index)
                    {
                        ui.horizontal(|ui| {
                            ui.label("開始");
                            let start_response = ui.add(
                                DragValue::new(&mut slide.start)
                                    .speed(0.1)
                                    .range(0.0..=slide.end),
                            );
                            ui.label("終了");
                            let end_response = ui.add(
                                DragValue::new(&mut slide.end)
                                    .speed(0.1)
                                    .range(slide.start..=f64::MAX),
                            );
                            if start_response.changed() || end_response.changed() {
                                self.history.clear_redo();
                                self.dirty = true;
                            }
                        });
                        ui.label(format!(
                            "文字起こし　{}文字",
                            self.transcript_buffer.chars().count()
                        ));
                        let response = ui.add(
                            TextEdit::multiline(&mut self.transcript_buffer)
                                .desired_rows(22)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Body),
                        );
                        self.transcript_has_focus = response.has_focus();
                        if response.changed() {
                            slide.transcript = self.transcript_buffer.clone();
                            self.history.clear_redo();
                            self.dirty = true;
                        }
                    }
                } else {
                    self.transcript_has_focus = false;
                    ui.label("スライドを選択してください");
                }
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.busy && self.project.is_some(), Button::new("開く"))
                    .clicked()
                {
                    self.open_project();
                }
                if ui
                    .add_enabled(!self.busy && self.project.is_some(), Button::new("保存"))
                    .clicked()
                {
                    self.save_project();
                }
                if ui
                    .add_enabled(
                        !self.busy && self.history.can_undo(),
                        Button::new("元に戻す"),
                    )
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(
                        !self.busy && self.history.can_redo(),
                        Button::new("やり直し"),
                    )
                    .clicked()
                {
                    self.redo();
                }
                if ui
                    .add_enabled(
                        !self.busy && self.project.is_some(),
                        Button::new(RichText::new("PDF出力").strong())
                            .fill(Color32::from_rgb(76, 194, 255)),
                    )
                    .clicked()
                {
                    self.pdf_dialog = Some(PdfDialogState {
                        mode: self.settings.pdf_export_mode,
                        minimum_font_size: self.settings.pdf_minimum_font_size,
                    });
                }
                ui.separator();
                if ui
                    .add_enabled(
                        !self.busy && self.selected_slide.is_some(),
                        Button::new("中点で分割"),
                    )
                    .clicked()
                {
                    self.start_split();
                }
                if ui
                    .add_enabled(
                        !self.busy && self.selected_slide.is_some() && slides.len() > 1,
                        Button::new("削除"),
                    )
                    .clicked()
                {
                    self.delete_slide();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        !self.busy && self.selected_slide.is_some_and(|i| i > 0),
                        Button::new("文字を前へ"),
                    )
                    .clicked()
                {
                    self.move_transcript(-1);
                }
                if ui
                    .add_enabled(
                        !self.busy && self.selected_slide.is_some_and(|i| i + 1 < slides.len()),
                        Button::new("文字を次へ"),
                    )
                    .clicked()
                {
                    self.move_transcript(1);
                }
                if ui
                    .add_enabled(
                        !self.busy && self.selected_slide.is_some(),
                        Button::new("画像差替"),
                    )
                    .clicked()
                {
                    self.replace_image();
                }
            });
            ui.separator();
            let slide = self.project.as_ref().and_then(|project| {
                self.selected_slide
                    .and_then(|index| project.slides.get(index))
                    .cloned()
            });
            if let Some(slide) = slide {
                let total = self
                    .project
                    .as_ref()
                    .map_or(0, |project| project.slides.len());
                ui.label(format!("スライド {} / {}", slide.number, total));
                if let Some(texture) = self.texture_for(ui.ctx(), &slide.image_path) {
                    ui.add(Image::new(&texture).max_size(ui.available_size()));
                } else {
                    ui.label("スライド画像が見つかりません");
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("動画を変換するか、プロジェクトを開いてください");
                });
            }
        });
    }

    fn show_settings(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.heading("設定");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("テーマ");
                    ui.checkbox(&mut self.settings.dark_mode, "ダークモード");
                });
                ui.separator();
                egui::Grid::new("settings-grid").num_columns(2).spacing([8.0, 8.0]).show(ui, |ui| {
                    ui.label("既定の出力先");
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut self.output_directory).desired_width(520.0));
                        if ui.button("参照...").clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.output_directory = path.to_string_lossy().to_string();
                            }
                    });
                    ui.end_row();
                    ui.label("モデル保存先");
                    ui.horizontal(|ui| {
                        let mut model_directory = self.settings.model_directory.to_string_lossy().to_string();
                        if ui.add(TextEdit::singleline(&mut model_directory).desired_width(520.0)).changed() {
                            self.settings.model_directory = PathBuf::from(model_directory);
                            self.model_path = self.settings.model_directory.join(MODEL_CHOICES[self.model_index].file_name).to_string_lossy().to_string();
                        }
                        if ui.button("参照...").clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.settings.model_directory = path;
                                self.model_path = self.settings.model_directory.join(MODEL_CHOICES[self.model_index].file_name).to_string_lossy().to_string();
                            }
                    });
                    ui.end_row();
                    ui.label("PDF最小フォント");
                    ui.add(DragValue::new(&mut self.settings.pdf_minimum_font_size).speed(0.25).range(5.0..=10.0).suffix(" pt"));
                    ui.end_row();
                });
                ui.label(RichText::new("小さい値ほど多くの文字を1ページに収められます。").weak());
                ui.separator();
                ui.heading("このアプリについて");
                ui.label("SlideVideoDivider 0.1.0");
                ui.label("処理はすべてローカルで実行されます。音声や動画は外部サービスへ送信されません。");
            });
        });
    }

    fn show_pdf_dialog(&mut self, ctx: &Context) {
        let Some(mut state) = self.pdf_dialog.take() else {
            return;
        };
        let mut open = true;
        let close_requested = Arc::new(AtomicBool::new(false));
        let button_close = close_requested.clone();
        egui::Window::new("PDF出力オプション")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("PDFに含める内容");
                ui.radio_value(
                    &mut state.mode,
                    PdfExportMode::Combined,
                    "スライド＋文字起こし（既定）",
                );
                ui.radio_value(&mut state.mode, PdfExportMode::SlidesOnly, "スライドのみ");
                ui.radio_value(
                    &mut state.mode,
                    PdfExportMode::TranscriptsOnly,
                    "文字起こしのみ",
                );
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("最小フォントサイズ");
                    ui.add(
                        DragValue::new(&mut state.minimum_font_size)
                            .speed(0.25)
                            .range(5.0..=10.0)
                            .suffix(" pt"),
                    );
                });
                ui.label(
                    RichText::new(
                        "スライドのみ／文字起こしのみは、それぞれ1スライド＝1ページです。",
                    )
                    .weak(),
                );
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("キャンセル").clicked() {
                        button_close.store(true, Ordering::Relaxed);
                    }
                    if ui.button("OK").clicked() {
                        button_close.store(true, Ordering::Relaxed);
                        self.confirm_pdf_export(state.mode, state.minimum_font_size);
                    }
                });
            });
        if !open || close_requested.load(Ordering::Relaxed) {
            self.pdf_dialog = None;
        } else {
            self.pdf_dialog = Some(state);
        }
    }

    fn show_message(&mut self, ctx: &Context) {
        let Some(message) = self.message.take() else {
            return;
        };
        let mut open = true;
        let close_requested = Arc::new(AtomicBool::new(false));
        let button_close = close_requested.clone();
        egui::Window::new(message.title.clone())
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(&message.text);
                if ui.button("OK").clicked() {
                    button_close.store(true, Ordering::Relaxed);
                }
            });
        if open && !close_requested.load(Ordering::Relaxed) {
            self.message = Some(message);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        if self.busy
            || (self.tab == Tab::Notes && self.transcript_has_focus)
            || self.pdf_dialog.is_some()
            || self.message.is_some()
        {
            return;
        }
        let (undo, redo) = ctx.input(|input| {
            let modifier = input.modifiers.ctrl || input.modifiers.command;
            (
                modifier && input.key_pressed(Key::Z) && !input.modifiers.shift,
                modifier
                    && (input.key_pressed(Key::Y)
                        || (input.key_pressed(Key::Z) && input.modifiers.shift)),
            )
        });
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
    }

    fn drain_events(&mut self, ctx: &Context) {
        let events: Vec<_> = self
            .receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                PipelineEvent::Status(message) => self.status = message,
                PipelineEvent::Overall(value) => self.progress = Some(value as f32),
                PipelineEvent::Finished(project) => {
                    self.load_project(project);
                    self.busy = false;
                    self.progress = None;
                    self.tab = Tab::Notes;
                }
                PipelineEvent::Failed(message) => {
                    self.busy = false;
                    self.progress = None;
                    self.status = "エラーが発生しました".to_string();
                    self.message = Some(MessageState {
                        title: "変換エラー".to_string(),
                        text: message,
                    });
                }
                PipelineEvent::Cancelled => {
                    self.busy = false;
                    self.progress = None;
                    self.status = "処理をキャンセルしました".to_string();
                }
                PipelineEvent::ModelReady => {
                    self.busy = false;
                    self.status = "モデルのダウンロードが完了しました".to_string();
                }
                PipelineEvent::PdfFinished(path) => {
                    self.busy = false;
                    self.status = format!("PDFを書き出しました: {}", path.display());
                    let _ = open::that(path);
                }
                PipelineEvent::PdfFailed(message) => {
                    self.busy = false;
                    self.status = "PDF出力に失敗しました".to_string();
                    self.message = Some(MessageState {
                        title: "PDF出力エラー".to_string(),
                        text: message,
                    });
                }
                PipelineEvent::FrameExtracted {
                    slide_index,
                    time,
                    image_path,
                } => {
                    self.busy = false;
                    if let Some(project) = self.project.as_mut()
                        && let Some(slide) = project.slides.get_mut(slide_index)
                    {
                        let old_end = slide.end;
                        slide.end = time;
                        project.slides.insert(
                            slide_index + 1,
                            crate::model::Slide {
                                number: slide_index + 2,
                                start: time,
                                end: old_end,
                                image_path,
                                transcript: String::new(),
                            },
                        );
                        project.renumber();
                        self.selected_slide = Some(slide_index + 1);
                        self.transcript_buffer.clear();
                        self.history.clear();
                        self.dirty = true;
                    }
                }
            }
        }
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn handle_dropped_files(&mut self, ctx: &Context) {
        if self.busy {
            return;
        }
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        for path in dropped {
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if extension == "json" {
                if let Ok(project) = project::load(&path) {
                    self.load_project(project);
                    self.tab = Tab::Notes;
                }
            } else if matches!(extension.as_str(), "mp4" | "mov" | "mkv" | "webm" | "avi") {
                self.video_path = path.to_string_lossy().to_string();
                self.tab = Tab::Convert;
            }
        }
    }

    fn start_pipeline(&mut self) {
        if self.busy
            || self.video_path.trim().is_empty()
            || !Path::new(self.video_path.trim()).is_file()
        {
            self.message = Some(MessageState {
                title: "入力エラー".to_string(),
                text: "動画ファイルを選択してください".to_string(),
            });
            return;
        }
        if self.output_directory.trim().is_empty() {
            self.message = Some(MessageState {
                title: "入力エラー".to_string(),
                text: "出力フォルダを指定してください".to_string(),
            });
            return;
        }
        if pipeline::ffmpeg::find_ffmpeg().is_none() {
            self.message = Some(MessageState {
                title: "FFmpegエラー".to_string(),
                text: "FFmpegが見つかりません。PATHを確認してください".to_string(),
            });
            return;
        }
        if self.model_path.trim().is_empty() {
            self.message = Some(MessageState {
                title: "入力エラー".to_string(),
                text: "音声認識モデルのパスを指定してください".to_string(),
            });
            return;
        }
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.receiver = Some(receiver);
        self.cancel = cancel.clone();
        self.busy = true;
        self.cancelable = true;
        self.progress = Some(0.0);
        let request = PipelineRequest {
            video_path: PathBuf::from(self.video_path.trim()),
            output_root: PathBuf::from(self.output_directory.trim()),
            model: MODEL_CHOICES[self.model_index].clone(),
            model_path: PathBuf::from(self.model_path.trim()),
            use_gpu: self.use_gpu,
            initial_prompt: self.initial_prompt.clone(),
            scene_threshold: self.scene_threshold,
            minimum_slide_seconds: self.minimum_slide_seconds,
        };
        std::thread::spawn(move || {
            let context = PipelineContext::new(sender.clone(), cancel);
            match pipeline::run_pipeline(request, &context) {
                Ok(project) => {
                    let _ = sender.send(PipelineEvent::Finished(project));
                }
                Err(error) => {
                    if error
                        .downcast_ref::<pipeline::ffmpeg::Cancelled>()
                        .is_some()
                        || context.cancel.load(Ordering::Relaxed)
                    {
                        let _ = sender.send(PipelineEvent::Cancelled);
                    } else {
                        let _ = sender.send(PipelineEvent::Failed(error.to_string()));
                    }
                }
            }
        });
    }

    fn start_model_download(&mut self) {
        if self.busy {
            return;
        }
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.receiver = Some(receiver);
        self.cancel = cancel.clone();
        self.busy = true;
        self.cancelable = true;
        let choice = MODEL_CHOICES[self.model_index].clone();
        let model_path = PathBuf::from(self.model_path.trim());
        std::thread::spawn(move || {
            let context = PipelineContext::new(sender.clone(), cancel);
            match pipeline::transcribe::ensure_model(&choice, &model_path, &context) {
                Ok(()) => {
                    let _ = sender.send(PipelineEvent::ModelReady);
                }
                Err(error) => {
                    if error
                        .downcast_ref::<pipeline::ffmpeg::Cancelled>()
                        .is_some()
                        || context.cancel.load(Ordering::Relaxed)
                    {
                        let _ = sender.send(PipelineEvent::Cancelled);
                    } else {
                        let _ = sender.send(PipelineEvent::Failed(error.to_string()));
                    }
                }
            }
        });
    }

    fn start_split(&mut self) {
        let Some(index) = self.selected_slide else {
            return;
        };
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let Some(slide) = project.slides.get(index) else {
            return;
        };
        if slide.duration() < 1.0 {
            self.message = Some(MessageState {
                title: "分割エラー".to_string(),
                text: "分割するには表示時間が1秒以上必要です".to_string(),
            });
            return;
        }
        let time = slide.start + slide.duration() / 2.0;
        let image_path = project
            .project_path
            .as_ref()
            .and_then(|path| path.parent())
            .unwrap_or_else(|| Path::new("."))
            .join("slides")
            .join(format!(
                "slide_extra_{}.jpg",
                chrono::Local::now().timestamp_millis()
            ));
        let video = project.video_path.clone();
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.receiver = Some(receiver);
        self.cancel = cancel.clone();
        self.busy = true;
        self.cancelable = true;
        std::thread::spawn(move || {
            let context = PipelineContext::new(sender.clone(), cancel);
            let result =
                pipeline::ffmpeg::extract_frame(&video, time, &image_path, &context.cancel);
            match result {
                Ok(()) => {
                    let _ = sender.send(PipelineEvent::FrameExtracted {
                        slide_index: index,
                        time,
                        image_path,
                    });
                }
                Err(error) => {
                    if error
                        .downcast_ref::<pipeline::ffmpeg::Cancelled>()
                        .is_some()
                        || context.cancel.load(Ordering::Relaxed)
                    {
                        let _ = sender.send(PipelineEvent::Cancelled);
                    } else {
                        let _ = sender.send(PipelineEvent::Failed(error.to_string()));
                    }
                }
            }
        });
    }

    fn select_slide(&mut self, index: usize) {
        self.commit_transcript();
        self.selected_slide = Some(index);
        self.transcript_buffer = self
            .project
            .as_ref()
            .and_then(|project| project.slides.get(index))
            .map(|slide| slide.transcript.clone())
            .unwrap_or_default();
    }

    fn commit_transcript(&mut self) {
        if let (Some(index), Some(project)) = (self.selected_slide, self.project.as_mut())
            && let Some(slide) = project.slides.get_mut(index)
        {
            slide.transcript = self.transcript_buffer.clone();
        }
    }

    fn move_transcript(&mut self, direction: i32) {
        self.commit_transcript();
        let Some(index) = self.selected_slide else {
            return;
        };
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let target_index = index as i32 + direction;
        if target_index < 0 || target_index as usize >= project.slides.len() {
            return;
        }
        let target_index = target_index as usize;
        let source_text = project::normalize_transcript(&project.slides[index].transcript);
        if source_text.is_empty() {
            return;
        }
        let target_text = project::normalize_transcript(&project.slides[target_index].transcript);
        let command = EditCommand::MoveTranscript {
            source: index,
            target: target_index,
            source_text,
            target_text,
        };
        command.apply(project);
        self.history.push(command);
        self.transcript_buffer.clear();
        self.dirty = true;
    }

    fn replace_image(&mut self) {
        let Some(index) = self.selected_slide else {
            return;
        };
        let Some(source) = rfd::FileDialog::new()
            .add_filter("画像", &["jpg", "jpeg", "png", "bmp"])
            .pick_file()
        else {
            return;
        };
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let directory = project
            .project_path
            .as_ref()
            .and_then(|path| path.parent())
            .unwrap_or_else(|| Path::new("."))
            .join("slides");
        if fs::create_dir_all(&directory).is_err() {
            return;
        }
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");
        let destination = directory.join(format!(
            "slide_replace_{}.{}",
            chrono::Local::now().timestamp_millis(),
            extension
        ));
        if fs::copy(&source, &destination).is_err() {
            return;
        }
        let old_path = project.slides[index].image_path.clone();
        let command = EditCommand::ReplaceImage {
            index,
            old_path,
            new_path: destination,
        };
        command.apply(project);
        self.history.push(command);
        self.dirty = true;
    }

    fn delete_slide(&mut self) {
        let Some(index) = self.selected_slide else {
            return;
        };
        let Some(project) = self.project.as_mut() else {
            return;
        };
        if project.slides.len() <= 1 {
            return;
        }
        if index > 0 {
            project.slides[index - 1].end = project.slides[index].end;
        } else {
            project.slides[1].start = project.slides[0].start;
        }
        project.slides.remove(index);
        project.renumber();
        self.selected_slide = Some(index.min(project.slides.len() - 1));
        let selected = self.selected_slide.unwrap();
        self.transcript_buffer = project.slides[selected].transcript.clone();
        self.history.clear();
        self.dirty = true;
    }

    fn undo(&mut self) {
        if self.busy {
            return;
        }
        self.commit_transcript();
        if let Some(project) = self.project.as_mut()
            && self.history.undo(project)
        {
            self.dirty = true;
            self.reload_selected();
        }
    }

    fn redo(&mut self) {
        if self.busy {
            return;
        }
        self.commit_transcript();
        if let Some(project) = self.project.as_mut()
            && self.history.redo(project)
        {
            self.dirty = true;
            self.reload_selected();
        }
    }

    fn reload_selected(&mut self) {
        self.transcript_buffer = self
            .project
            .as_ref()
            .and_then(|project| {
                self.selected_slide
                    .and_then(|index| project.slides.get(index))
            })
            .map(|slide| slide.transcript.clone())
            .unwrap_or_default();
    }

    fn load_project(&mut self, project: Project) {
        if self.dirty {
            self.commit_transcript();
            if let Some(current) = self.project.as_mut()
                && let Some(path) = current.project_path.clone()
            {
                let _ = project::save(&path, current);
            }
        }
        self.project = Some(project);
        self.selected_slide = if self.project.as_ref().is_some_and(|p| !p.slides.is_empty()) {
            Some(0)
        } else {
            None
        };
        self.reload_selected();
        self.history.clear();
        self.dirty = false;
        self.video_path = self
            .project
            .as_ref()
            .map(|p| p.video_path.to_string_lossy().to_string())
            .unwrap_or_default();
        self.textures.clear();
    }

    fn open_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("プロジェクト", &["json"])
            .pick_file()
        {
            match project::load(&path) {
                Ok(project) => {
                    self.load_project(project);
                    self.tab = Tab::Notes;
                }
                Err(error) => {
                    self.message = Some(MessageState {
                        title: "プロジェクトを開けません".to_string(),
                        text: error.to_string(),
                    })
                }
            }
        }
    }

    fn save_project(&mut self) {
        self.commit_transcript();
        let Some(project) = self.project.as_mut() else {
            return;
        };
        if project.project_path.is_none() {
            let name = format!(
                "{}_project.json",
                Path::new(project.video_name.as_str())
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            let Some(path) = rfd::FileDialog::new()
                .add_filter("プロジェクト", &["json"])
                .set_file_name(name.as_str())
                .save_file()
            else {
                return;
            };
            project.project_path = Some(path);
        }
        let path = project.project_path.clone().unwrap();
        match project::save(&path, project) {
            Ok(()) => {
                self.dirty = false;
                self.status = format!("プロジェクトを保存しました: {}", path.display());
            }
            Err(error) => {
                self.message = Some(MessageState {
                    title: "保存エラー".to_string(),
                    text: error.to_string(),
                })
            }
        }
    }

    fn confirm_pdf_export(&mut self, mode: PdfExportMode, minimum_font_size: f64) {
        self.commit_transcript();
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let suffix = match mode {
            PdfExportMode::Combined => "_スライドノート",
            PdfExportMode::SlidesOnly => "_スライド",
            PdfExportMode::TranscriptsOnly => "_文字起こし",
        };
        let name = format!(
            "{}{suffix}.pdf",
            Path::new(project.video_name.as_str())
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name(name.as_str())
            .save_file()
        else {
            return;
        };
        self.settings.pdf_export_mode = mode;
        self.settings.pdf_minimum_font_size = minimum_font_size;
        let project = project.clone();
        let (sender, receiver) = channel();
        self.receiver = Some(receiver);
        self.busy = true;
        self.cancelable = false;
        std::thread::spawn(move || {
            let _ = sender.send(PipelineEvent::Status(
                "PDFを書き出しています...".to_string(),
            ));
            match pdf::export(
                &project,
                &path,
                PdfOptions {
                    mode,
                    minimum_font_size,
                },
            ) {
                Ok(()) => {
                    let _ = sender.send(PipelineEvent::PdfFinished(path));
                }
                Err(error) => {
                    let _ = sender.send(PipelineEvent::PdfFailed(error.to_string()));
                }
            }
        });
    }

    fn texture_for(&mut self, ctx: &Context, path: &Path) -> Option<TextureHandle> {
        if let Some(texture) = self.textures.get(path) {
            return Some(texture.clone());
        }
        let image = image::open(path)
            .ok()?
            .resize(640, 360, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (width, height) = image.dimensions();
        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &image);
        let texture = ctx.load_texture(
            path.to_string_lossy().into_owned(),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.textures.insert(path.to_path_buf(), texture.clone());
        Some(texture)
    }

    fn sync_settings(&mut self) {
        self.settings.output_directory = PathBuf::from(self.output_directory.trim());
        self.settings.model_index = self.model_index;
        self.settings.use_gpu = self.use_gpu;
        self.settings.scene_threshold = self.scene_threshold;
        self.settings.minimum_slide_seconds = self.minimum_slide_seconds;
        self.settings.initial_prompt = self.initial_prompt.clone();
    }
}

impl App for SlideVideoDividerApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame) {
        SlideVideoDividerApp::render(self, ui, frame);
    }

    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        self.sync_settings();
        if self.dirty {
            self.commit_transcript();
            if let Some(project) = self.project.as_mut()
                && let Some(path) = project.project_path.clone()
            {
                let _ = project::save(&path, project);
            }
        }
        let _ = self.settings.save();
    }
}

#[cfg(target_os = "windows")]
fn apply_title_bar(frame: &Frame, dark: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::c_void;
    use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};

    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut c_void);
    let value: i32 = if dark { 1 } else { 0 };
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&value as *const i32).cast::<c_void>(),
            std::mem::size_of::<i32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_title_bar(_frame: &Frame, _dark: bool) {}

fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "M PLUS 1p".to_string(),
        egui::FontData::from_static(FONT_BYTES).into(),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "M PLUS 1p".to_string());
    fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap()
        .push("M PLUS 1p".to_string());
    ctx.set_fonts(fonts);
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
