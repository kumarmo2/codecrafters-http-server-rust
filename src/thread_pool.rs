#![allow(dead_code)]

use std::marker::PhantomData;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
pub(crate) struct NotStarted;

#[derive(Clone)]
pub(crate) struct Started;

#[derive(Clone)]
pub(crate) struct ThreadPool<T> {
    _phantom: PhantomData<T>,
    capacity: usize,
    end_chan: (Sender<()>, Arc<Mutex<Receiver<()>>>),
    worker_chan: (
        Sender<Box<dyn FnOnce() + Send>>,
        Arc<Mutex<Receiver<Box<dyn FnOnce() + Send>>>>,
    ),
}

impl<T> ThreadPool<T> {
    fn new(capacity: usize) -> ThreadPool<T> {
        let (tx, rx) = mpsc::channel();
        let (worker_tx, worker_rx) = mpsc::channel();
        ThreadPool {
            _phantom: PhantomData,
            capacity,
            end_chan: (tx, Arc::new(Mutex::new(rx))),
            worker_chan: (worker_tx, Arc::new(Mutex::new(worker_rx))),
        }
    }
}

pub(crate) struct ThreadPoolBuilder {}

impl ThreadPoolBuilder {
    pub(crate) fn build(&self) -> ThreadPool<NotStarted> {
        ThreadPool::new(8)
    }
}

impl ThreadPool<NotStarted> {
    pub(crate) fn start(&self) -> ThreadPool<Started> {
        let pool = self.clone();
        let _ = std::thread::spawn(move || {
            for _ in 0..pool.capacity {
                let (_, worker_rx) = &pool.worker_chan;
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
            let guard = pool.end_chan.1.lock().unwrap();
            guard.recv();
        });
        ThreadPool {
            _phantom: PhantomData,
            capacity: self.capacity,
            end_chan: self.end_chan.clone(),
            worker_chan: self.worker_chan.clone(),
        }
    }
}

impl ThreadPool<Started> {
    pub(crate) fn run(&self, f: Box<dyn FnOnce() + Send>) {
        let (tx, _) = &self.worker_chan;
        tx.send(f).unwrap();
    }
}

impl<T> Drop for ThreadPool<T> {
    fn drop(&mut self) {
        let _ = self.end_chan.0.send(());
    }
}
