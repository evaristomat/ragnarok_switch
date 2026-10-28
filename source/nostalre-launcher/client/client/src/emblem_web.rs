//! Guild emblems through rAthena's web server (`nostalre_network::emblem_http`) on one worker thread: the game
//! thread queues jobs and gets the results back as events, with the network's.

use nostalre_game::event::GameEvent;
use nostalre_network::emblem_http::{self, WebAuth, WebError};
use std::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;

pub(crate) enum EmblemJob {
    Download {
        auth: WebAuth,
        gdid: u32,
        version: i32,
    },
    Upload {
        auth: WebAuth,
        gdid: u32,
        bmp: Vec<u8>,
    },
}

/// Starts the worker; None when no thread can be made (emblems then come from the map server only).
pub(crate) fn spawn(events: UnboundedSender<GameEvent>) -> Option<mpsc::Sender<EmblemJob>> {
    let (jobs, queue) = mpsc::channel();
    std::thread::Builder::new()
        .name("emblem-web".into())
        .spawn(move || run(queue, events))
        .inspect_err(|e| tracing::warn!("no emblem worker thread: {e}"))
        .ok()?;
    Some(jobs)
}

fn run(queue: mpsc::Receiver<EmblemJob>, events: UnboundedSender<GameEvent>) {
    // once a connection fails, nothing is there (the phone runs no web server): no more tries this session
    let mut unreachable = false;
    for job in queue {
        let event = match job {
            EmblemJob::Download { .. } if unreachable => None,
            EmblemJob::Download {
                auth,
                gdid,
                version,
            } => emblem_http::download(&auth, gdid)
                .inspect_err(|e| unreachable |= log_failure(e))
                .ok()
                .map(|bmp| GameEvent::GuildEmblem {
                    gdid,
                    version,
                    bmp,
                    from_map_server: false,
                }),
            EmblemJob::Upload { auth, gdid, bmp } => {
                let version = (!unreachable)
                    .then(|| emblem_http::upload(&auth, gdid, &bmp))
                    .and_then(|r| r.inspect_err(|e| unreachable |= log_failure(e)).ok());
                Some(GameEvent::GuildEmblemUploaded { gdid, version, bmp })
            }
        };
        if let Some(event) = event
            && events.send(event).is_err()
        {
            return; // the game is gone
        }
    }
}

/// Logs a failed request; whether it says there is no web server.
fn log_failure(e: &WebError) -> bool {
    match e {
        WebError::Unreachable(m) => {
            tracing::info!("no web server for guild emblems ({m}): the map server's only");
            true
        }
        WebError::Failed(m) => {
            tracing::warn!("guild emblem web request: {m}");
            false
        }
    }
}
