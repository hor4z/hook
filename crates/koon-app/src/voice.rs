use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const MODELS: [(&str, &str); 2] = [("small", "ggml-small-q5_1.bin"), ("turbo", "ggml-large-v3-turbo-q5_0.bin")];
const MODEL_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";
const RATE: usize = 16_000;
const PROMPT_ES: &str = "Comentario para Claude sobre la interfaz de koon: el botón, el header, el dock, el texto y los colores. Lo subo a GitHub.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Listening,
    Downloading,
    Transcribing,
}

struct Shared {
    samples: Mutex<Vec<f32>>,
    rate: AtomicU32,
    level: AtomicU32,
    peak: AtomicU32,
    phase: AtomicU8,
    stop: AtomicBool,
    cancel: AtomicBool,
}

pub struct Take {
    pub id: u64,
    shared: Arc<Shared>,
}

static NEXT: AtomicU64 = AtomicU64::new(1);
static CONTEXT: Mutex<Option<(PathBuf, WhisperContext)>> = Mutex::new(None);

struct Settings {
    lang: String,
    prompt: String,
    model: PathBuf,
    server: Option<String>,
}

fn settings(root: &std::path::Path) -> Settings {
    let prefs = koon_core::Store::at(root).prefs();
    let env = |k: &str| std::env::var(k).ok();
    let lang = env("KOON_VOICE_LANG").or(prefs.voice_lang).unwrap_or_else(|| "es".into());
    let base = if lang == "es" { PROMPT_ES } else { "" };
    let prompt = env("KOON_VOICE_PROMPT").unwrap_or_else(|| format!("{base} {}", prefs.voice_context.unwrap_or_default()).trim().to_string());
    let name = env("KOON_WHISPER_MODEL").or(prefs.voice_model).unwrap_or_else(|| "small".into());
    let model = match MODELS.iter().find(|(k, _)| *k == name) {
        Some((_, file)) => root.join("models").join(file),
        None => PathBuf::from(name),
    };
    let server = env("KOON_VOICE_SERVER").or(prefs.voice_server).filter(|u| !u.is_empty());
    Settings { lang, prompt, model, server }
}

