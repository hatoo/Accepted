use std::sync::mpsc;
use std::sync::{atomic::AtomicUsize, atomic::Ordering, Arc};
use std::thread;

pub struct JobQueue<S, T> {
    jobs: Arc<AtomicUsize>,
    s_tx: mpsc::Sender<S>,
    t_rx: mpsc::Receiver<T>,
}

impl<S: Send + 'static, T: Send + 'static> JobQueue<S, T> {
    pub fn new<F: FnMut(S) -> T + Send + 'static>(mut func: F) -> Self {
        let jobs = Arc::new(AtomicUsize::new(0));
        let (s_tx, s_rx) = mpsc::channel();
        let (t_tx, t_rx) = mpsc::channel();

        let j = jobs.clone();
        thread::spawn(move || {
            for s in s_rx {
                if t_tx.send(func(s)).is_err() {
                    return;
                }
                j.fetch_sub(1, Ordering::Relaxed);
            }
        });

        Self { jobs, s_tx, t_rx }
    }

    pub fn rx(&self) -> &mpsc::Receiver<T> {
        &self.t_rx
    }

    pub fn send(&self, s: S) -> Result<(), mpsc::SendError<S>> {
        self.jobs.fetch_add(1, Ordering::Relaxed);
        self.s_tx.send(s)
    }

    pub fn is_running(&self) -> bool {
        self.jobs.load(Ordering::Relaxed) != 0
    }
}
