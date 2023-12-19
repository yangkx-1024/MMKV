use crate::Error::{IOError, LockError};
use crate::Result;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

const LOG_TAG: &str = "MMKV:IO";

type Job = Box<dyn FnOnce(&mut dyn std::any::Any) + Send + 'static>;

enum Signal {
    Normal,
    Kill(Job),
}

pub trait Callback: Send {}

pub struct IOLooper {
    sender: Option<Sender<Signal>>,
    executor: Executor,
}

struct Executor {
    queue: Arc<Mutex<VecDeque<Job>>>,
    join_handle: Option<JoinHandle<()>>,
}

impl IOLooper {
    pub fn new<T>(callback: T) -> Self
    where
        T: Callback + 'static,
    {
        let (sender, receiver) = channel::<Signal>();
        let executor = Executor::new(receiver, callback);
        IOLooper {
            sender: Some(sender),
            executor,
        }
    }

    pub fn post_and_kill<F>(&mut self, task: F)
    where
        F: FnOnce(&mut dyn std::any::Any) + Send + 'static,
    {
        debug!(LOG_TAG, "try kill io thread");
        self.executor.queue.lock().unwrap().clear();
        self.sender
            .as_ref()
            .unwrap()
            .send(Signal::Kill(Box::new(task)))
            .unwrap();
        drop(self.sender.take());
        if let Some(handle) = self.executor.join_handle.take() {
            handle.join().unwrap();
            verbose!(LOG_TAG, "io thread finished");
        }
    }

    pub fn post<F>(&self, task: F) -> Result<()>
    where
        F: FnOnce(&mut dyn std::any::Any) + Send + 'static,
    {
        self.executor
            .queue
            .lock()
            .map(|mut queue| queue.push_back(Box::new(task)))
            .map_err(|e| LockError(e.to_string()))?;

        self.sender
            .as_ref()
            .unwrap()
            .send(Signal::Normal)
            .map_err(|e| IOError(e.to_string()))?;
        Ok(())
    }

    pub fn sync(&self) {
        let synced = Arc::new(AtomicBool::new(false));
        let synced_clone = synced.clone();
        self.post(move |_| {
            synced.store(true, Ordering::Release);
        })
        .unwrap();
        loop {
            if synced_clone.load(Ordering::Acquire) {
                break;
            }
        }
    }
}

impl Drop for IOLooper {
    fn drop(&mut self) {
        drop(self.sender.take());

        if let Some(handle) = self.executor.join_handle.take() {
            handle.join().unwrap();
            verbose!(LOG_TAG, "io thread finished");
        }
    }
}

impl Executor {
    pub fn new<T>(receiver: Receiver<Signal>, mut callback: T) -> Self
    where
        T: Callback + std::any::Any + 'static,
    {
        let queue: Arc<Mutex<VecDeque<Job>>> = Arc::new(Mutex::new(VecDeque::with_capacity(100)));
        let queue_clone = Arc::clone(&queue);
        let handle = thread::spawn(move || loop {
            let callback = &mut callback;
            let signal = receiver.recv();

            match signal {
                Ok(Signal::Kill(job)) => {
                    job(callback);
                    break;
                }
                Ok(Signal::Normal) => loop {
                    let mut locked_queue = queue.lock().unwrap();
                    let job = locked_queue.pop_front();
                    drop(locked_queue);
                    match job {
                        Some(job) => {
                            job(callback);
                        }
                        None => break,
                    }
                },
                Err(_) => {
                    break;
                }
            }
            thread::yield_now();
        });
        Executor {
            queue: queue_clone,
            join_handle: Some(handle),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::io_looper::{Callback, IOLooper};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    struct SimpleCallback;

    impl Callback for SimpleCallback {}
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
                callback
                    .downcast_ref::<SimpleCallback>()
                    .unwrap()
                    .print("first job")
            })
            .expect("failed to execute job");
        io_looper
            .post(|callback| {
                thread::sleep(Duration::from_millis(100));
                callback
                    .downcast_ref::<SimpleCallback>()
                    .unwrap()
                    .print("second job")
            })
            .expect("failed to execute job");
        assert!(io_looper.sender.is_some());
        assert_eq!(io_looper.executor.queue.lock().unwrap().len(), 2);
        assert!(io_looper.executor.join_handle.is_some());
        thread::sleep(Duration::from_millis(50));
        io_looper.post_and_kill(|callback| {
            callback
                .downcast_ref::<SimpleCallback>()
                .unwrap()
                .print("last job")
        });
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
            })
            .expect("failed to execute job");
        drop(io_looper);
        assert_eq!(*value.lock().unwrap(), 2);
    }
}
