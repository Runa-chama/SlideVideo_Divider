use crate::model::{ModelChoice, TranscriptSegment};
use crate::pipeline::PipelineContext;
use anyhow::{Context, Result, anyhow};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const SAMPLE_RATE: usize = 16_000;
/// このピーク振幅を下回る区間は無音として文字起こしを省略する。
const SILENCE_PEAK: f32 = 0.01;
/// GPUで並列実行する場合の1コンテキストあたりの想定モデルサイズ上限。
const GPU_PARALLEL_MODEL_LIMIT: u64 = 1_200_000_000;

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

pub struct SlideTranscript {
    pub index: usize,
    pub segments: Vec<TranscriptSegment>,
}

struct ChunkJob {
    slide_index: usize,
    start_time: f64,
    end_time: f64,
    start_sample: usize,
    end_sample: usize,
    silent: bool,
}

struct AbortFlags {
    cancel: Arc<AtomicBool>,
    abort: Arc<AtomicBool>,
}

struct ProgressTracker {
    total: usize,
    done: usize,
    active: HashMap<usize, f64>,
    ctx: PipelineContext,
}

impl ProgressTracker {
    fn new(total: usize, silent: usize, ctx: PipelineContext) -> Self {
        Self {
            total,
            done: silent,
            active: HashMap::new(),
            ctx,
        }
    }

    fn report(&self) {
        let progress = if self.total == 0 {
            1.0
        } else {
            (self.done as f64 + self.active.values().sum::<f64>()) / self.total as f64
        }
        .clamp(0.0, 1.0);
        self.ctx.overall(50.0 + progress * 45.0);
        self.ctx.status(format!(
            "文字起こし中... {:.0}%（{} / {}スライド）",
            progress * 100.0,
            self.done.min(self.total),
            self.total
        ));
    }

    fn start(&mut self, index: usize) {
        self.active.insert(index, 0.0);
        self.report();
    }

    fn update(&mut self, index: usize, fraction: f64) {
        self.active.insert(index, fraction.clamp(0.0, 1.0));
        self.report();
    }

    fn finish(&mut self, index: usize) {
        self.active.remove(&index);
        self.done += 1;
        self.report();
    }
}

/// スライド区間ごとに音声を分割して文字起こしする。
///
/// 分割点をスライド境界付近の無音へ寄せることで、単語の途中で切ることを避ける。
/// 文字起こし結果は発話の重なり判定を経ず、そのまま対応するスライドへ割り当てる。
pub fn transcribe_slides(
    wav_path: &Path,
    model_path: &Path,
    use_gpu: bool,
    initial_prompt: &str,
    ranges: &[(f64, f64)],
    ctx: &PipelineContext,
) -> Result<Vec<SlideTranscript>> {
    if !model_path.is_file() {
        return Err(anyhow!(
            "音声認識モデルが見つかりません。先にモデルをダウンロードしてください。"
        ));
    }
    if ranges.is_empty() {
        return Err(anyhow!("スライド区間がありません"));
    }
    ctx.status("音声をスライド境界で分割しています...");
    let pcm = Arc::new(read_wav(wav_path)?);
    let jobs = build_jobs(&pcm, ranges);
    let silent = jobs.iter().filter(|job| job.silent).count();
    let pending: Vec<usize> = jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| !job.silent)
        .map(|(index, _)| index)
        .collect();
    if pending.is_empty() {
        return Err(anyhow!(
            "音声が検出できませんでした。動画に音声が含まれているか確認してください。"
        ));
    }

    let model_size = fs::metadata(model_path).map(|meta| meta.len()).unwrap_or(0);
    let desired_workers = if use_gpu && model_size <= GPU_PARALLEL_MODEL_LIMIT {
        2
    } else {
        1
    };
    let workers = desired_workers.min(pending.len()).max(1);

    ctx.status("音声認識モデルを読み込んでいます...");
    let contexts = create_contexts(model_path, use_gpu, workers, ctx)?;

    let total = jobs.len();
    let jobs = Arc::new(jobs);
    let pcm_ref = pcm.clone();
    let queue = Arc::new(Mutex::new(VecDeque::from(pending.clone())));
    let tracker = Arc::new(Mutex::new(ProgressTracker::new(total, silent, ctx.clone())));
    let abort = Arc::new(AtomicBool::new(false));
    let prompt = initial_prompt.trim().to_string();
    let (sender, receiver) = mpsc::channel::<Result<SlideTranscript, String>>();
    let mut handles = Vec::new();
    for context in contexts {
        let jobs = jobs.clone();
        let pcm = pcm_ref.clone();
        let queue = queue.clone();
        let tracker = tracker.clone();
        let abort = abort.clone();
        let cancel = ctx.cancel.clone();
        let sender = sender.clone();
        let prompt = prompt.clone();
        handles.push(std::thread::spawn(move || {
            run_worker(
                context, jobs, pcm, queue, tracker, abort, cancel, sender, prompt,
            );
        }));
    }
    drop(sender);

    let mut segments_by_slide: Vec<Option<Vec<TranscriptSegment>>> =
        (0..total).map(|_| None).collect();
    let mut first_error = None;
    for outcome in receiver {
        match outcome {
            Ok(result) => segments_by_slide[result.index] = Some(result.segments),
            Err(message) => {
                if first_error.is_none() {
                    first_error = Some(message);
                }
                abort.store(true, Ordering::Relaxed);
            }
        }
    }
    for handle in handles {
        let _ = handle.join();
    }
    ctx.check_cancel()?;
    if let Some(message) = first_error {
        return Err(anyhow!(message));
    }
    let mut results = Vec::with_capacity(total);
    for (index, segments) in segments_by_slide.into_iter().enumerate() {
        results.push(SlideTranscript {
            index,
            segments: segments.unwrap_or_default(),
        });
    }
    Ok(results)
}

