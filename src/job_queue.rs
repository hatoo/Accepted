use std::sync::{self, Arc, Mutex};
use std::sync::mpsc;
use std::thread;

pub struct JobQueue<S, T> {
    jobs: Arc<Mutex<usize>>,
    s_tx: mpsc::Sender<S>,
    t_rx: mpsc::Receiver<T>,
}

impl<S: Send + 'static, T: Send + 'static> JobQueue<S, T> {
    pub fn new<F: FnMut(S) -> T + Send + 'static>(mut func: F) -> Self {
        let jobs = Arc::new(Mutex::new(0));
        let (s_tx, s_rx) = mpsc::channel();
        let (t_tx, t_rx) = mpsc::channel();

        let j = jobs.clone();
        thread::spawn(move || {
            for s in s_rx {
                t_tx.send(func(s)).unwrap();
                *j.lock().unwrap() -= 1;
            }
        });

        Self { jobs, s_tx, t_rx }
    }

    pub fn rx(&self) -> &mpsc::Receiver<T> {
        &self.t_rx
    }

    pub fn send(&self, s: S) -> Result<(), mpsc::SendError<S>> {
        {
            *self.jobs.lock().unwrap() += 1;
        }
        self.s_tx.send(s)
    }

    pub fn is_running(
        &self,
    ) -> std::result::Result<bool, sync::PoisonError<std::sync::MutexGuard<'_, usize>>> {
        self.jobs.lock().map(|x| *x != 0)
    }
}
