//! One running job, one replaceable pending job, and one result. No unbounded queue.
use std::io;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;
struct State<T> {
    generation: u64,
    pending: Option<(u64, Job<T>)>,
    ready: Option<(u64, T)>,
    closed: bool,
}
pub(crate) struct LatestWorker<T: Send + 'static> {
    shared: Arc<(Mutex<State<T>>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}
impl<T: Send + 'static> LatestWorker<T> {
    pub(crate) fn new(name: &str) -> io::Result<Self> {
        let shared = Arc::new((Mutex::new(State {
            generation: 0, pending: None, ready: None, closed: false,
        }), Condvar::new()));
        let state = shared.clone();
        let worker = thread::Builder::new().name(name.into()).stack_size(512 * 1024).spawn(move || {
            loop {
                let (generation, job) = {
                    let (mutex, wake) = &*state;
                    let mut state = mutex.lock().unwrap();
                    while state.pending.is_none() && !state.closed { state = wake.wait(state).unwrap(); }
                    if state.closed { break; }
                    state.pending.take().unwrap()
                };
                let result = job();
                let stale = {
                    let mut state = state.0.lock().unwrap();
                    if state.closed || state.generation != generation { Some((generation, result)) }
                    else { state.ready.replace((generation, result)) }
                };
                drop(stale); // Large decoded results are freed outside the mailbox lock.
            }
        })?;
        Ok(Self { shared, worker: Some(worker) })
    }
    pub(crate) fn request(&self, job: impl FnOnce() -> T + Send + 'static) {
        let stale = {
            let mut state = self.shared.0.lock().unwrap();
            state.generation = state.generation.wrapping_add(1);
            let generation = state.generation;
            (state.pending.replace((generation, Box::new(job))), state.ready.take())
        };
        drop(stale);
        self.shared.1.notify_one();
    }
    pub(crate) fn cancel(&self) {
        let stale = {
            let mut state = self.shared.0.lock().unwrap();
            state.generation = state.generation.wrapping_add(1);
            (state.pending.take(), state.ready.take())
        };
        drop(stale);
    }
    pub(crate) fn take_ready(&self) -> Option<T> {
        let (generation, ready) = {
            let mut state = self.shared.0.lock().unwrap();
            (state.generation, state.ready.take())
        };
        ready.and_then(|(id, value)| (id == generation).then_some(value))
    }
}
impl<T: Send + 'static> Drop for LatestWorker<T> {
    fn drop(&mut self) {
        {
            let mut state = self.shared.0.lock().unwrap();
            state.closed = true;
            state.pending = None;
        }
        self.shared.1.notify_one();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    fn result(worker: &LatestWorker<usize>) -> usize {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(value) = worker.take_ready() { return value; }
            assert!(Instant::now() < deadline, "worker did not produce a result");
            thread::yield_now();
        }
    }
    #[test]
    fn loading_runs_off_caller_and_only_latest_pending_job_runs() {
        let caller = thread::current().id();
        let worker = LatestWorker::new("test decoder").unwrap();
        let (started, receive_started) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        worker.request(move || {
            assert_ne!(thread::current().id(), caller);
            started.send(()).unwrap(); blocked.recv().unwrap(); 1
        });
        receive_started.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.request(|| panic!("superseded queued job ran"));
        worker.request(|| 3);
        release.send(()).unwrap();
        assert_eq!(result(&worker), 3);
    }
    #[test]
    fn cancellation_discards_results_of_running_job() {
        let worker = LatestWorker::new("test decoder").unwrap();
        let (started, receive_started) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        worker.request(move || { started.send(()).unwrap(); blocked.recv().unwrap(); 9 });
        receive_started.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.cancel(); release.send(()).unwrap();
        worker.request(|| 10);
        assert_eq!(result(&worker), 10);
    }
    #[test]
    fn drop_waits_until_worker_has_released_resources() {
        let worker = LatestWorker::new("test decoder").unwrap();
        worker.request(|| 1);
        drop(worker);
    }
}

#[cfg(test)]mod memory_regression {
 use super::*;
 use std::time::{Duration,Instant};
 #[test]fn cancel_releases_an_already_decoded_track_immediately(){
  let worker=LatestWorker::new("memory test").unwrap();
  let track=Arc::new(vec![0u8;1024*1024]);let weak=Arc::downgrade(&track);
  worker.request(move||track);
  let deadline=Instant::now()+Duration::from_secs(2);
  while worker.shared.0.lock().unwrap().ready.is_none(){assert!(Instant::now()<deadline);thread::yield_now();}
  worker.cancel();
  assert!(weak.upgrade().is_none(),"cancel kept an unused decoded track until a future frame/job");
 }
}
