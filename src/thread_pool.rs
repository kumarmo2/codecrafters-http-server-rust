#![allow(dead_code)]

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use std::thread;

#[derive(Clone)]
pub(crate) struct ThreadPool {
    capacity: usize,
    end_chan: (Sender<()>, Arc<Mutex<Receiver<()>>>),
    worker_chan: (
        Sender<Box<dyn FnOnce() + Send>>,
        Arc<Mutex<Receiver<Box<dyn FnOnce() + Send>>>>,
    ),
}

impl ThreadPool {
    pub(crate) fn new(capacity: usize) -> ThreadPool {
        let (tx, rx) = mpsc::channel();
        let (worker_tx, worker_rx) = mpsc::channel();
        ThreadPool {
            capacity,
            end_chan: (tx, Arc::new(Mutex::new(rx))),
            worker_chan: (worker_tx, Arc::new(Mutex::new(worker_rx))),
        }
    }

    pub(crate) fn start(&self) {
        for _ in 0..self.capacity {
            let (_, worker_rx) = &self.worker_chan;
            let worker_rx = worker_rx.clone();
            thread::spawn(move || loop {
                let item = {
                    let lock = worker_rx.lock().unwrap();
                    let item = lock.recv().unwrap();
                    item
                };
                item();
            });
        }
        let guard = self.end_chan.1.lock().unwrap();
        guard.recv();
    }

    pub(crate) fn run(&self, f: Box<dyn FnOnce() -> () + Send>) {
        let (tx, _) = &self.worker_chan;
        tx.send(f).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        let _ = self.end_chan.0.send(());
    }
}
