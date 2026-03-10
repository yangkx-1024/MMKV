use crate::Error::IOError;
use crate::Result;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:IO";

type Job<T> = Box<dyn FnOnce(&mut T) -> Result<()> + Send + 'static>;

enum Message<T> {
    Job(Job<T>),
    Quit,
}

pub trait Callback: Send + 'static {}

pub struct IOLooper<T> {
    sender: Option<Sender<Message<T>>>,
    executor: Executor,
}

struct Executor {
    pending_jobs: Arc<AtomicUsize>,
    join_handle: Option<JoinHandle<()>>,
}

impl<T: Callback> IOLooper<T> {
    pub fn new(callback: T) -> Self {
        let (sender, receiver) = unbounded::<Message<T>>();
        let executor = Executor::new(receiver, callback);
        IOLooper {
            sender: Some(sender),
            executor,
        }
    }

    pub fn quit(&mut self) -> Result<()> {
        self.sender
            .take()
            .map(|sender| {
                sender
                    .send(Message::Quit)
                    .map_err(|e| IOError(e.to_string()))
            })
            .transpose()?;
        if let Some(handle) = self.executor.join_handle.take() {
            debug!(LOG_TAG, "waiting for remain tasks to finish");
            handle
                .join()
                .map_err(|_| IOError("io thread dead unexpected".to_string()))?;
        }
        Ok(())
    }

    pub fn post<F: FnOnce(&mut T) -> Result<()> + Send + 'static>(&self, task: F) -> Result<()> {
        let sender = self.sender.as_ref().ok_or(IOError(
            "failed to post, channel closed unexpected".to_string(),
        ))?;
        self.executor.pending_jobs.fetch_add(1, Ordering::Relaxed);
        let send_result = sender.send(Message::Job(Box::new(task)));
        if send_result.is_err() {
            self.executor.pending_jobs.fetch_sub(1, Ordering::Relaxed);
        }
        send_result.map_err(|e| IOError(e.to_string()))
    }

    #[allow(dead_code)]
    pub fn sync(&self) -> Result<()> {
        let (sender, receiver) = bounded::<()>(0);
        self.post(move |_| {
            sender
                .send(())
                .map_err(|e| IOError(format!("failed to sync, sender dropped: {e}")))
        })?;
        receiver
            .recv()
            .map_err(|_| IOError("failed to sync, channel closed unexpected".to_string()))?;
        Ok(())
    }
}

impl<T> Drop for IOLooper<T> {
    fn drop(&mut self) {
        let time_start = Instant::now();
        drop(self.sender.take());

        if let Some(handle) = self.executor.join_handle.take() {
            match handle.join() {
                Ok(()) => verbose!(LOG_TAG, "io thread finished"),
                Err(_) => error!(LOG_TAG, "failed to join io thread while dropping IOLooper"),
            }
        }
        debug!(LOG_TAG, "IOLooper dropped, cost {:?}", time_start.elapsed());
    }
}

impl Executor {
    fn run_job<T>(job: Job<T>, callback: &mut T, pending_jobs: &AtomicUsize) {
        if let Err(e) = job(callback) {
            error!(LOG_TAG, "failed to execute io job: {:?}", e);
        }
        pending_jobs.fetch_sub(1, Ordering::Relaxed);
    }

    fn drain_pending_jobs<T>(
        receiver: &Receiver<Message<T>>,
        callback: &mut T,
        pending_jobs: &AtomicUsize,
        reason: &str,
    ) {
        while let Ok(message) = receiver.try_recv() {
            match message {
                Message::Job(job) => Self::run_job(job, callback, pending_jobs),
                Message::Quit => {
                    debug!(LOG_TAG, "stop draining pending jobs: {}", reason);
                    break;
                }
            }
        }
    }

