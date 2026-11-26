use slide_video_divider::model::{MODEL_CHOICES, PdfExportMode, PdfOptions};
use slide_video_divider::pdf;
use slide_video_divider::pipeline::{self, PipelineContext, PipelineEvent, PipelineRequest};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

fn main() {
    let mut video = None;
    let mut output = PathBuf::from("smoke-output");
    let mut model_path = None;
    let mut model_index = 0usize;
    let mut use_gpu = slide_video_divider::gpu::detect_gpu().is_some();
    let mut export_pdfs = false;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--video" => video = args.next().map(PathBuf::from),
            "--output" => output = args.next().map(PathBuf::from).unwrap_or(output),
            "--model" => model_path = args.next().map(PathBuf::from),
            "--model-index" => {
                model_index = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0)
            }
            "--cpu" => use_gpu = false,
            "--pdf" => export_pdfs = true,
            _ => {}
        }
    }
    let Some(video) = video else {
        eprintln!(
            "Usage: smoke --video <path> [--output <dir>] [--model <bin>] [--model-index <0-4>] [--cpu]"
        );
        std::process::exit(2);
    };
    model_index = model_index.min(MODEL_CHOICES.len() - 1);
    let model_path = model_path.unwrap_or_else(|| {
        slide_video_divider::settings::AppSettings::default()
            .model_directory
            .join(MODEL_CHOICES[model_index].file_name)
    });
    let (sender, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_sender = sender.clone();
    let worker_cancel = cancel.clone();
    let request = PipelineRequest {
        video_path: video,
        output_root: output,
        model: MODEL_CHOICES[model_index].clone(),
        model_path,
        use_gpu,
        initial_prompt: "工学、数学、物理学、制御工学、専門用語。".to_string(),
        scene_threshold: 0.12,
        minimum_slide_seconds: 1.5,
    };
    std::thread::spawn(move || {
        let context = PipelineContext::new(worker_sender.clone(), worker_cancel);
        match pipeline::run_pipeline(request, &context) {
            Ok(project) => {
                let _ = worker_sender.send(PipelineEvent::Finished(project));
            }
            Err(error) => {
                if error
                    .downcast_ref::<pipeline::ffmpeg::Cancelled>()
                    .is_some()
                {
                    let _ = worker_sender.send(PipelineEvent::Cancelled);
                } else {
                    let _ = worker_sender.send(PipelineEvent::Failed(error.to_string()));
                }
            }
        }
    });
    while let Ok(event) = receiver.recv() {
        match event {
            PipelineEvent::Status(message) => println!("{message}"),
            PipelineEvent::Overall(value) => println!("progress {value:.1}%"),
            PipelineEvent::Finished(project) => {
                if export_pdfs {
                    let directory = project
                        .project_path
                        .as_ref()
                        .and_then(|path| path.parent())
                        .unwrap_or_else(|| std::path::Path::new("."));
                    for mode in [
                        PdfExportMode::Combined,
                        PdfExportMode::SlidesOnly,
                        PdfExportMode::TranscriptsOnly,
                    ] {
                        let path = directory.join(format!("smoke_{mode:?}.pdf"));
                        pdf::export(
                            &project,
                            &path,
                            PdfOptions {
                                mode,
                                minimum_font_size: 6.5,
                            },
                        )
                        .unwrap_or_else(|error| {
                            eprintln!("PDF export failed: {error}");
                            std::process::exit(1);
                        });
                        println!("PDF: {}", path.display());
                    }
                }
                println!(
                    "SMOKE TEST PASSED: {} slides, {} segments",
                    project.slides.len(),
                    project.segments.len()
                );
                return;
            }
            PipelineEvent::Failed(message) => {
                eprintln!("SMOKE TEST FAILED: {message}");
                std::process::exit(1);
            }
            PipelineEvent::Cancelled => {
                eprintln!("cancelled");
                std::process::exit(1);
            }
            _ => {}
        }
    }
}
