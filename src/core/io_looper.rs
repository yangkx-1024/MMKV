use crate::Error::IOError;
use crate::Result;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:IO";

type Job<T> = Box<dyn FnOnce(&mut T) -> Result<()> + Send + 'static>;

enum Message<T> {
    Job(Job<T>),
    Quit,
}

pub trait Executor: Send + 'static {}

pub struct IOLooper<T> {
    sender: Option<Sender<Message<T>>>,
    inner_looper: InnerLooper,
}

struct InnerLooper {
    pending_jobs: Arc<AtomicUsize>,
    join_handle: Option<JoinHandle<()>>,
}

impl<T: Executor> IOLooper<T> {
    pub fn new(executor: T) -> Self {
        let (sender, receiver) = unbounded::<Message<T>>();
        IOLooper {
            sender: Some(sender),
            inner_looper: InnerLooper::new(receiver, executor),
        }
    }

    /// Quit the looper, this call will wait for all queued tasks to finish.
    pub fn quit(&mut self) -> Result<()> {
        self.sender
            .take()
            .map(|sender| {
                sender
                    .send(Message::Quit)
                    .map_err(|e| IOError(e.to_string()))
            })
            .transpose()?;
        if let Some(handle) = self.inner_looper.join_handle.take() {
            debug!(LOG_TAG, "waiting for remain tasks to finish");
            handle
                .join()
                .map_err(|_| IOError("io thread dead unexpected".to_string()))?;
        }
        Ok(())
    }

    /// Post a task async, the returned `Result<()>` is the result of the post operation
    /// instead of task result.
    pub fn post<F>(&self, task: F) -> Result<()>
    where
        F: FnOnce(&mut T) -> Result<()> + Send + 'static,
    {
        let sender = self.sender.as_ref().ok_or(IOError(
            "failed to post, channel closed unexpected".to_string(),
        ))?;
        self.inner_looper
            .pending_jobs
            .fetch_add(1, Ordering::Relaxed);
        let send_result = sender.send(Message::Job(Box::new(task)));
        if send_result.is_err() {
            self.inner_looper
                .pending_jobs
                .fetch_sub(1, Ordering::Relaxed);
        }
        send_result.map_err(|e| IOError(e.to_string()))
    }

    /// Execute a task and wait for the task result.
    pub fn call<R, F>(&self, task: F) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut T) -> Result<R> + Send + 'static,
    {
        let (sender, receiver) = bounded::<Result<R>>(1);
        self.post(move |executor| {
            let result = task(executor);
            sender
                .send(result)
                .map_err(|e| IOError(format!("failed to return call result: {e}")))
        })?;
        receiver
            .recv()
            .map_err(|_| IOError("failed to receive call result, channel closed".to_string()))?
    }

    #[allow(dead_code)]
    pub fn sync(&self) -> Result<()> {
        self.call(|_| Ok(()))
    }
}

impl<T> Drop for IOLooper<T> {
    fn drop(&mut self) {
        let time_start = Instant::now();
        drop(self.sender.take());

        if let Some(handle) = self.inner_looper.join_handle.take() {
            match handle.join() {
                Ok(()) => verbose!(LOG_TAG, "io thread finished"),
                Err(_) => error!(LOG_TAG, "failed to join io thread while dropping IOLooper"),
            }
        }
        debug!(LOG_TAG, "IOLooper dropped, cost {:?}", time_start.elapsed());
    }
}

impl InnerLooper {
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

    pub fn new<T: Executor>(receiver: Receiver<Message<T>>, mut executor: T) -> Self {
        let pending_jobs = Arc::new(AtomicUsize::new(0));
        let pending_jobs_clone = Arc::clone(&pending_jobs);
        let handle = thread::spawn(move || {
            loop {
                match receiver.recv() {
                    Ok(Message::Job(job)) => Self::run_job(job, &mut executor, &pending_jobs),
                    Ok(Message::Quit) => {
                        debug!(LOG_TAG, "received quit signal, draining pending jobs");
                        Self::drain_pending_jobs(
                            &receiver,
                            &mut executor,
                            &pending_jobs,
                            "quit signal received while draining",
                        );
                        break;
                    }
                    Err(_) => {
                        debug!(LOG_TAG, "io channel closed, draining pending jobs");
                        Self::drain_pending_jobs(
                            &receiver,
                            &mut executor,
                            &pending_jobs,
                            "channel closed while draining",
                        );
                        break;
                    }
                }
            }
        });
        InnerLooper {
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

    use crate::core::io_looper::{Executor, IOLooper};

    struct SimpleExecutor;

    impl Executor for SimpleExecutor {}

    impl Drop for SimpleExecutor {
        fn drop(&mut self) {
            info!("MMKV:IO", "Executor dropped")
        }
    }

    impl SimpleExecutor {
        fn print(&self, str: &str) {
            info!("MMKV:IO", "{str}")
        }
    }

    #[test]
    fn test_io_loop() {
        let mut io_looper = IOLooper::new(SimpleExecutor);
        io_looper
            .post(|executor| {
                thread::sleep(Duration::from_millis(100));
                executor.print("first job");
                Ok(())
            })
            .expect("failed to execute job");
        io_looper
            .post(|executor| {
                thread::sleep(Duration::from_millis(100));
                executor.print("second job");
                Ok(())
            })
            .expect("failed to execute job");
        assert!(io_looper.sender.is_some());
        assert_eq!(
            io_looper.inner_looper.pending_jobs.load(Ordering::Relaxed),
            2
        );
        assert!(io_looper.inner_looper.join_handle.is_some());
        thread::sleep(Duration::from_millis(50));
        io_looper
            .post(|executor| {
                executor.print("last job");
                Ok(())
            })
            .unwrap();
        io_looper.quit().unwrap();
        assert!(io_looper.sender.is_none());
        assert_eq!(
            io_looper.inner_looper.pending_jobs.load(Ordering::Relaxed),
            0
        );
        assert!(io_looper.inner_looper.join_handle.is_none());
        drop(io_looper);
        let value = Arc::new(Mutex::new(1));
        let cloned_value = value.clone();
        io_looper = IOLooper::new(SimpleExecutor);
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
        struct CountingExecutor {
            executed: Arc<Mutex<Vec<usize>>>,
        }

        impl Executor for CountingExecutor {}

        let executed = Arc::new(Mutex::new(Vec::new()));
        let io_looper = Arc::new(Mutex::new(IOLooper::new(CountingExecutor {
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

    #[test]
    fn test_call_returns_result_in_order() {
        struct CountingExecutor {
            value: usize,
        }

        impl Executor for CountingExecutor {}

        let io_looper = IOLooper::new(CountingExecutor { value: 1 });
        let value = io_looper
            .call(|executor| {
                executor.value += 1;
                Ok(executor.value)
            })
            .unwrap();
        assert_eq!(value, 2);

        let value = io_looper
            .call(|executor| {
                executor.value += 3;
                Ok(executor.value)
            })
            .unwrap();
        assert_eq!(value, 5);
    }
}
