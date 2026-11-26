#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use slide_video_divider::app;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();
    let initial_project = std::env::args_os().skip(1).map(PathBuf::from).find(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            && path.is_file()
    });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SlideVideoDivider")
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([680.0, 480.0])
            .with_clamp_size_to_monitor_size(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "SlideVideoDivider",
        options,
        Box::new(move |creation_context| {
            Ok(Box::new(app::SlideVideoDividerApp::new(
                creation_context,
                initial_project,
            )))
        }),
    )
}
