const CAPACITY: usize = 512;
pub(crate) struct FrameStats {
    samples: [u32; CAPACITY], used: usize, cursor: usize,
    pub frames: u32, pub max_us: u32, pub slow_frames: u32,
}
impl Default for FrameStats {
    fn default() -> Self { Self { samples: [0; CAPACITY], used: 0, cursor: 0, frames: 0, max_us: 0, slow_frames: 0 } }
}
impl FrameStats {
    pub(crate) fn record(&mut self, micros: u32) {
        self.samples[self.cursor] = micros;
        self.cursor = (self.cursor + 1) % CAPACITY;
        self.used = (self.used + 1).min(CAPACITY);
        self.frames += 1;
        self.max_us = self.max_us.max(micros);
        self.slow_frames += u32::from(micros > 33_334);
    }
    pub(crate) fn percentiles(&self) -> [u32; 3] {
        if self.used == 0 { return [0; 3]; }
        let mut sorted = self.samples;
        sorted[..self.used].sort_unstable();
        [50, 95, 99].map(|p| sorted[(self.used * p).div_ceil(100).saturating_sub(1)])
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn uniform_frames_keep_exact_percentiles() {
        let mut stats = FrameStats::default();
        for _ in 0..300 { stats.record(16_667); }
        assert_eq!(stats.percentiles(), [16_667; 3]); assert_eq!(stats.slow_frames, 0);
    }
    #[test] fn isolated_stall_is_preserved_in_max_and_slow_count() {
        let mut stats = FrameStats::default();
        stats.record(100_000); for _ in 0..99 { stats.record(16_667); }
        assert_eq!(stats.max_us, 100_000); assert_eq!(stats.slow_frames, 1);
        assert_eq!(stats.percentiles(), [16_667; 3]);
    }
    #[test] fn ring_is_bounded_but_period_max_and_counts_cover_all_frames() {
        let mut stats = FrameStats::default(); stats.record(200_000);
        for _ in 0..1024 { stats.record(10_000); }
        assert_eq!(stats.used, CAPACITY); assert_eq!(stats.frames, 1025);
        assert_eq!(stats.max_us, 200_000); assert_eq!(stats.percentiles(), [10_000; 3]);
    }
}
