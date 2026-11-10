//! A fixed-size generation pool with a priority queue the main thread re-prioritises each frame.
//!
//! Bevy's task pools would work too, but E3/E4 need an exact worker count (the 4-worker cap in
//! docs/21) and nearest-first ordering that changes as the camera moves.

use crate::patch::{self, PatchMesh};
use crate::quadtree::{Lod, NodeId};
use spike_field::Body;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

struct Queue {
    /// Wanted patches and their priority (lower = sooner).
    wanted: HashMap<NodeId, f64>,
    /// Taken by a worker and not yet received by the main thread: building, or finished and
    /// waiting in the channel. Never re-queued while here.
    ///
    /// E3 finding: without this, patches waiting in the channel still looked missing, were queued
    /// again, and were rebuilt repeatedly; the backlog grew to 13 GB.
    in_flight: HashSet<NodeId>,
    shutdown: bool,
}

pub struct Pool {
    shared: Arc<(Mutex<Queue>, Condvar)>,
    results: Mutex<Receiver<PatchMesh>>,
}

impl Pool {
    pub fn new(workers: usize, body: Arc<Body>, lod: Lod) -> Pool {
        let shared = Arc::new((
            Mutex::new(Queue {
                wanted: HashMap::new(),
                in_flight: HashSet::new(),
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let (tx, rx): (Sender<PatchMesh>, Receiver<PatchMesh>) = channel();
        for w in 0..workers {
            let shared = shared.clone();
            let body = body.clone();
            let tx = tx.clone();
            std::thread::Builder::new()
                .name(format!("gen-{w}"))
                .spawn(move || {
                    loop {
                        let job = {
                            let (lock, cv) = &*shared;
                            let mut q = lock.lock().unwrap();
                            loop {
                                if q.shutdown {
                                    return;
                                }
                                let best = q
                                    .wanted
                                    .iter()
                                    .min_by(|a, b| a.1.total_cmp(b.1).then(a.0.cmp(b.0)))
                                    .map(|(id, _)| *id);
                                if let Some(id) = best {
                                    q.wanted.remove(&id);
                                    q.in_flight.insert(id);
                                    break id;
                                }
                                q = cv.wait(q).unwrap();
                            }
                        };
                        if tx.send(patch::build(&body, &lod, job)).is_err() {
                            return;
                        }
                    }
                })
                .expect("spawn worker");
        }
        Pool {
            shared,
            results: Mutex::new(rx),
        }
    }

    /// Replace the wanted set, skipping anything already in flight. Jobs already running are not
    /// cancelled; jobs no longer wanted are dropped from the queue. Returns the queue length.
    pub fn set_wanted(&self, mut wanted: HashMap<NodeId, f64>) -> usize {
        let (lock, cv) = &*self.shared;
        let mut q = lock.lock().unwrap();
        wanted.retain(|id, _| !q.in_flight.contains(id));
        let n = wanted.len();
        q.wanted = wanted;
        cv.notify_all();
        n
    }

    pub fn in_flight(&self) -> usize {
        self.shared.0.lock().unwrap().in_flight.len()
    }

    pub fn try_recv(&self) -> Option<PatchMesh> {
        let p = self.results.lock().unwrap().try_recv().ok()?;
        self.shared.0.lock().unwrap().in_flight.remove(&p.id);
        Some(p)
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        let (lock, cv) = &*self.shared;
        lock.lock().unwrap().shutdown = true;
        cv.notify_all();
    }
}
