use std::{
    error::Error,
    ffi::{CStr, CString},
    fs::File,
    io::{Read, Seek, SeekFrom},
    os::raw::{c_char, c_int, c_void},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use crate::config::SoundConfig;

pub const HELPER_ENV: &str = "PIGEON_SOUND_HELPER";

const MAX_PULSE_WAV_BYTES: u64 = 8 * 1024 * 1024;
const MAX_WAV_FMT_CHUNK_BYTES: u32 = 256;
const SOUND_THREAD_STACK_BYTES: usize = 256 * 1024;

#[derive(Default)]
pub struct SoundPlayer {
    last_played: Mutex<Option<Instant>>,
}

impl SoundPlayer {
    pub fn play(&self, config: &SoundConfig) {
        if !config.enabled {
            return;
        }

        if config.file.as_os_str().is_empty() {
            tracing::warn!("sound is enabled but no sound.file is configured");
            return;
        }

        let path = config.file.clone();
        if self.is_on_cooldown(config.cooldown) {
            return;
        }

        let volume = config.volume;
        if let Err(error) = thread::Builder::new()
            .name("pigeon-sound".into())
            .stack_size(SOUND_THREAD_STACK_BYTES)
            .spawn(move || {
                if let Err(error) = play_with_helper(&path, volume) {
                    tracing::warn!(
                        %error,
                        path = %path.display(),
                        "failed to play notification sound"
                    );
                }
                crate::memory::trim_free_heap_pages();
            })
        {
            tracing::warn!(%error, "failed to spawn notification sound thread");
        }
    }

    fn is_on_cooldown(&self, cooldown: u64) -> bool {
        let cooldown = Duration::from_millis(cooldown);
        let now = Instant::now();
        let mut last_played = self.last_played.lock().expect("sound lock poisoned");

        if last_played.is_some_and(|last| now.duration_since(last) < cooldown) {
            return true;
        }

        *last_played = Some(now);
        false
    }
}

pub fn run_helper<I>(mut args: I) -> Result<(), Box<dyn Error + Send + Sync>>
where
    I: Iterator<Item = std::ffi::OsString>,
{
    let path = args
        .next()
        .map(PathBuf::from)
        .ok_or("sound helper requires a path argument")?;
    let volume = args
        .next()
        .ok_or("sound helper requires a volume argument")?
        .to_string_lossy()
        .parse::<f32>()?;

    play_file(&path, volume)
}

fn play_with_helper(path: &Path, volume: f32) -> Result<(), Box<dyn Error + Send + Sync>> {
    let output = Command::new(std::env::current_exe()?)
        .env(HELPER_ENV, "1")
        .arg(path)
        .arg(volume.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "sound helper exited with {}: {}",
        output.status,
        stderr.trim()
    )
    .into())
}

fn play_file(path: &Path, volume: f32) -> Result<(), Box<dyn Error + Send + Sync>> {
    play_wav(path, volume)
}

fn play_wav(path: &Path, volume: f32) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut file = File::open(path)?;
    let mut wav = read_pcm_s16le_wav(&mut file)?
        .ok_or("sound.file must be a PCM s16le WAV when native sound is enabled")?;
    drop(file);

    if volume != 1.0 {
        scale_s16le(&mut wav.data, volume);
    }

    match play_wav_with_pulse(&wav) {
        Ok(()) => Ok(()),
        Err(pulse_error) => match play_wav_with_alsa(&wav) {
            Ok(()) => Ok(()),
            Err(alsa_error) => Err(format!(
                "pulse playback failed: {pulse_error}; alsa playback failed: {alsa_error}"
            )
            .into()),
        },
    }
}

fn play_wav_with_pulse(wav: &WavData) -> Result<(), Box<dyn Error + Send + Sync>> {
    let app_name = CString::new("pigeon")?;
    let stream_name = CString::new("notification")?;
    let sample_spec = PaSampleSpec {
        format: PA_SAMPLE_S16LE,
        rate: wav.sample_rate,
        channels: wav.channels,
    };

    let pulse_api = PulseApi::open()?;
    let mut error = 0;
    let pulse = unsafe {
        (pulse_api.pa_simple_new)(
            std::ptr::null(),
            app_name.as_ptr(),
            PA_STREAM_PLAYBACK,
            std::ptr::null(),
            stream_name.as_ptr(),
            &sample_spec,
            std::ptr::null(),
            std::ptr::null(),
            &mut error,
        )
    };
    if pulse.is_null() {
        return Err(format!("pa_simple_new failed: {}", pulse_api.error(error)).into());
    }

    let result = write_pulse_stream(&pulse_api, pulse, &wav.data);
    unsafe {
        (pulse_api.pa_simple_free)(pulse);
    }
    result?;

    Ok(())
}

