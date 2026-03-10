use crate::Error::{IOError, LockError};
use crate::Result;
use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:IO";

type Job<T> = Box<dyn FnOnce(&mut T) -> Result<()> + Send + 'static>;

enum Signal {
    Next,
    Quit,
}

pub trait Callback: Send + 'static {}

pub struct IOLooper<T> {
    sender: Option<Sender<Signal>>,
    executor: Executor<T>,
}

struct Executor<T> {
    queue: Arc<Mutex<VecDeque<Job<T>>>>,
    join_handle: Option<JoinHandle<()>>,
}

impl<T: Callback> IOLooper<T> {
    pub fn new(callback: T) -> Self {
        let (sender, receiver) = channel::<Signal>();
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
                    .send(Signal::Quit)
                    .map_err(|e| IOError(e.to_string()))
            })
            .transpose()?;
        if let Some(handle) = self.executor.join_handle.take() {
            debug!(LOG_TAG, "waiting for remain tasks to finish");
            drop(self.sender.take());
            handle
                .join()
                .map_err(|_| IOError("io thread dead unexpected".to_string()))?;
        }
        Ok(())
    }

    pub fn post<F: FnOnce(&mut T) -> Result<()> + Send + 'static>(&self, task: F) -> Result<()> {
        let job: Job<T> = Box::new(task);
        self.executor
            .queue
            .lock()
            .map(|mut queue| queue.push_back(job))
            .map_err(|e| LockError(e.to_string()))?;

        self.sender
            .as_ref()
            .map(|sender| {
                sender
                    .send(Signal::Next)
                    .map_err(|e| IOError(e.to_string()))
            })
            .ok_or(IOError(
                "failed to post, channel closed unexpected".to_string(),
            ))?
    }

    #[allow(dead_code)]
    pub fn sync(&self) -> Result<()> {
        let (sender, receiver) = channel::<()>();
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

impl<T: Callback> Executor<T> {
    fn drain_pending_jobs(
        queue: &Arc<Mutex<VecDeque<Job<T>>>>,
        buffer: &mut VecDeque<Job<T>>,
        callback: &mut T,
    ) -> Result<()> {
        let mut current_queue = queue.lock().map_err(|e| LockError(e.to_string()))?;
        std::mem::swap(buffer, &mut *current_queue);
        drop(current_queue);

        let mut first_error = None;
        while let Some(job) = buffer.pop_front() {
            if let Err(e) = job(callback) {
                error!(LOG_TAG, "failed to execute io job: {:?}", e);
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
        match first_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub fn new(receiver: Receiver<Signal>, mut callback: T) -> Self {
        let mut buffer: VecDeque<Job<T>> = VecDeque::with_capacity(100);
        let queue: Arc<Mutex<VecDeque<Job<T>>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(100)));
        let queue_clone = Arc::clone(&queue);
        let handle = thread::spawn(move || loop {
            let signal = receiver.recv();

            match signal {
                Ok(Signal::Quit) => {
                    if let Err(e) =
                        Executor::drain_pending_jobs(&queue, &mut buffer, &mut callback)
                    {
                        error!(LOG_TAG, "failed to drain pending jobs before quit: {:?}", e);
                    }
                    break;
                }
                Ok(Signal::Next) => {
                    if let Err(e) =
                        Executor::drain_pending_jobs(&queue, &mut buffer, &mut callback)
                    {
                        error!(LOG_TAG, "failed to execute queued jobs: {:?}", e);
                    }
                }
                Err(_) => {
                    if let Err(e) =
                        Executor::drain_pending_jobs(&queue, &mut buffer, &mut callback)
                    {
                        error!(LOG_TAG, "failed to drain pending jobs after channel close: {:?}", e);
                    }
                    break;
                }
            }
        });
        Executor {
            queue: queue_clone,
            join_handle: Some(handle),
        }
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(io_looper.executor.queue.lock().unwrap().len(), 2);
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
        assert!(io_looper.executor.queue.lock().unwrap().is_empty());
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
}
