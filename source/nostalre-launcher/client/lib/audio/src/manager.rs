use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::bgm_retry::BgmRetry;
use crate::decode::{self, Pcm};
use crate::mixer::MixerCore;
use crate::latest_worker::LatestWorker;
type DecodedBgm = (String, Result<Pcm, decode::DecodeError>);

const CACHE_EXPIRE: Duration = Duration::from_secs(15);
const SWEEP_INTERVAL: Duration = Duration::from_secs(1);

struct CacheEntry {
    pcm: Arc<Pcm>,
    last_used: Instant,
}

pub struct SoundManager {
    core: Option<Arc<Mutex<MixerCore>>>,
    _stream: Option<cpal::Stream>,
    cache: HashMap<String, CacheEntry>,
    cache_bytes: usize,
    failed: HashSet<String>,
    bgm_retry: BgmRetry,
    current_bgm: Option<String>,
    pending_bgm: Option<String>,
    bgm_worker: Option<LatestWorker<DecodedBgm>>,
    out_rate: u32,
    last_sweep: Instant,
    dropped_voices: u64,
    paused: bool,
}

impl SoundManager {
    pub fn new(bgm_volume: f32, sfx_volume: f32) -> Self {
        match init_stream(bgm_volume, sfx_volume) {
            Ok((core, stream, out_rate)) => Self {
                core: Some(core),
                _stream: Some(stream),
                cache: HashMap::new(),
                cache_bytes: 0,
                failed: HashSet::new(),
                bgm_retry: BgmRetry::default(),
                current_bgm: None,
                pending_bgm: None,
                bgm_worker: None,
                out_rate,
                last_sweep: Instant::now(),
                dropped_voices: 0,
                paused: false,
            },
            Err(e) => {
                tracing::warn!("audio disabled: {e}");
                Self::disabled()
            }
        }
    }