fn play_wav_with_alsa(wav: &WavData) -> Result<(), Box<dyn Error + Send + Sync>> {
    let alsa_api = AlsaApi::open()?;
    let default_device = CString::new("default")?;
    let mut pcm = std::ptr::null_mut();

    let open_result = unsafe {
        (alsa_api.snd_pcm_open)(
            &mut pcm,
            default_device.as_ptr(),
            SND_PCM_STREAM_PLAYBACK,
            0,
        )
    };
    if open_result < 0 {
        return Err(format!("snd_pcm_open failed: {}", alsa_api.error(open_result)).into());
    }

    let result = write_alsa_stream(&alsa_api, pcm, wav);
    unsafe {
        (alsa_api.snd_pcm_close)(pcm);
    }
    result
}

fn write_alsa_stream(
    alsa_api: &AlsaApi,
    pcm: *mut c_void,
    wav: &WavData,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let set_params_result = unsafe {
        (alsa_api.snd_pcm_set_params)(
            pcm,
            SND_PCM_FORMAT_S16_LE,
            SND_PCM_ACCESS_RW_INTERLEAVED,
            u32::from(wav.channels),
            wav.sample_rate,
            1,
            ALSA_LATENCY_US,
        )
    };
    if set_params_result < 0 {
        return Err(format!(
            "snd_pcm_set_params failed: {}",
            alsa_api.error(set_params_result)
        )
        .into());
    }

    let frame_bytes = usize::from(wav.channels) * 2;
    if frame_bytes == 0 || wav.data.len() % frame_bytes != 0 {
        return Err("invalid PCM frame layout".into());
    }

    let mut frame_offset = 0;
    let frame_count = wav.data.len() / frame_bytes;
    while frame_offset < frame_count {
        let byte_offset = frame_offset * frame_bytes;
        let remaining_frames = frame_count - frame_offset;
        let write_result = unsafe {
            (alsa_api.snd_pcm_writei)(
                pcm,
                wav.data[byte_offset..].as_ptr().cast::<c_void>(),
                remaining_frames,
            )
        };

        if write_result < 0 {
            let recovered = unsafe { (alsa_api.snd_pcm_recover)(pcm, write_result as c_int, 1) };
            if recovered < 0 {
                return Err(format!("snd_pcm_writei failed: {}", alsa_api.error(recovered)).into());
            }
            continue;
        }

        if write_result == 0 {
            return Err("snd_pcm_writei wrote zero frames".into());
        }

        frame_offset += write_result as usize;
    }

    let drain_result = unsafe { (alsa_api.snd_pcm_drain)(pcm) };
    if drain_result < 0 {
        return Err(format!("snd_pcm_drain failed: {}", alsa_api.error(drain_result)).into());
    }

    Ok(())
}

fn write_pulse_stream(
    pulse_api: &PulseApi,
    pulse: *mut c_void,
    data: &[u8],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut error = 0;
    let write_result = unsafe {
        (pulse_api.pa_simple_write)(
            pulse,
            data.as_ptr().cast::<c_void>(),
            data.len(),
            &mut error,
        )
    };
    if write_result < 0 {
        return Err(format!("pa_simple_write failed: {}", pulse_api.error(error)).into());
    }

    let drain_result = unsafe { (pulse_api.pa_simple_drain)(pulse, &mut error) };
    if drain_result < 0 {
        return Err(format!("pa_simple_drain failed: {}", pulse_api.error(error)).into());
    }

    Ok(())
}

struct WavData {
    sample_rate: u32,
    channels: u8,
    data: Vec<u8>,
}

