#![allow(dead_code)]

use std::marker::PhantomData;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
pub(crate) struct NotStarted;

#[derive(Clone)]
pub(crate) struct Started;

type Job = Box<dyn FnOnce() + Send>;

#[derive(Clone)]
pub(crate) struct Inner<T> {
    _phantom: PhantomData<T>,
    capacity: usize,
    // damn these Arc<Mutex<_>>. they creep everywhere. is this the promise of "Feareless Concurrency" or is it just Skill
    // issue?
    end_chan: (Sender<()>, Arc<Mutex<Receiver<()>>>),
    worker_chan: (Sender<Job>, Arc<Mutex<Receiver<Job>>>),
}

#[derive(Clone)]
pub(crate) struct ThreadPool<T> {
    _inner: Arc<Inner<T>>,
}

impl<T> ThreadPool<T> {
    fn new(capacity: usize) -> ThreadPool<T> {
        let (tx, rx) = mpsc::channel();
        let (worker_tx, worker_rx) = mpsc::channel();
        let _inner = Arc::new(Inner {
            _phantom: PhantomData,
            capacity,
            end_chan: (tx, Arc::new(Mutex::new(rx))),
            worker_chan: (worker_tx, Arc::new(Mutex::new(worker_rx))),
        });
        ThreadPool { _inner }
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
        let pool = self._inner.clone();
        let _ = std::thread::spawn(move || {
            for _ in 0..pool.capacity {
                let (_, worker_rx) = &pool.worker_chan;
                let worker_rx = worker_rx.clone();
                thread::spawn(move || loop {
                    let item = match worker_rx.lock() {
                        Ok(guard) => match guard.recv() {
                            Err(_) => continue,
                            Ok(item) => item,
                        },
                        Err(_) => {
                            continue;
                        }
                    };
                    item();
                });
            }
            loop {
                let Ok(guard) = pool.end_chan.1.lock() else {
                    continue;
                };

                let _ = guard.recv();
                break;
            }
        });
        let pool = self._inner.clone();
        let _inner = Inner::<Started> {
            _phantom: PhantomData,
            capacity: pool.capacity,
            end_chan: pool.end_chan.clone(),
            worker_chan: pool.worker_chan.clone(),
        };
        ThreadPool {
            _inner: Arc::new(_inner),
        }
    }
}

impl ThreadPool<Started> {
    pub(crate) fn run(&self, f: Job) {
        let (tx, _) = &self._inner.worker_chan;
        let _ = tx.send(f);
    }
}

impl<T> Drop for ThreadPool<T> {
    fn drop(&mut self) {
        let _ = self._inner.end_chan.0.send(());
    }
}
