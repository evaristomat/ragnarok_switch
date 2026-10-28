//! One running asset job and at most one ready result; render calls never wait.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
};

pub(crate) struct BackgroundWorker<Job, Ready> {
    requests: Option<SyncSender<Job>>,
    results: Receiver<Ready>,
    cancelled: Arc<AtomicBool>,
    busy: bool,
}

impl<Job: Send + 'static, Ready: Send + 'static> BackgroundWorker<Job, Ready> {
    pub(crate) fn new(
        name: &str,
        mut load: impl FnMut(Job) -> Ready + Send + 'static,
    ) -> std::io::Result<Self> {
        let (requests, jobs) = mpsc::sync_channel::<Job>(1);
        let (completed, results) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        std::thread::Builder::new()
            .name(name.into())
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let ready = load(job);
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    if completed.send(ready).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            results,
            cancelled,
            busy: false,
        })
    }

    pub(crate) fn submit(&mut self, job: Job) -> Result<(), TrySendError<Job>> {
        if self.busy {
            return Err(TrySendError::Full(job));
        }
        self.requests.as_ref().unwrap().try_send(job)?;
        self.busy = true;
        Ok(())
    }

    pub(crate) fn poll(&mut self) -> Result<Option<Ready>, TryRecvError> {
        match self.results.try_recv() {
            Ok(ready) => {
                self.busy = false;
                Ok(Some(ready))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(error) => {
                self.busy = false;
                Err(error)
            }
        }
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.busy
    }
}

impl<Job, Ready> Drop for BackgroundWorker<Job, Ready> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.requests.take();
        // Dropping results disconnects a sender even if it is completing a job.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn slow_asset_never_runs_on_or_blocks_the_render_thread() {
        let render_thread = std::thread::current().id();
        let (entered, inside) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let mut worker = BackgroundWorker::new("slow-effect-test", move |value| {
            entered.send(std::thread::current().id()).unwrap();
            blocked.recv().unwrap();
            value * 2
        })
        .unwrap();
        worker.submit(7).unwrap();
        assert_ne!(
            inside.recv_timeout(Duration::from_secs(2)).unwrap(),
            render_thread
        );
        assert!(
            worker.poll().unwrap().is_none(),
            "poll must return while the asset is still blocked"
        );
        assert!(matches!(worker.submit(8), Err(TrySendError::Full(8))));
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(result) = worker.poll().unwrap() {
                assert_eq!(result, 14);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!worker.is_busy());
    }

    #[test]
    fn a_ready_result_must_be_consumed_before_another_job_can_allocate() {
        let (finished, done) = mpsc::channel();
        let mut worker = BackgroundWorker::new("bounded-effect-test", move |value| {
            finished.send(()).unwrap();
            value
        })
        .unwrap();
        worker.submit(1).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(worker.submit(2), Err(TrySendError::Full(2))));
        while worker.poll().unwrap().is_none() {
            std::thread::yield_now();
        }
        worker.submit(2).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
        loop {
            if let Some(value) = worker.poll().unwrap() {
                assert_eq!(value, 2);
                break;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn dropping_an_idle_worker_releases_its_captured_resources() {
        let resource = Arc::new(());
        let weak = Arc::downgrade(&resource);
        let worker = BackgroundWorker::<(), ()>::new("cancel-idle-effect-test", move |_| {
            let _ = &resource;
        })
        .unwrap();
        drop(worker);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while weak.upgrade().is_some() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    #[test]
    fn cancelling_a_running_job_does_not_keep_its_ready_payload() {
        let (entered, inside) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let resource = Arc::new(());
        let weak = Arc::downgrade(&resource);
        let mut worker = BackgroundWorker::new("cancel-running-effect-test", move |_| {
            entered.send(()).unwrap();
            blocked.recv().unwrap();
            resource.clone()
        })
        .unwrap();
        worker.submit(()).unwrap();
        inside.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(worker);
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while weak.upgrade().is_some() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
}
