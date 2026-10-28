use std::time::{Duration, Instant};
const INTERVAL: Duration = Duration::from_micros(16_667);
pub(crate) struct FramePacer { deadline: Instant }
impl FramePacer {
    pub(crate) fn new(now: Instant) -> Self { Self { deadline: now } }
    pub(crate) fn delay_after_frame(&mut self, now: Instant) -> Option<Duration> {
        self.deadline += INTERVAL;
        if self.deadline <= now {
            // A slow frame needs no additional sleep or burst of catch-up frames.
            self.deadline = now;
            None
        } else { Some(self.deadline - now) }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn work_consumes_the_budget_instead_of_extending_it() {
        let now = Instant::now(); let mut pacer = FramePacer::new(now);
        assert_eq!(pacer.delay_after_frame(now + Duration::from_millis(4)), Some(Duration::from_micros(12_667)));
    }
    #[test] fn scheduler_oversleep_does_not_accumulate_as_frame_drift() {
        let now = Instant::now(); let mut pacer = FramePacer::new(now);
        pacer.delay_after_frame(now + Duration::from_millis(4));
        assert_eq!(pacer.delay_after_frame(now + Duration::from_micros(21_667)), Some(Duration::from_micros(11_667)));
    }
    #[test] fn long_frame_has_no_extra_wait_and_no_repeated_catchup() {
        let now = Instant::now(); let mut pacer = FramePacer::new(now);
        assert_eq!(pacer.delay_after_frame(now + Duration::from_millis(30)), None);
        assert_eq!(pacer.delay_after_frame(now + Duration::from_millis(34)), Some(Duration::from_micros(12_667)));
    }
}
