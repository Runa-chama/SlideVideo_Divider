use anyhow::{Context, Result, anyhow};
use regex::Regex;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::model::Slide;

#[derive(Debug, Clone, Copy)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("処理はキャンセルされました")
    }
}

impl std::error::Error for Cancelled {}

pub fn find_ffmpeg() -> Option<PathBuf> {
    find_tool("ffmpeg.exe")
}

pub fn find_ffprobe() -> Option<PathBuf> {
    find_tool("ffprobe.exe")
}

fn find_tool(name: &str) -> Option<PathBuf> {
    let local = PathBuf::from("tools").join(name);
    if local.is_file() {
        return Some(local);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|directory| {
            let candidate = directory.join(name);
            candidate.is_file().then_some(candidate)
        })
    })
}

pub fn probe_duration(video: &Path, cancel: &Arc<AtomicBool>) -> Result<f64> {
    let ffprobe = find_ffprobe().context("FFprobeが見つかりません。PATHを確認してください。")?;
    let output = run_command(
        &ffprobe,
        &[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
            &video.to_string_lossy(),
        ],
        cancel,
    )?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|duration| *duration > 0.0)
        .context("動画の再生時間を取得できませんでした")
}

pub fn detect_scene_times(
    video: &Path,
    duration: f64,
    threshold: f64,
    minimum_seconds: f64,
    cancel: &Arc<AtomicBool>,
) -> Result<Vec<f64>> {
    let ffmpeg = find_ffmpeg().context("FFmpegが見つかりません。PATHを確認してください。")?;
    let filter = format!("select=gt(scene\\,{threshold}),metadata=print:file=-");
    let output = run_command(
        &ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            &video.to_string_lossy(),
            "-vf",
            &filter,
            "-an",
            "-f",
            "null",
            "-",
        ],
        cancel,
    )?;
    let regex = Regex::new(r"pts_time:([0-9]+(?:\.[0-9]+)?)")?;
    let mut times = vec![0.0];
    for capture in regex.captures_iter(&String::from_utf8_lossy(&output.stdout)) {
        let Ok(time) = capture[1].parse::<f64>() else {
            continue;
        };
        if time > 0.05
            && time < duration - minimum_seconds
            && time - times.last().unwrap() >= minimum_seconds
        {
            times.push(time);
        }
    }
    Ok(times)
}

pub fn extract_slides(
    video: &Path,
    boundaries: &[f64],
    duration: f64,
    slides_directory: &Path,
    cancel: &Arc<AtomicBool>,
    progress: &impl Fn(f64),
) -> Result<Vec<Slide>> {
    let ffmpeg = find_ffmpeg().context("FFmpegが見つかりません。PATHを確認してください。")?;
    std::fs::create_dir_all(slides_directory)?;
    let mut slides = Vec::new();
    for (index, &start) in boundaries.iter().enumerate() {
        check_cancel(cancel)?;
        let end = boundaries.get(index + 1).copied().unwrap_or(duration);
        if end <= start {
            continue;
        }
        let offset = ((end - start) * 0.12).clamp(0.20, 1.25);
        let capture_time = (end - 0.05).min(start + offset);
        let image_path = slides_directory.join(format!("slide_{:03}.jpg", index + 1));
        run_command(
            &ffmpeg,
            &[
                "-y",
                "-v",
                "error",
                "-ss",
                &capture_time.to_string(),
                "-i",
                &video.to_string_lossy(),
                "-frames:v",
                "1",
                "-q:v",
                "2",
                &image_path.to_string_lossy(),
            ],
            cancel,
        )?;
        if !image_path.is_file() {
            return Err(anyhow!(
                "スライド画像を抽出できませんでした: {capture_time:.2}秒"
            ));
        }
        slides.push(Slide {
            number: slides.len() + 1,
            start,
            end,
            image_path,
            transcript: String::new(),
        });
        progress((index + 1) as f64 / boundaries.len() as f64 * 100.0);
    }
    if slides.is_empty() {
        return Err(anyhow!("スライドを抽出できませんでした"));
    }
    Ok(slides)
}

pub fn extract_audio(video: &Path, wav_path: &Path, cancel: &Arc<AtomicBool>) -> Result<()> {
    let ffmpeg = find_ffmpeg().context("FFmpegが見つかりません。PATHを確認してください。")?;
    run_command(
        &ffmpeg,
        &[
            "-y",
            "-v",
            "error",
            "-i",
            &video.to_string_lossy(),
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-c:a",
            "pcm_s16le",
            &wav_path.to_string_lossy(),
        ],
        cancel,
    )?;
    if !wav_path.is_file() {
        return Err(anyhow!(
            "音声を抽出できませんでした。動画に音声トラックがあるか確認してください。"
        ));
    }
    Ok(())
}

pub fn extract_frame(
    video: &Path,
    time: f64,
    output: &Path,
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    let ffmpeg = find_ffmpeg().context("FFmpegが見つかりません。PATHを確認してください。")?;
    run_command(
        &ffmpeg,
        &[
            "-y",
            "-v",
            "error",
            "-ss",
            &time.to_string(),
            "-i",
            &video.to_string_lossy(),
            "-frames:v",
            "1",
            "-q:v",
            "2",
            &output.to_string_lossy(),
        ],
        cancel,
    )?;
    if !output.is_file() {
        return Err(anyhow!("指定時刻のフレームを抽出できませんでした"));
    }
    Ok(())
}

pub fn check_cancel(cancel: &Arc<AtomicBool>) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(Cancelled.into())
    } else {
        Ok(())
    }
}

fn run_command(program: &Path, arguments: &[&str], cancel: &Arc<AtomicBool>) -> Result<Output> {
    check_cancel(cancel)?;
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("プロセスを開始できません: {}", program.display()))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_thread = std::thread::spawn(move || {
        stdout
            .map(|mut stream| {
                let mut data = Vec::new();
                stream.read_to_end(&mut data).ok();
                data
            })
            .unwrap_or_default()
    });
    let stderr_thread = std::thread::spawn(move || {
        stderr
            .map(|mut stream| {
                let mut data = Vec::new();
                stream.read_to_end(&mut data).ok();
                data
            })
            .unwrap_or_default()
    });
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Cancelled.into());
        }
        if let Some(status) = child.try_wait()? {
            let stdout = stdout_thread.join().unwrap_or_default();
            let stderr = stderr_thread.join().unwrap_or_default();
            if !status.success() {
                let message = String::from_utf8_lossy(&stderr).trim().to_string();
                return Err(anyhow!(
                    "{}",
                    if message.is_empty() {
                        format!("{} failed (exit code {})", program.display(), status)
                    } else {
                        message
                    }
                ));
            }
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}
