use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};

use std::thread::{spawn, JoinHandle};

type Job = Box<dyn Send + FnOnce() + 'static>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<Sender<Job>>,
}

impl ThreadPool {
    /// Create a new ThreadPool
    ///
    /// size is the number of threads inside the pool
    ///
    /// # Panics
    ///
    /// panics if the size is 0
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let mut workers: Vec<Worker> = Vec::with_capacity(size);

        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));

        for i in 0..size {
            let worker = Worker::new(i, Arc::clone(&receiver));
            workers.push(worker);
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    /// Use the thread pool to execute a task
    ///
    /// the task must implement FnOnce(), Send and has a static lifetime
    ///
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap()
    }
}

impl Drop for ThreadPool {
    /// close the channel, join the threads and leaving a none in the worker's optional thread.
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

/// Workers that have an optional thread and an id
///
/// the option is used when handling the shutdown, thus, used in the drop implementation.
struct Worker {
    id: usize,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// create a new worker that loops and waits for a job to be sent to the channel.
    ///
    /// Does not do a spinning loop, instead sleeps until a job is available.
    ///
    /// Shuts down when the sender is closed, due to the mpsc channel implementation.
    ///
    /// # Panics
    ///
    /// panics if the lock is in a poisonous state.
    ///
    /// in a production setting, we might want to handle that state.
    ///
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Job>>>) -> Self {
        let thread = spawn(move || loop {
            let message = receiver.lock().unwrap().recv();
            match message {
                Ok(job) => {
                    println!("Thread {} got a job and is executing it!", id);
                    job();
                }
                Err(e) => {
                    eprintln!(
                        "Worker {} disconnected; shutting down. closing due to: {}",
                        id, e
                    );
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
        }
    }
}