    pub fn new<T: Callback>(receiver: Receiver<Message<T>>, mut callback: T) -> Self {
        let pending_jobs = Arc::new(AtomicUsize::new(0));
        let pending_jobs_clone = Arc::clone(&pending_jobs);
        let handle = thread::spawn(move || {
            loop {
                match receiver.recv() {
                    Ok(Message::Job(job)) => Self::run_job(job, &mut callback, &pending_jobs),
                    Ok(Message::Quit) => {
                        debug!(LOG_TAG, "received quit signal, draining pending jobs");
                        Self::drain_pending_jobs(
                            &receiver,
                            &mut callback,
                            &pending_jobs,
                            "quit signal received while draining",
                        );
                        break;
                    }
                    Err(_) => {
                        debug!(LOG_TAG, "io channel closed, draining pending jobs");
                        Self::drain_pending_jobs(
                            &receiver,
                            &mut callback,
                            &pending_jobs,
                            "channel closed while draining",
                        );
                        break;
                    }
                }
            }
        });
        Executor {
            pending_jobs: pending_jobs_clone,
            join_handle: Some(handle),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use crate::core::io_looper::{Callback, IOLooper};

    struct SimpleCallback;

    impl Callback for SimpleCallback {}

    impl Drop for SimpleCallback {
        fn drop(&mut self) {
            info!("MMKV:IO", "Callback dropped")
        }
    }

    impl SimpleCallback {
        fn print(&self, str: &str) {
            info!("MMKV:IO", "{str}")
        }
    }

    #[test]
    fn test_io_loop() {
        let mut io_looper = IOLooper::new(SimpleCallback);
        io_looper
            .post(|callback| {
                thread::sleep(Duration::from_millis(100));
                callback.print("first job");
                Ok(())
            })
            .expect("failed to execute job");
        io_looper
            .post(|callback| {
                thread::sleep(Duration::from_millis(100));
                callback.print("second job");
                Ok(())
            })
            .expect("failed to execute job");
        assert!(io_looper.sender.is_some());
        assert_eq!(io_looper.executor.pending_jobs.load(Ordering::Relaxed), 2);
        assert!(io_looper.executor.join_handle.is_some());
        thread::sleep(Duration::from_millis(50));
        io_looper
            .post(|callback| {
                callback.print("last job");
                Ok(())
            })
            .unwrap();
        io_looper.quit().unwrap();
        assert!(io_looper.sender.is_none());
        assert_eq!(io_looper.executor.pending_jobs.load(Ordering::Relaxed), 0);
        assert!(io_looper.executor.join_handle.is_none());
        drop(io_looper);
        let value = Arc::new(Mutex::new(1));
        let cloned_value = value.clone();
        io_looper = IOLooper::new(SimpleCallback);
        io_looper
            .post(move |_| {
                thread::sleep(Duration::from_millis(100));
                *cloned_value.lock().unwrap() = 2;
                Ok(())
            })
            .expect("failed to execute job");
        assert_eq!(*value.lock().unwrap(), 1);
        drop(io_looper);
        assert_eq!(*value.lock().unwrap(), 2);
    }

    #[test]
    fn test_concurrent_post_and_quit_does_not_drop_accepted_jobs() {
        struct CountingCallback {
            executed: Arc<Mutex<Vec<usize>>>,
        }

        impl Callback for CountingCallback {}

        let executed = Arc::new(Mutex::new(Vec::new()));
        let io_looper = Arc::new(Mutex::new(IOLooper::new(CountingCallback {
            executed: Arc::clone(&executed),
        })));
        let accepted = Arc::new(Mutex::new(Vec::new()));
        let next_id = Arc::new(AtomicUsize::new(0));

        let producer_count = 6;
        let jobs_per_producer = 200;
        let mut producers = Vec::with_capacity(producer_count);
        for _ in 0..producer_count {
            let io_looper = Arc::clone(&io_looper);
            let accepted = Arc::clone(&accepted);
            let next_id = Arc::clone(&next_id);
            producers.push(thread::spawn(move || {
                for _ in 0..jobs_per_producer {
                    let job_id = next_id.fetch_add(1, Ordering::Relaxed);
                    let result = io_looper.lock().unwrap().post(move |callback| {
                        callback.executed.lock().unwrap().push(job_id);
                        Ok(())
                    });
                    if result.is_ok() {
                        accepted.lock().unwrap().push(job_id);
                    } else {
                        break;
                    }
                }
            }));
        }

        let quit_looper = Arc::clone(&io_looper);
        let quitter = thread::spawn(move || {
            thread::sleep(Duration::from_millis(2));
            quit_looper.lock().unwrap().quit().unwrap();
        });

        for producer in producers {
            producer.join().unwrap();
        }
        quitter.join().unwrap();

        let accepted = accepted.lock().unwrap().clone();
        let mut executed = executed.lock().unwrap().clone();
        executed.sort_unstable();

        let mut accepted_sorted = accepted.clone();
        accepted_sorted.sort_unstable();

        assert_eq!(accepted_sorted, executed);
        assert_eq!(
            accepted_sorted
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len(),
            accepted_sorted.len()
        );
    }
}
