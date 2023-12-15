use crate::Error::{IOError, LockError};
use crate::Result;
use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:IOLooper";

type Job = Box<dyn FnOnce() + Send + 'static>;

type Signal = u8;
const SIGNAL: Signal = 0;

pub struct IOLooper {
    sender: Option<Sender<Signal>>,
    executor: Executor,
}

struct Executor {
    queue: Arc<Mutex<VecDeque<Job>>>,
    join_handle: Option<JoinHandle<()>>,
}

impl IOLooper {
    pub fn new() -> Self {
        let (sender, receiver) = channel::<Signal>();
        let executor = Executor::new(receiver);
        IOLooper {
            sender: Some(sender),
            executor,
        }
    }

    pub fn kill(&mut self) {
        let mut queue = self.executor.queue.lock().unwrap();
        queue.clear();
        drop(self.sender.take());
    }

    pub fn execute<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        let mut queue = self
            .executor
            .queue
            .lock()
            .map_err(|e| LockError(e.to_string()))?;
        queue.push_back(job);

        self.sender
            .as_ref()
            .unwrap()
            .send(SIGNAL)
            .map_err(|e| IOError(e.to_string()))?;
        Ok(())
    }
}

impl Drop for IOLooper {
    fn drop(&mut self) {
        drop(self.sender.take());

        if let Some(handle) = self.executor.join_handle.take() {
            handle.join().unwrap()
        }
    }
}

impl Executor {
    pub fn new(receiver: Receiver<Signal>) -> Self {
        verbose!(LOG_TAG, "io thread launched.");
        let queue: Arc<Mutex<VecDeque<Job>>> = Arc::new(Mutex::new(VecDeque::with_capacity(100)));
        let queue_clone = Arc::clone(&queue);
        let handle = thread::spawn(move || loop {
            let signal = receiver.recv();
            verbose!(LOG_TAG, "io thread wake up");
            match signal {
                Ok(_) => loop {
                    let job = {
                        let mut locked_queue = queue.lock().unwrap();
                        verbose!(LOG_TAG, "remain {} job(s)", locked_queue.len());
                        locked_queue.pop_front()
                    };
                    match job {
                        Some(job) => {
                            let start_time = Instant::now();
                            job();
                            verbose!(
                                LOG_TAG,
                                "job executed, cost {:?}",
                                Instant::now().duration_since(start_time)
                            )
                        }
                        None => break,
                    }
                },
                Err(_) => {
                    verbose!(LOG_TAG, "shutting down.");
                    break;
                }
            }
            verbose!(LOG_TAG, "io thread yield");
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
    use crate::core::io_looper::IOLooper;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_io_loop() {
        let io_looper = IOLooper::new();
        io_looper
            .execute(|| println!("first job"))
            .expect("failed to execute job");
        io_looper
            .execute(|| println!("second job"))
            .expect("failed to execute job");
        thread::sleep(Duration::from_secs(2));
        drop(io_looper);
    }
}
