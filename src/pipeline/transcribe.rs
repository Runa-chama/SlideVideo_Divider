use crate::model::{ModelChoice, TranscriptSegment};
use crate::pipeline::PipelineContext;
use anyhow::{Context, Result, anyhow};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub fn ensure_model(choice: &ModelChoice, model_path: &Path, ctx: &PipelineContext) -> Result<()> {
    if model_path.is_file()
        && fs::metadata(model_path)
            .map(|m| m.len() > 1024 * 1024)
            .unwrap_or(false)
    {
        return Ok(());
    }
    if model_path.exists() {
        fs::remove_file(model_path)?;
    }
    fs::create_dir_all(model_path.parent().unwrap_or_else(|| Path::new(".")))?;
    let partial = model_path.with_extension("bin.part");
    let url = format!(
        "https://huggingface.co/sandrohanea/whisper.net/resolve/v5/{}/{}",
        choice.subdirectory, choice.download_name
    );
    ctx.status(format!(
        "音声認識モデルをダウンロード中... {}",
        choice.display_name
    ));
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(60 * 60))
        .build()
        .context("モデルダウンロード用クライアントを作成できませんでした")?;
    let mut response = client
        .get(&url)
        .send()
        .context("モデルダウンロードの開始に失敗しました")?
        .error_for_status()?;
    let mut output = File::create(&partial)?;
    let mut buffer = vec![0u8; 128 * 1024];
    let mut total = 0u64;
    let mut last_report = 0u64;
    const REPORT_INTERVAL: u64 = 5 * 1024 * 1024;
    loop {
        ctx.check_cancel()?;
        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        total += read as u64;
        if total - last_report >= REPORT_INTERVAL {
            last_report = total;
            ctx.status(format!(
                "音声認識モデルをダウンロード中... {:.0} MB",
                total as f64 / 1024.0 / 1024.0
            ));
        }
    }
    output.flush()?;
    drop(output);
    if total <= 1024 * 1024 {
        return Err(anyhow!("音声認識モデルのダウンロードに失敗しました"));
    }
    fs::rename(&partial, model_path)?;
    Ok(())
}

pub fn transcribe(
    wav_path: &Path,
    model_path: &Path,
    use_gpu: bool,
    initial_prompt: &str,
    ctx: &PipelineContext,
) -> Result<Vec<TranscriptSegment>> {
    if !model_path.is_file() {
        return Err(anyhow!(
            "音声認識モデルが見つかりません。先にモデルをダウンロードしてください。"
        ));
    }
    ctx.status("音声認識モデルを読み込んでいます...");
    let mut gpu_params = WhisperContextParameters::new();
    gpu_params.use_gpu(use_gpu).flash_attn(false);
    let context = match WhisperContext::new_with_params(model_path, gpu_params) {
        Ok(context) => context,
        Err(error) if use_gpu => {
            ctx.status(format!("GPU初期化に失敗したためCPUへ切り替えます: {error}"));
            let mut cpu_params = WhisperContextParameters::new();
            cpu_params.use_gpu(false).flash_attn(false);
            WhisperContext::new_with_params(model_path, cpu_params)
                .context("CPUでの音声認識モデル読み込みにも失敗しました")?
        }
        Err(error) => return Err(error.into()),
    };
    ctx.status("文字起こし中...");
    let mut pcm = read_wav(wav_path)?;
    if pcm.len() < 30 * 16000 {
        pcm.resize(30 * 16000, 0.0);
    }
    let peak = pcm
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    ctx.status(format!(
        "文字起こし中... 音声サンプル {} / ピーク {peak:.3}",
        pcm.len()
    ));
    let mut state = context.create_state()?;
    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: 5,
        patience: -1.0,
    });
    params.set_n_threads(
        std::thread::available_parallelism()
            .map(|n| n.get().clamp(1, 4) as i32)
            .unwrap_or(4),
    );
    params.set_language(Some("ja"));
    params.set_no_context(true);
    params.set_token_timestamps(true);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    if !initial_prompt.trim().is_empty() {
        params.set_initial_prompt(initial_prompt.trim());
    }
    let progress_ctx = ctx.clone();
    params.set_progress_callback_safe(move |percent: i32| {
        let percent = percent.clamp(0, 100);
        progress_ctx.overall(50.0 + percent as f64 * 0.45);
        progress_ctx.status(format!("文字起こし中... {percent}%"));
    });
    let cancel = ctx.cancel.clone();
    // whisper-rs 0.16.0 の安全版 abort callback は参照渡しが壊れているため、
    // 有効期間中のみ参照する自前の C trampoline を使用する。
    unsafe {
        params.set_abort_callback(Some(abort_trampoline));
        params.set_abort_callback_user_data(Arc::as_ptr(&cancel) as *mut std::ffi::c_void);
    }
    if let Err(error) = state.full(params, &pcm) {
        ctx.check_cancel()?;
        return Err(error.into());
    }
    ctx.check_cancel()?;
    let mut segments = Vec::new();
    for segment in state.as_iter() {
        let text =
            crate::project::normalize_transcript(&segment.to_str_lossy().unwrap_or_default());
        if text.is_empty() {
            continue;
        }
        let mut tokens = Vec::new();
        for token_index in 0..segment.n_tokens() {
            let Some(token) = segment.get_token(token_index) else {
                continue;
            };
            let token_text = token.to_str_lossy().unwrap_or_default();
            if token_text.is_empty()
                || (token_text.starts_with("<|") && token_text.ends_with("|>"))
                || (token_text.starts_with("[_") && token_text.ends_with("_]"))
            {
                continue;
            }
            let data = token.token_data();
            if data.t0 >= 0 && data.t1 > data.t0 {
                tokens.push(crate::model::TranscriptToken {
                    start: data.t0 as f64 / 10.0,
                    end: data.t1 as f64 / 10.0,
                    text: token_text.to_string(),
                });
            }
        }

        segments.push(TranscriptSegment {
            start: segment.start_timestamp() as f64 / 10.0,
            end: segment.end_timestamp() as f64 / 10.0,
            text,
            tokens,
        });
    }
    if segments.is_empty() {
        return Err(anyhow!(
            "文字起こし結果が空でした。音声の有無・言語設定を確認してください。"
        ));
    }
    Ok(segments)
}

unsafe extern "C" fn abort_trampoline(user_data: *mut std::ffi::c_void) -> bool {
    let cancel = unsafe { &*(user_data as *const AtomicBool) };
    cancel.load(Ordering::Relaxed)
}

fn read_wav(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).context("音声WAVを開けませんでした")?;
    let spec = reader.spec();
    if spec.sample_rate != 16000 || spec.channels != 1 {
        return Err(anyhow!("音声は16kHzモノラルである必要があります"));
    }
    let mut samples = Vec::new();
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                samples.push(sample?);
            }
        }
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            match bits {
                8 => {
                    for sample in reader.samples::<i8>() {
                        samples.push(sample? as f32 / 128.0);
                    }
                }
                16 => {
                    for sample in reader.samples::<i16>() {
                        samples.push(sample? as f32 / 32768.0);
                    }
                }
                24 => {
                    for sample in reader.samples::<i32>() {
                        samples.push(sample? as f32 / 8_388_608.0);
                    }
                }
                32 => {
                    for sample in reader.samples::<i32>() {
                        samples.push(sample? as f32 / 2_147_483_648.0);
                    }
                }
                _ => return Err(anyhow!("未対応の音声ビット深度です: {bits}bit")),
            }
        }
    }
    Ok(samples)
}