impl Take {
    pub fn start(root: PathBuf, done: impl FnOnce(u64, Result<String, String>) + Send + 'static) -> Take {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let shared = Arc::new(Shared {
            samples: Mutex::new(Vec::new()),
            rate: AtomicU32::new(0),
            level: AtomicU32::new(0),
            peak: AtomicU32::new(0),
            phase: AtomicU8::new(0),
            stop: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        });
        let warm = root.clone();
        std::thread::spawn(move || {
            if settings(&warm).server.is_none() {
                let _ = context(&warm, None);
            }
        });
        let s = shared.clone();
        std::thread::spawn(move || {
            let res = run(&s, &root);
            if !s.cancel.load(Ordering::Relaxed) {
                done(id, res);
            }
        });
        Take { id, shared }
    }

    pub fn level(&self) -> f32 {
        f32::from_bits(self.shared.level.load(Ordering::Relaxed))
    }

    pub fn phase(&self) -> Phase {
        match self.shared.phase.load(Ordering::Relaxed) {
            1 => Phase::Downloading,
            2 => Phase::Transcribing,
            _ => Phase::Listening,
        }
    }

    pub fn listening(&self) -> bool {
        !self.shared.stop.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Take {
    fn drop(&mut self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

fn run(s: &Arc<Shared>, root: &std::path::Path) -> Result<String, String> {
    record(s)?;
    if s.cancel.load(Ordering::Relaxed) {
        return Ok(String::new());
    }
    let rate = s.rate.load(Ordering::Relaxed) as usize;
    let raw = std::mem::take(&mut *s.samples.lock().map_err(|e| e.to_string())?);
    if f32::from_bits(s.peak.load(Ordering::Relaxed)) < 0.005 || raw.len() < rate / 4 {
        return Ok(String::new());
    }
    let pcm = resample(&raw, rate);
    if settings(root).server.is_none() {
        context(root, Some(s))?;
    }
    s.phase.store(2, Ordering::Relaxed);
    transcribe(&pcm, root)
}

fn decode(path: &std::ffi::OsStr) -> Result<Vec<f32>, String> {
    let out = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-ac", "1", "-ar", "16000", "-f", "f32le", "-"])
        .output()
        .map_err(|e| format!("cannot run ffmpeg: {e}"))?;
    Ok(out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}

fn record(s: &Arc<Shared>) -> Result<(), String> {
    if let Some(file) = std::env::var_os("KOON_VOICE_FILE") {
        let pcm = decode(&file)?;
        s.rate.store(RATE as u32, Ordering::Relaxed);
        s.peak.store(pcm.iter().fold(0f32, |a, x| a.max(x.abs())).to_bits(), Ordering::Relaxed);
        s.level.store(0.6f32.to_bits(), Ordering::Relaxed);
        s.samples.lock().map_err(|e| e.to_string())?.extend(pcm);
        while !s.stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(15));
        }
        return Ok(());
    }
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("no hay micrófono")?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let channels = config.channels() as usize;
    s.rate.store(config.sample_rate(), Ordering::Relaxed);
    let err = |e: cpal::Error| eprintln!("koon: microphone: {e}");
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(config.into(), feed::<f32>(s.clone(), channels), err, None),
        SampleFormat::I16 => device.build_input_stream(config.into(), feed::<i16>(s.clone(), channels), err, None),
        SampleFormat::I32 => device.build_input_stream(config.into(), feed::<i32>(s.clone(), channels), err, None),
        SampleFormat::U16 => device.build_input_stream(config.into(), feed::<u16>(s.clone(), channels), err, None),
        SampleFormat::U8 => device.build_input_stream(config.into(), feed::<u8>(s.clone(), channels), err, None),
        f => return Err(format!("unsupported sample format {f}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    while !s.stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(15));
    }
    drop(stream);
    s.level.store(0f32.to_bits(), Ordering::Relaxed);
    Ok(())
}

fn feed<T>(s: Arc<Shared>, channels: usize) -> impl FnMut(&[T], &cpal::InputCallbackInfo) + Send + 'static
where
    T: Sample + Send + 'static,
    f32: FromSample<T>,
{
    move |data: &[T], _| {
        let mono: Vec<f32> = data.chunks(channels.max(1)).map(|c| c.iter().map(|x| f32::from_sample(*x)).sum::<f32>() / c.len() as f32).collect();
        if mono.is_empty() {
            return;
        }
        let rms = (mono.iter().map(|x| x * x).sum::<f32>() / mono.len() as f32).sqrt();
        let level = (rms * 9.0).sqrt().clamp(0.0, 1.0);
        s.level.store(level.to_bits(), Ordering::Relaxed);
        let peak = mono.iter().fold(0f32, |a, x| a.max(x.abs()));
        if peak > f32::from_bits(s.peak.load(Ordering::Relaxed)) {
            s.peak.store(peak.to_bits(), Ordering::Relaxed);
        }
        if let Ok(mut v) = s.samples.lock() {
            v.extend(mono);
        }
    }
}

fn resample(raw: &[f32], rate: usize) -> Vec<f32> {
    if rate == RATE || rate == 0 {
        return raw.to_vec();
    }
    let step = rate as f64 / RATE as f64;
    let n = (raw.len() as f64 / step) as usize;
    (0..n)
        .map(|i| {
            let a = (i as f64 * step) as usize;
            let b = (((i + 1) as f64 * step) as usize).clamp(a + 1, raw.len());
            raw[a..b].iter().sum::<f32>() / (b - a) as f32
        })
        .collect()
}

fn download(path: &std::path::Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = path.with_extension("part");
    eprintln!("koon: downloading the voice model to {}", path.display());
    let ok = std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&part)
        .arg(format!("{MODEL_URL}{}", path.file_name().and_then(|n| n.to_str()).unwrap_or_default()))
        .status()
        .map_err(|e| format!("cannot run curl: {e}"))?
        .success();
    if !ok {
        let _ = std::fs::remove_file(&part);
        return Err("no pude descargar el modelo de voz".into());
    }
    std::fs::rename(&part, path).map_err(|e| e.to_string())
}

fn context(root: &std::path::Path, s: Option<&Arc<Shared>>) -> Result<(), String> {
    let mut ctx = CONTEXT.lock().map_err(|e| e.to_string())?;
    let path = settings(root).model;
    if ctx.as_ref().is_some_and(|(p, _)| *p == path) {
        return Ok(());
    }
    if !path.exists() {
        if let Some(s) = s {
            s.phase.store(1, Ordering::Relaxed);
        }
        download(&path)?;
    }
    whisper_rs::install_logging_hooks();
    let c = WhisperContext::new_with_params(&path, WhisperContextParameters::default()).map_err(|e| format!("cannot load the voice model: {e}"))?;
    *ctx = Some((path, c));
    Ok(())
}

fn transcribe(pcm: &[f32], root: &std::path::Path) -> Result<String, String> {
    let mut pcm = pcm.to_vec();
    if pcm.len() < RATE + RATE / 4 {
        pcm.resize(RATE + RATE / 4, 0.0);
    }
    let pcm = pcm.as_slice();
    let set = settings(root);
    if let Some(url) = set.server.as_deref() {
        return remote(url, pcm, &set).map(|t| clean(&t));
    }
    let guard = CONTEXT.lock().map_err(|e| e.to_string())?;
    let (_, ctx) = guard.as_ref().ok_or("voice model not loaded")?;
    let mut state = ctx.create_state().map_err(|e| e.to_string())?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_language(Some(&set.lang));
    if !set.prompt.is_empty() {
        p.set_initial_prompt(&set.prompt);
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    p.set_n_threads(threads as i32);
    if std::env::var("KOON_VOICE_AUDIO_CTX").is_ok_and(|v| v == "full") {
        p.set_audio_ctx(0);
    } else {
        let secs = pcm.len() as f32 / RATE as f32 + 1.0;
        p.set_audio_ctx(((secs * 50.0).ceil() as i32).min(1500));
    }
    p.set_no_context(true);
    p.set_suppress_blank(true);
    p.set_print_special(false);
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_timestamps(false);
    state.full(p, pcm).map_err(|e| e.to_string())?;
    let mut text = String::new();
    for seg in state.as_iter() {
        if seg.no_speech_probability() > 0.8 {
            continue;
        }
        if let Ok(t) = seg.to_str_lossy() {
            text.push_str(&t);
        }
    }
    Ok(clean(&text))
}

fn remote(url: &str, pcm: &[f32], set: &Settings) -> Result<String, String> {
    let file = std::env::temp_dir().join(format!("koon-voice-{}.wav", std::process::id()));
    std::fs::write(&file, wav(pcm)).map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-fsS", "--max-time", "60", "-F"])
        .arg(format!("file=@{}", file.display()))
        .args(["-F", "response_format=json", "-F", "model=whisper-1", "-F"])
        .arg(format!("language={}", set.lang));
    if !set.prompt.is_empty() {
        cmd.arg("-F").arg(format!("prompt={}", set.prompt));
    }
    if let Ok(token) = std::env::var("KOON_VOICE_TOKEN") {
        cmd.arg("-H").arg(format!("Authorization: Bearer {token}"));
    }
    let out = cmd.arg(url).output().map_err(|e| format!("cannot run curl: {e}"));
    let _ = std::fs::remove_file(&file);
    let out = out?;
    if !out.status.success() {
        return Err(format!("el servidor de voz respondió con error: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("respuesta inválida del servidor de voz: {e}"))?;
    Ok(v["text"].as_str().unwrap_or_default().to_string())
}

fn wav(pcm: &[f32]) -> Vec<u8> {
    let mut wav = Vec::with_capacity(44 + pcm.len() * 2);
    let data = (pcm.len() * 2) as u32;
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&(RATE as u32).to_le_bytes());
    wav.extend_from_slice(&(RATE as u32 * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    for x in pcm {
        wav.extend_from_slice(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    wav
}

fn clean(text: &str) -> String {
    let mut t = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => t.push(c),
            _ => {}
        }
    }
    let mut t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let junk = [
        "Subtítulos realizados por la comunidad de Amara.org",
        "¡Gracias por ver!",
        "¡Suscríbete al canal!",
        "¡Suscríbete!",
        "Suscríbete al canal",
        "Gracias por ver el video.",
    ];
    for j in junk {
        t = t.replace(j, "");
    }
    t.trim().to_string()
}

pub fn record_file(secs: u64, path: &str) -> Result<(), String> {
    let s = Arc::new(Shared {
        samples: Mutex::new(Vec::new()),
        rate: AtomicU32::new(0),
        level: AtomicU32::new(0),
        peak: AtomicU32::new(0),
        phase: AtomicU8::new(0),
        stop: AtomicBool::new(false),
        cancel: AtomicBool::new(false),
    });
    let timer = s.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(secs));
        timer.stop.store(true, Ordering::Relaxed);
    });
    println!("recording {secs}s, speak now");
    record(&s)?;
    let raw = std::mem::take(&mut *s.samples.lock().map_err(|e| e.to_string())?);
    let pcm = resample(&raw, s.rate.load(Ordering::Relaxed) as usize);
    std::fs::write(path, wav(&pcm)).map_err(|e| e.to_string())?;
    println!("saved {path} (peak {:.3})", f32::from_bits(s.peak.load(Ordering::Relaxed)));
    Ok(())
}

pub fn transcribe_file(path: &str) -> Result<(), String> {
    let root = koon_core::Store::open().root().to_path_buf();
    let pcm = decode(path.as_ref())?;
    let started = std::time::Instant::now();
    if settings(&root).server.is_none() {
        context(&root, None)?;
    }
    let loaded = started.elapsed().as_secs_f32();
    let text = transcribe(&pcm, &root)?;
    println!(
        "{:.1}s audio, model {loaded:.1}s, total {:.1}s: {text}",
        pcm.len() as f32 / RATE as f32,
        started.elapsed().as_secs_f32()
    );
    Ok(())
}

pub fn listen(secs: u64) -> Result<(), String> {
    let root = koon_core::Store::open().root().to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    let take = Take::start(root, move |_, r| {
        let _ = tx.send(r);
    });
    println!("listening for {secs}s, speak now");
    let until = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < until {
        let l = take.level();
        print!("\r{:<40}", "#".repeat((l * 40.0) as usize));
        use std::io::Write;
        let _ = std::io::stdout().flush();
        std::thread::sleep(Duration::from_millis(50));
    }
    println!();
    take.stop();
    let started = std::time::Instant::now();
    let mut last = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(r) => {
                println!("heard in {:.1}s: {:?}", started.elapsed().as_secs_f32(), r);
                return r.map(|_| ());
            }
            Err(_) => {
                let p = take.phase();
                if last != Some(p) {
                    println!("{p:?}");
                    last = Some(p);
                }
            }
        }
    }
}