fn build_jobs(pcm: &[f32], ranges: &[(f64, f64)]) -> Vec<ChunkJob> {
    let boundaries = snapped_boundaries(pcm, ranges);
    let mut jobs = Vec::with_capacity(ranges.len());
    for (index, &(start, end)) in ranges.iter().enumerate() {
        let start = if index == 0 {
            start
        } else {
            boundaries[index - 1]
        };
        let end = if index + 1 == ranges.len() {
            end
        } else {
            boundaries[index]
        };
        let start_sample = ((start.max(0.0) * SAMPLE_RATE as f64) as usize).min(pcm.len());
        let end_sample = ((end.max(0.0) * SAMPLE_RATE as f64) as usize).min(pcm.len());
        let end_sample = end_sample.max(start_sample).min(pcm.len());
        let slice = &pcm[start_sample..end_sample];
        let peak = slice
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        jobs.push(ChunkJob {
            slide_index: index,
            start_time: start,
            end_time: end,
            start_sample,
            end_sample,
            silent: peak < SILENCE_PEAK || slice.len() < SAMPLE_RATE / 2,
        });
    }
    jobs
}

fn snapped_boundaries(pcm: &[f32], ranges: &[(f64, f64)]) -> Vec<f64> {
    let mut boundaries = Vec::with_capacity(ranges.len().saturating_sub(1));
    for index in 1..ranges.len() {
        let boundary = ranges[index].0;
        let left = (boundary - ranges[index - 1].0).max(0.0);
        let right = (ranges[index].1 - boundary).max(0.0);
        let window = 2.0_f64.min(left * 0.5).min(right * 0.5);
        boundaries.push(snap_to_silence(pcm, boundary, window));
    }
    boundaries
}

/// 境界から±window秒の範囲で最も音量の小さいフレーム中心を返す。
fn snap_to_silence(pcm: &[f32], boundary: f64, window: f64) -> f64 {
    if window <= 0.0 || pcm.is_empty() {
        return boundary;
    }
    let frame = SAMPLE_RATE / 20; // 50ms
    let half = (window * SAMPLE_RATE as f64) as usize;
    let center = ((boundary * SAMPLE_RATE as f64) as usize).min(pcm.len());
    let low = center.saturating_sub(half);
    let high = (center + half).min(pcm.len());
    if high <= low + frame {
        return boundary;
    }
    let mut best_energy = f64::MAX;
    let mut best_distance = usize::MAX;
    let mut best_center = center;
    let mut offset = low;
    while offset + frame <= high {
        let energy: f64 = pcm[offset..offset + frame]
            .iter()
            .map(|sample| (*sample as f64) * (*sample as f64))
            .sum();
        let frame_center = offset + frame / 2;
        let distance = frame_center.abs_diff(center);
        if energy < best_energy || (energy <= best_energy && distance < best_distance) {
            best_energy = energy;
            best_distance = distance;
            best_center = frame_center;
        }
        offset += frame;
    }
    best_center as f64 / SAMPLE_RATE as f64
}

fn create_contexts(
    model_path: &Path,
    use_gpu: bool,
    workers: usize,
    ctx: &PipelineContext,
) -> Result<Vec<WhisperContext>> {
    if use_gpu {
        let mut contexts = Vec::with_capacity(workers);
        let mut failed = None;
        for _ in 0..workers {
            let mut params = WhisperContextParameters::new();
            params.use_gpu(true).flash_attn(false);
            match WhisperContext::new_with_params(model_path, params) {
                Ok(context) => contexts.push(context),
                Err(error) => {
                    failed = Some(error);
                    break;
                }
            }
        }
        if failed.is_none() {
            return Ok(contexts);
        }
        if let Some(error) = failed {
            ctx.status(format!("GPU初期化に失敗したためCPUへ切り替えます: {error}"));
        }
    }
    let mut params = WhisperContextParameters::new();
    params.use_gpu(false).flash_attn(false);
    let context = WhisperContext::new_with_params(model_path, params)
        .context("CPUでの音声認識モデル読み込みに失敗しました")?;
    Ok(vec![context])
}