fn read_pcm_s16le_wav(
    reader: &mut (impl Read + Seek),
) -> Result<Option<WavData>, Box<dyn Error + Send + Sync>> {
    let mut header = [0; 12];
    reader.read_exact(&mut header)?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Ok(None);
    }

    let mut format = None;
    let mut data = None;

    loop {
        let mut chunk_header = [0; 8];
        match reader.read_exact(&mut chunk_header) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }

        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]);

        match chunk_id {
            b"fmt " => {
                if chunk_size < 16 || chunk_size > MAX_WAV_FMT_CHUNK_BYTES {
                    return Ok(None);
                }
                let mut chunk = vec![0; chunk_size as usize];
                reader.read_exact(&mut chunk)?;

                let audio_format = u16::from_le_bytes([chunk[0], chunk[1]]);
                let channels = u16::from_le_bytes([chunk[2], chunk[3]]);
                let sample_rate = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                let bits_per_sample = u16::from_le_bytes([chunk[14], chunk[15]]);
                if audio_format != 1 || bits_per_sample != 16 || channels == 0 || channels > 255 {
                    return Ok(None);
                }

                format = Some((sample_rate, channels as u8));
            }
            b"data" if chunk_size as u64 <= MAX_PULSE_WAV_BYTES => {
                let mut chunk = vec![0; chunk_size as usize];
                reader.read_exact(&mut chunk)?;
                data = Some(chunk);
            }
            b"data" => return Ok(None),
            _ => {
                reader.seek(SeekFrom::Current(i64::from(chunk_size)))?;
            }
        };

        if chunk_size % 2 == 1 {
            reader.seek(SeekFrom::Current(1))?;
        }
    }

    let Some((sample_rate, channels)) = format else {
        return Ok(None);
    };
    let Some(data) = data else {
        return Ok(None);
    };

    Ok(Some(WavData {
        sample_rate,
        channels,
        data,
    }))
}

fn scale_s16le(data: &mut [u8], volume: f32) {
    for sample in data.chunks_exact_mut(2) {
        let value = i16::from_le_bytes([sample[0], sample[1]]);
        let scaled = (f32::from(value) * volume)
            .round()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        sample.copy_from_slice(&scaled.to_le_bytes());
    }
}

struct PulseApi {
    simple_handle: *mut c_void,
    pulse_handle: *mut c_void,
    pa_simple_new: PaSimpleNew,
    pa_simple_write: PaSimpleWrite,
    pa_simple_drain: PaSimpleDrain,
    pa_simple_free: PaSimpleFree,
    pa_strerror: PaStrerror,
}

impl PulseApi {
    fn open() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let simple_handle = open_library("libpulse-simple.so.0")?;
        let pulse_handle = match open_library("libpulse.so.0") {
            Ok(handle) => handle,
            Err(error) => {
                close_library(simple_handle);
                return Err(error);
            }
        };

        let api = unsafe {
            Self {
                simple_handle,
                pulse_handle,
                pa_simple_new: load_symbol(simple_handle, "pa_simple_new")?,
                pa_simple_write: load_symbol(simple_handle, "pa_simple_write")?,
                pa_simple_drain: load_symbol(simple_handle, "pa_simple_drain")?,
                pa_simple_free: load_symbol(simple_handle, "pa_simple_free")?,
                pa_strerror: load_symbol(pulse_handle, "pa_strerror")?,
            }
        };

        Ok(api)
    }

    fn error(&self, error: c_int) -> String {
        let message = unsafe { (self.pa_strerror)(error) };
        if message.is_null() {
            format!("PulseAudio error {error}")
        } else {
            unsafe { CStr::from_ptr(message).to_string_lossy().into_owned() }
        }
    }
}

impl Drop for PulseApi {
    fn drop(&mut self) {
        close_library(self.pulse_handle);
        close_library(self.simple_handle);
    }
}

struct AlsaApi {
    handle: *mut c_void,
    snd_pcm_open: SndPcmOpen,
    snd_pcm_set_params: SndPcmSetParams,
    snd_pcm_writei: SndPcmWritei,
    snd_pcm_recover: SndPcmRecover,
    snd_pcm_drain: SndPcmDrain,
    snd_pcm_close: SndPcmClose,
    snd_strerror: SndStrerror,
}

impl AlsaApi {
    fn open() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let handle = open_library("libasound.so.2")?;

