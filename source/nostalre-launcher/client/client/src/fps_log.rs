#[cfg(target_os = "horizon")]
#[path = "fps_log_switch.rs"] mod switch;
#[cfg(target_os = "horizon")]
pub(crate) use switch::FpsLog;

#[cfg(not(target_os = "horizon"))]
mod desktop {
//! Frames per second in the log, one line a second, when SOLO_FPS_LOG is set (env.txt on Android): what the
//! native skill spam test (Ragnarok Solo's scripts/test-skill-spam-native.sh) reads.

use std::time::{Duration, Instant};

pub(crate) struct FpsLog {
    since: Option<Instant>,
    frames: u32,
}

impl FpsLog {
    pub(crate) fn from_env() -> Self {
        Self {
            since: std::env::var_os("SOLO_FPS_LOG").map(|_| Instant::now()),
            frames: 0,
        }
    }

    pub(crate) fn frame(&mut self, now: Instant) {
        let Some(since) = self.since else {
            return;
        };
        self.frames += 1;
        if now.duration_since(since) >= Duration::from_secs(1) {
            tracing::info!("fps {}", self.frames);
            self.frames = 0;
            self.since = Some(now);
        }
    }
}

}
#[cfg(not(target_os = "horizon"))]
pub(crate) use desktop::FpsLog;
