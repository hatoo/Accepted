use std::sync::mpsc;
use std::sync::{atomic::AtomicUsize, atomic::Ordering, Arc};
use std::thread;

pub struct JobQueue<S, T> {
    jobs: Arc<AtomicUsize>,
    s_tx: tokio::sync::mpsc::UnboundedSender<S>,
    t_rx: tokio::sync::mpsc::UnboundedReceiver<T>,
}

impl<S: Send + 'static, T: Send + 'static> JobQueue<S, T> {
    pub fn new<F: FnMut(S) -> futures::future::BoxFuture<'static, T> + Send + Sync + 'static>(
        mut func: F,
    ) -> Self {
        let jobs = Arc::new(AtomicUsize::new(0));
        let (s_tx, s_rx) = tokio::sync::mpsc::unbounded_channel();
        let (t_tx, t_rx) = tokio::sync::mpsc::unbounded_channel();

        let j = jobs.clone();
        tokio::spawn(async move {
            while let Some(s) = s_rx.recv().await {
                if t_tx.send(func(s).await).is_err() {
                    return;
                }
                j.fetch_sub(1, Ordering::Relaxed);
            }
        });

        Self { jobs, s_tx, t_rx }
    }

    pub fn rx(&mut self) -> &mut tokio::sync::mpsc::UnboundedReceiver<T> {
        &mut self.t_rx
    }

    pub fn send(&self, s: S) -> Result<(), tokio::sync::mpsc::error::SendError<S>> {
        self.jobs.fetch_add(1, Ordering::Relaxed);
        self.s_tx.send(s)
    }

    pub fn is_running(&self) -> bool {
        self.jobs.load(Ordering::Relaxed) != 0
    }
}
