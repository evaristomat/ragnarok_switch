//! Fixed-size frametime statistics; one queued log line per reporting interval.
use std::time::{Duration, Instant};
#[path = "frame_stats.rs"] mod frame_stats;
use frame_stats::FrameStats;
pub(crate) struct FpsLog {
    stage_us: [u32; 11], stage_totals: [u64; 11], stage_frames: u32,
    moving_frames: u32, distance_cells: f32, last_position: Option<(f32, f32)>,
    worst_stages: [u32; 11], worst_total: u64,
    enabled: bool, since: Option<Instant>, last: Option<Instant>, stats: FrameStats,
}
impl FpsLog {
    pub(crate) fn from_env() -> Self {
        Self {
            enabled: cfg!(target_os = "horizon") || std::env::var_os("SOLO_FPS_LOG").is_some(),
            stage_us: [0; 11], stage_totals: [0; 11], stage_frames: 0, worst_stages: [0; 11], worst_total: 0,
            moving_frames: 0, distance_cells: 0.0, last_position: None,
            since: None, last: None, stats: FrameStats::default(),
        }
    }
    pub(crate) fn begin_window(&mut self, now: Instant) {
        self.since = Some(now); self.last = Some(now); self.stats = FrameStats::default();
        self.stage_us = [0; 11]; self.stage_totals = [0; 11]; self.stage_frames = 0;
        self.worst_stages = [0; 11]; self.worst_total = 0;
        self.moving_frames = 0; self.distance_cells = 0.0;
    }
    pub(crate) fn stage(&mut self, index: usize, last: &mut Instant) {
        let now = Instant::now();
        self.stage_us[index] = now.saturating_duration_since(*last).as_micros().min(u32::MAX as u128) as u32;
        *last = now;
    }
    pub(crate) fn render_parts(&mut self, values: [u32; 3]) { self.stage_us[8..].copy_from_slice(&values); }
    pub(crate) fn finish_stages(&mut self, moving: bool, position: Option<(f32, f32)>) {
        if let (true, Some((x, y)), Some((px, py))) = (moving, position, self.last_position) {
            let distance = ((x-px).powi(2) + (y-py).powi(2)).sqrt();
            // Exclude map transfers and stationary movement states from the walking test.
            if distance > 0.0001 && distance < 4.0 {
                self.moving_frames += 1; self.distance_cells += distance;
            }
        }
        self.last_position = position;
        let total: u64 = self.stage_us[..8].iter().map(|&v| u64::from(v)).sum();
        for (sum, value) in self.stage_totals.iter_mut().zip(self.stage_us) { *sum += u64::from(value); }
        self.stage_frames += 1;
        if total > self.worst_total { self.worst_total = total; self.worst_stages = self.stage_us; }
    }
    pub(crate) fn frame(&mut self, now: Instant) {
        if !self.enabled { return; }
        let since = *self.since.get_or_insert(now);
        if let Some(last) = self.last.replace(now) {
            self.stats.record(now.saturating_duration_since(last).as_micros().min(u32::MAX as u128) as u32);
        }
        let elapsed = now.saturating_duration_since(since);
        if elapsed >= Duration::from_secs(5) {
            let [p50,p95,p99] = self.stats.percentiles();
            tracing::info!("[Perf] fps {:.1}; frame_ms p50={:.2} p95={:.2} p99={:.2} max={:.2}; frames_gt_33ms={}; moving_frames={}; measured_frames={}; distance_cells={:.2}",
                self.stats.frames as f64 / elapsed.as_secs_f64(), p50 as f64/1000.0,
                p95 as f64/1000.0, p99 as f64/1000.0, self.stats.max_us as f64/1000.0, self.stats.slow_frames, self.moving_frames, self.stage_frames, self.distance_cells);
            let count = self.stage_frames.max(1) as f64 * 1000.0;
            let averages = self.stage_totals.map(|value| value as f64 / count);
            tracing::info!("[PerfStage] order=input,events,ui,simulation_assets,scene_pick,overlays,render,end_ui,acquire_image,encode_submit,present; avg_ms={:?}; worst_frame_us={:?}", averages, self.worst_stages);
            self.moving_frames = 0; self.distance_cells = 0.0;
            self.stage_totals = [0; 11]; self.stage_frames = 0; self.worst_total = 0; self.worst_stages = [0; 11];
            self.stats = FrameStats::default(); self.since = Some(now);
        }
    }
}

#[cfg(test)] mod window_tests {
    use super::*;
    #[test] fn new_window_discards_loading_but_keeps_first_walk_stall() {
        let now = Instant::now(); let mut log = FpsLog::from_env(); log.enabled = true;
        log.frame(now); log.frame(now + Duration::from_millis(700));
        assert_eq!(log.stats.max_us, 700_000);
        log.begin_window(now + Duration::from_secs(1));
        assert_eq!(log.stats.frames, 0);
        log.frame(now + Duration::from_millis(1900));
        assert_eq!(log.stats.frames, 1); assert_eq!(log.stats.max_us, 900_000);
    }
}