        let api = unsafe {
            Self {
                handle,
                snd_pcm_open: load_symbol(handle, "snd_pcm_open")?,
                snd_pcm_set_params: load_symbol(handle, "snd_pcm_set_params")?,
                snd_pcm_writei: load_symbol(handle, "snd_pcm_writei")?,
                snd_pcm_recover: load_symbol(handle, "snd_pcm_recover")?,
                snd_pcm_drain: load_symbol(handle, "snd_pcm_drain")?,
                snd_pcm_close: load_symbol(handle, "snd_pcm_close")?,
                snd_strerror: load_symbol(handle, "snd_strerror")?,
            }
        };

        Ok(api)
    }

    fn error(&self, error: c_int) -> String {
        let message = unsafe { (self.snd_strerror)(error) };
        if message.is_null() {
            format!("ALSA error {error}")
        } else {
            unsafe { CStr::from_ptr(message).to_string_lossy().into_owned() }
        }
    }
}

impl Drop for AlsaApi {
    fn drop(&mut self) {
        close_library(self.handle);
    }
}

const PA_STREAM_PLAYBACK: c_int = 1;
const PA_SAMPLE_S16LE: c_int = 3;
const SND_PCM_STREAM_PLAYBACK: c_int = 0;
const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;
const SND_PCM_FORMAT_S16_LE: c_int = 2;
const ALSA_LATENCY_US: u32 = 100_000;

#[repr(C)]
struct PaSampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}

type PaSimpleNew = unsafe extern "C" fn(
    server: *const c_char,
    name: *const c_char,
    dir: c_int,
    dev: *const c_char,
    stream_name: *const c_char,
    ss: *const PaSampleSpec,
    map: *const c_void,
    attr: *const c_void,
    error: *mut c_int,
) -> *mut c_void;
type PaSimpleWrite = unsafe extern "C" fn(
    s: *mut c_void,
    data: *const c_void,
    bytes: usize,
    error: *mut c_int,
) -> c_int;
type PaSimpleDrain = unsafe extern "C" fn(s: *mut c_void, error: *mut c_int) -> c_int;
type PaSimpleFree = unsafe extern "C" fn(s: *mut c_void);
type PaStrerror = unsafe extern "C" fn(error: c_int) -> *const c_char;
type SndPcmOpen = unsafe extern "C" fn(
    pcmp: *mut *mut c_void,
    name: *const c_char,
    stream: c_int,
    mode: c_int,
) -> c_int;
type SndPcmSetParams = unsafe extern "C" fn(
    pcm: *mut c_void,
    format: c_int,
    access: c_int,
    channels: u32,
    rate: u32,
    soft_resample: c_int,
    latency: u32,
) -> c_int;
type SndPcmWritei =
    unsafe extern "C" fn(pcm: *mut c_void, buffer: *const c_void, size: usize) -> isize;
type SndPcmRecover = unsafe extern "C" fn(pcm: *mut c_void, err: c_int, silent: c_int) -> c_int;
type SndPcmDrain = unsafe extern "C" fn(pcm: *mut c_void) -> c_int;
type SndPcmClose = unsafe extern "C" fn(pcm: *mut c_void) -> c_int;
type SndStrerror = unsafe extern "C" fn(errnum: c_int) -> *const c_char;

fn open_library(name: &str) -> Result<*mut c_void, Box<dyn Error + Send + Sync>> {
    let name = CString::new(name)?;
    let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
    if handle.is_null() {
        Err(dynamic_loader_error("dlopen failed").into())
    } else {
        Ok(handle)
    }
}

fn close_library(handle: *mut c_void) {
    if !handle.is_null() {
        unsafe {
            dlclose(handle);
        }
    }
}

unsafe fn load_symbol<T>(handle: *mut c_void, name: &str) -> Result<T, Box<dyn Error + Send + Sync>>
where
    T: Copy,
{
    let name = CString::new(name)?;
    let symbol = unsafe { dlsym(handle, name.as_ptr()) };
    if symbol.is_null() {
        Err(dynamic_loader_error("dlsym failed").into())
    } else {
        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&symbol) })
    }
}

fn dynamic_loader_error(context: &str) -> String {
    let error = unsafe { dlerror() };
    if error.is_null() {
        context.into()
    } else {
        format!("{context}: {}", unsafe {
            CStr::from_ptr(error).to_string_lossy()
        })
    }
}

const RTLD_NOW: c_int = 2;
const RTLD_LOCAL: c_int = 0;

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
    fn dlerror() -> *const c_char;
}