#[allow(clippy::too_many_arguments)]
fn run_worker(
    context: WhisperContext,
    jobs: Arc<Vec<ChunkJob>>,
    pcm: Arc<Vec<f32>>,
    queue: Arc<Mutex<VecDeque<usize>>>,
    tracker: Arc<Mutex<ProgressTracker>>,
    abort: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    sender: mpsc::Sender<Result<SlideTranscript, String>>,
    prompt: String,
) {
    let flags = AbortFlags {
        cancel: cancel.clone(),
        abort,
    };
    loop {
        if flags.cancel.load(Ordering::Relaxed) || flags.abort.load(Ordering::Relaxed) {
            break;
        }
        let Some(job_index) = queue
            .lock()
            .map(|mut queue| queue.pop_front())
            .unwrap_or(None)
        else {
            break;
        };
        let job = &jobs[job_index];
        if let Ok(mut tracker) = tracker.lock() {
            tracker.start(job.slide_index);
        }
        let outcome = transcribe_chunk(&context, job, &pcm, &tracker, &flags, &prompt);
        if let Ok(mut tracker) = tracker.lock() {
            tracker.finish(job.slide_index);
        }
        match outcome {
            Ok(segments) => {
                let _ = sender.send(Ok(SlideTranscript {
                    index: job.slide_index,
                    segments,
                }));
            }
            Err(message) => {
                let _ = sender.send(Err(message));
                flags.abort.store(true, Ordering::Relaxed);
                break;
            }
        }
    }
}

fn transcribe_chunk(
    context: &WhisperContext,
    job: &ChunkJob,
    pcm: &[f32],
    tracker: &Arc<Mutex<ProgressTracker>>,
    flags: &AbortFlags,
    prompt: &str,
) -> Result<Vec<TranscriptSegment>, String> {
    let mut state = context.create_state().map_err(|error| error.to_string())?;
    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: 5,
        patience: -1.0,
    });
    params.set_n_threads(thread_count());
    params.set_language(Some("ja"));
    params.set_no_context(true);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    if !prompt.is_empty() {
        params.set_initial_prompt(prompt);
    }
    let slide_index = job.slide_index;
    let progress_tracker = tracker.clone();
    params.set_progress_callback_safe(move |percent: i32| {
        if let Ok(mut tracker) = progress_tracker.lock() {
            tracker.update(slide_index, percent as f64 / 100.0);
        }
    });
    // whisper-rs 0.16.0 の安全版 abort callback は参照渡しが壊れているため、
    // 有効期間中のみ参照する自前の C trampoline を使用する。
    unsafe {
        params.set_abort_callback(Some(abort_trampoline));
        params.set_abort_callback_user_data(flags as *const AbortFlags as *mut std::ffi::c_void);
    }
    let slice = &pcm[job.start_sample..job.end_sample];
    let padded;
    let samples = if slice.len() < SAMPLE_RATE {
        padded = {
            let mut samples = slice.to_vec();
            samples.resize(SAMPLE_RATE, 0.0);
            samples
        };
        padded.as_slice()
    } else {
        slice
    };
    if let Err(error) = state.full(params, samples) {
        if flags.cancel.load(Ordering::Relaxed) {
            return Err("処理はキャンセルされました".to_string());
        }
        return Err(error.to_string());
    }
    if flags.cancel.load(Ordering::Relaxed) {
        return Err("処理はキャンセルされました".to_string());
    }
    let mut segments = Vec::new();
    for segment in state.as_iter() {
        let text =
            crate::project::normalize_transcript(&segment.to_str_lossy().unwrap_or_default());
        if text.is_empty() {
            continue;
        }
        let start = (segment.start_timestamp() as f64 / 10.0 + job.start_time)
            .clamp(job.start_time, job.end_time);
        let end =
            (segment.end_timestamp() as f64 / 10.0 + job.start_time).clamp(start, job.end_time);
        segments.push(TranscriptSegment { start, end, text });
    }
    Ok(segments)
}

fn thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|count| count.get().clamp(1, 8) as i32)
        .unwrap_or(8)
}

unsafe extern "C" fn abort_trampoline(user_data: *mut std::ffi::c_void) -> bool {
    let flags = unsafe { &*(user_data as *const AbortFlags) };
    flags.cancel.load(Ordering::Relaxed) || flags.abort.load(Ordering::Relaxed)
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