    pub fn disabled() -> Self {
        Self {
            core: None,
            _stream: None,
            cache: HashMap::new(),
            cache_bytes: 0,
            failed: HashSet::new(),
            bgm_retry: BgmRetry::default(),
            current_bgm: None,
            pending_bgm: None,
            bgm_worker: None,
            out_rate: 44100,
            last_sweep: Instant::now(),
            dropped_voices: 0,
            paused: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.core.is_some()
    }

    pub fn play_sfx(
        &mut self,
        key: &str,
        gain: f32,
        pan: f32,
        load: impl FnOnce() -> Option<Vec<u8>>,
    ) {
        let Some(core) = &self.core else { return };
        // A paused mixer never advances a voice cursor, so anything accepted here
        // would sit at frame 0 and burst out together on resume.
        if self.paused {
            return;
        }
        if self.failed.contains(key) {
            return;
        }
        let now = Instant::now();
        let pcm = if let Some(entry) = self.cache.get_mut(key) {
            entry.last_used = now;
            entry.pcm.clone()
        } else {
            let Some(bytes) = load() else {
                tracing::warn!("sound not found: {key}");
                self.failed.insert(key.to_string());
                return;
            };
            match decode::decode(&bytes, self.out_rate) {
                Ok(pcm) => {
                    let pcm = Arc::new(pcm);
                    self.cache_bytes += pcm.samples.len() * std::mem::size_of::<f32>();
                    self.cache.insert(
                        key.to_string(),
                        CacheEntry {
                            pcm: pcm.clone(),
                            last_used: now,
                        },
                    );
                    pcm
                }
                Err(e) => {
                    tracing::warn!("failed to decode sound {key}: {e}");
                    self.failed.insert(key.to_string());
                    return;
                }
            }
        };
        if !core.lock().unwrap().play(pcm, gain, pan) {
            self.dropped_voices += 1;
            tracing::trace!(
                "sfx voice pool full, dropped {key} ({} total)",
                self.dropped_voices
            );
        }
        #[cfg(target_os = "horizon")]
        self.trim_cache(16 * 1024 * 1024);
    }

    pub fn play_bgm(&mut self, key: &str, load: impl FnOnce() -> Option<Vec<u8>>) {
        let Some(core) = &self.core else { return };
        if self.current_bgm.as_deref() == Some(key) {
            return;
        }
        let Some(bytes) = load() else {
            tracing::warn!("bgm not found: {key}");
            self.bgm_retry.schedule(key, Instant::now());
            return;
        };
        match decode::decode(&bytes, self.out_rate) {
            Ok(pcm) => {
                core.lock().unwrap().play_bgm(Arc::new(pcm));
                self.current_bgm = Some(key.to_string());
                self.bgm_retry.clear();
            }
            Err(e) => {
                tracing::warn!("failed to decode bgm {key}: {e}");
                self.bgm_retry.schedule(key, Instant::now());
            }
        }
    }

    /// Read and decode on a single bounded worker. New tracks supersede queued
    /// tracks; results from a previous map or a stopped track cannot start later.
    pub fn play_bgm_async(&mut self, key: &str, load: impl FnOnce() -> Option<Vec<u8>> + Send + 'static) {
        if self.core.is_none() { return; }
        if self.current_bgm.as_deref() == Some(key) {
            if let Some(worker) = &self.bgm_worker { worker.cancel(); }
            self.pending_bgm = None;
            return;
        }
        if self.pending_bgm.as_deref() == Some(key) { return; }
        if self.bgm_worker.is_none() {
            match LatestWorker::new("nostalre music decoder") {
                Ok(worker) => self.bgm_worker = Some(worker),
                Err(error) => {
                    tracing::warn!("music worker unavailable: {error}");
                    self.bgm_retry.schedule(key, Instant::now()); return;
                }
            }
        }
        let out_rate = self.out_rate;
        let name = key.to_string();
        self.bgm_worker.as_ref().unwrap().request(move || {
            let result = load().ok_or_else(|| decode::DecodeError("music file not found".into()))
                .and_then(|bytes| decode::decode(&bytes, out_rate));
            (name, result)
        });
        self.pending_bgm = Some(key.to_string());
        self.bgm_retry.clear();
    }

    fn poll_bgm(&mut self) {
        let Some((name, result)) = self.bgm_worker.as_ref().and_then(|worker| worker.take_ready()) else { return; };
        if self.pending_bgm.as_deref() != Some(&name) { return; }
        self.pending_bgm = None;
        match result {
            Ok(pcm) => {
                if let Some(core) = &self.core { core.lock().unwrap().play_bgm(Arc::new(pcm)); }
                self.current_bgm = Some(name);
                self.bgm_retry.clear();
            }
            Err(error) => {
                tracing::warn!("failed to load bgm {name}: {error}");
                self.bgm_retry.schedule(&name, Instant::now());
            }
        }
    }

    /// The track to re-attempt, once its retry interval has elapsed.
    pub fn take_bgm_retry(&mut self) -> Option<String> {
        self.bgm_retry.take_due(Instant::now())
    }

    pub fn stop_bgm(&mut self) {
        if let Some(worker) = &self.bgm_worker { worker.cancel(); }
        self.pending_bgm = None;
        if let Some(core) = &self.core {
            core.lock().unwrap().stop_bgm();
        }
        self.current_bgm = None;
        self.bgm_retry.clear();
    }

    pub fn stop_all_sfx(&mut self) {
        if let Some(core) = &self.core {
            core.lock().unwrap().stop_all_sfx();
        }
    }

    pub fn set_stereo(&mut self, stereo: bool) {
        if let Some(core) = &self.core {
            core.lock().unwrap().set_stereo(stereo);
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if let Some(core) = &self.core {
            core.lock().unwrap().set_paused(paused);
        }
    }

    pub fn set_volumes(&mut self, bgm: f32, sfx: f32) {
        if let Some(core) = &self.core {
            core.lock().unwrap().set_masters(bgm, sfx);
        }
    }

    pub fn tick(&mut self) {
        self.poll_bgm();
        let now = Instant::now();
        if now.duration_since(self.last_sweep) >= SWEEP_INTERVAL {
            self.last_sweep = now;
            self.sweep(now);
        }
    }

    fn sweep(&mut self, now: Instant) {
        let bytes = &mut self.cache_bytes;
        self.cache.retain(|_, entry| {
            let keep = now.duration_since(entry.last_used) < CACHE_EXPIRE;
            if !keep { *bytes -= entry.pcm.samples.len() * std::mem::size_of::<f32>(); }
            keep
        });
    }

    fn trim_cache(&mut self, budget: usize) {
        while self.cache_bytes > budget {
            let Some(key) = self.cache.iter().min_by_key(|(_, entry)| entry.last_used).map(|(key, _)| key.clone()) else { break; };
            if let Some(entry) = self.cache.remove(&key) {
                self.cache_bytes -= entry.pcm.samples.len() * std::mem::size_of::<f32>();
            }
        }
    }
}

fn init_stream(
    bgm_volume: f32,
    sfx_volume: f32,
) -> Result<(Arc<Mutex<MixerCore>>, cpal::Stream, u32), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no output device".to_string())?;
    let default = device.default_output_config().map_err(|e| e.to_string())?;
    let config = if default.sample_format() == cpal::SampleFormat::F32 {
        default
    } else {
        device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .find(|c| c.sample_format() == cpal::SampleFormat::F32)
            .map(|c| c.with_max_sample_rate())
            .ok_or("no f32 output config".to_string())?
    };
    let out_rate = config.sample_rate();
    let out_channels = config.channels() as usize;
    let core = Arc::new(Mutex::new(MixerCore::new(out_rate, bgm_volume, sfx_volume)));
    let cb_core = core.clone();
    let stream = device
        .build_output_stream(
            &config.config(),
            move |data: &mut [f32], _| match cb_core.try_lock() {
                Ok(mut mixer) => mixer.render(data, out_channels),
                Err(_) => data.fill(0.0),
            },
            |e| tracing::warn!("audio stream error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((core, stream, out_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_sweep_expires_stale_entries_but_arc_stays_alive() {
        let mut m = SoundManager::disabled();
        let pcm = Arc::new(Pcm {
            samples: vec![0.0; 10],
            channels: 1,
        });
        let playing = pcm.clone();
        let now = Instant::now();
        m.cache.insert(
            "old.wav".into(),
            CacheEntry {
                pcm,
                last_used: now - Duration::from_secs(20),
            },
        );
        m.cache.insert(
            "fresh.wav".into(),
            CacheEntry {
                pcm: Arc::new(Pcm {
                    samples: vec![0.0; 10],
                    channels: 1,
                }),
                last_used: now,
            },
        );
        m.cache_bytes = 80;
        m.sweep(now);
        assert!(!m.cache.contains_key("old.wav"));
        assert!(m.cache.contains_key("fresh.wav"));
        assert_eq!(playing.frames(), 10);
    }

    /// A mixer-backed manager with no output stream.
    fn silent(out_rate: u32) -> SoundManager {
        let mut m = SoundManager::disabled();
        m.core = Some(Arc::new(Mutex::new(MixerCore::new(out_rate, 1.0, 1.0))));
        m.out_rate = out_rate;
        m
    }

    fn active_voices(m: &SoundManager) -> usize {
        m.core.as_ref().unwrap().lock().unwrap().active_sfx_voices()
    }

    #[test]
    fn sfx_requested_while_paused_is_discarded_not_deferred() {
        let mut m = silent(22050);
        let wav = || Some(crate::decode::wav_pcm16(22050, 1, 2205));

        m.set_paused(true);
        for _ in 0..5 {
            m.play_sfx("hit.wav", 1.0, 0.0, wav);
        }
        assert_eq!(active_voices(&m), 0);
        assert!(m.cache.is_empty());

        m.set_paused(false);
        m.play_sfx("hit.wav", 1.0, 0.0, wav);
        assert_eq!(active_voices(&m), 1);
    }

    #[test]
    fn disabled_manager_is_noop() {
        let mut m = SoundManager::disabled();
        assert!(!m.is_enabled());
        m.play_sfx("x.wav", 1.0, 0.0, || Some(vec![0u8; 4]));
        m.play_bgm("01.mp3", || None);
        m.stop_bgm();
        m.set_volumes(0.5, 0.5);
        m.tick();
        assert!(m.cache.is_empty());
    }

    fn wait_for_bgm(m: &mut SoundManager, name: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while m.current_bgm.as_deref() != Some(name) {
            m.tick();
            assert!(Instant::now() < deadline, "background music never became ready");
            std::thread::yield_now();
        }
    }

    #[test]
    fn async_music_loads_off_caller_and_deduplicates_pending_requests() {
        let mut m = silent(22050);
        let caller = std::thread::current().id();
        m.play_bgm_async("map.wav", move || {
            assert_ne!(std::thread::current().id(), caller);
            Some(crate::decode::wav_pcm16(22050, 1, 2205))
        });
        m.play_bgm_async("map.wav", || panic!("duplicate request ran"));
        wait_for_bgm(&mut m, "map.wav");
        m.play_bgm_async("map.wav", || panic!("already playing request ran"));
    }

    #[test]
    fn stopped_async_music_cannot_resume_after_a_new_map() {
        let mut m = silent(22050);
        let (started, wait_started) = std::sync::mpsc::channel();
        let (release, wait_release) = std::sync::mpsc::channel();
        m.play_bgm_async("old.wav", move || {
            started.send(()).unwrap();
            wait_release.recv().unwrap();
            Some(crate::decode::wav_pcm16(22050, 1, 2205))
        });
        wait_started.recv_timeout(Duration::from_secs(2)).unwrap();
        m.stop_bgm();
        m.play_bgm_async("new.wav", || Some(crate::decode::wav_pcm16(22050, 1, 2205)));
        release.send(()).unwrap();
        wait_for_bgm(&mut m, "new.wav");
        assert!(m.pending_bgm.is_none());
    }

    #[test]
    fn disabled_async_music_never_loads_or_starts_a_worker() {
        let mut m = SoundManager::disabled();
        m.play_bgm_async("map.wav", || panic!("disabled sound loaded a file"));
        assert!(m.bgm_worker.is_none());
    }

    #[test]
    fn sfx_byte_budget_evicts_oldest_without_freeing_playing_voice() {
        let mut m = silent(22050);
        let pcm = Arc::new(Pcm { samples: vec![0.0; 10], channels: 1 });
        let playing = pcm.clone();
        let now = Instant::now();
        m.cache.insert("old.wav".into(), CacheEntry { pcm, last_used: now - Duration::from_secs(1) });
        m.cache.insert("fresh.wav".into(), CacheEntry { pcm: Arc::new(Pcm { samples: vec![0.0; 10], channels: 1 }), last_used: now });
        m.cache_bytes = 80;
        m.trim_cache(40);
        assert!(!m.cache.contains_key("old.wav"));
        assert!(m.cache.contains_key("fresh.wav"));
        assert_eq!(playing.frames(), 10);
    }
}

#[cfg(test)]mod budget_benchmark {
 use super::*;
 #[test]#[ignore]fn repeated_budget_check_benchmark(){
  let mut m=SoundManager::disabled();let now=Instant::now();
  for i in 0..2048 {m.cache.insert(i.to_string(),CacheEntry{pcm:Arc::new(Pcm{samples:vec![0.;10],channels:1}),last_used:now});}
  m.cache_bytes=2048*40;
  let start=Instant::now();
  for _ in 0..10000 {m.trim_cache(std::hint::black_box(16*1024*1024));}
  println!("SFX_BENCH checks=10000 entries=2048 elapsed_us={}",start.elapsed().as_micros());
 }
}

#[cfg(test)]mod accounting_regression {
 use super::*;
 #[test]fn budget_accounting_matches_real_load_reuse_eviction_and_expiry(){
  let mut m=SoundManager::disabled();m.core=Some(Arc::new(Mutex::new(MixerCore::new(22050,1.,1.))));m.out_rate=22050;
  m.play_sfx("a.wav",1.,0.,||Some(crate::decode::wav_pcm16(22050,1,64)));
  let first=m.cache_bytes;assert!(first>0);
  m.play_sfx("a.wav",1.,0.,||panic!("cache hit loaded again"));assert_eq!(m.cache_bytes,first);
  m.play_sfx("b.wav",1.,0.,||Some(crate::decode::wav_pcm16(22050,1,128)));
  assert_eq!(m.cache_bytes,m.cache.values().map(|e|e.pcm.samples.len()*4).sum::<usize>());
  m.trim_cache(first);assert!(m.cache_bytes<=first);
  m.sweep(Instant::now()+Duration::from_secs(60));assert_eq!(m.cache_bytes,0);assert!(m.cache.is_empty());
 }
}
