// SPDX-License-Identifier: MIT OR Apache-2.0
//! Disposable source instrumentation. Never linked into the ordinary game.
#![allow(dead_code)]
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::{BTreeMap, BTreeSet},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use voxel_core::{ChunkPos, streaming::Column};
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
struct Meter;
// SAFETY: unchanged pointers/layouts are forwarded to System. Counters never allocate.
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let n = LIVE.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(n, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            LIVE.fetch_sub(l.size(), Ordering::Relaxed);
            let v = LIVE.fetch_add(n, Ordering::Relaxed) + n;
            PEAK.fetch_max(v, Ordering::Relaxed);
        }
        q
    }
}
#[global_allocator]
static ALLOC: Meter = Meter;
#[derive(Default)]
struct Data {
    samples: BTreeMap<&'static str, Vec<f64>>,
    loads: BTreeSet<(Column, Instant)>,
    dirty: BTreeSet<(ChunkPos, Instant)>,
}
static DATA: Mutex<Data> = Mutex::new(Data {
    samples: BTreeMap::new(),
    loads: BTreeSet::new(),
    dirty: BTreeSet::new(),
});
pub fn reset() {
    *DATA.lock().unwrap() = Data::default();
}
pub fn value(metric: &'static str, v: f64) {
    static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *TRACE.get_or_init(|| std::env::var_os("RUSTCRAFT_STREAM_TRACE").is_some()) {
        println!("S32_TRACE,{metric},{v:.6}");
        return;
    }
    DATA.lock()
        .unwrap()
        .samples
        .entry(metric)
        .or_default()
        .push(v);
}
pub fn duration(metric: &'static str, v: Duration) {
    value(metric, v.as_secs_f64() * 1e6);
}
pub fn dispatch(
    load: Option<Column>,
    targets: &BTreeSet<ChunkPos>,
    loads: &BTreeMap<Column, Instant>,
    dirty: &BTreeMap<ChunkPos, Instant>,
) {
    let now = Instant::now();
    let mut data = DATA.lock().unwrap();
    data.loads.retain(|(c, t)| loads.get(c) == Some(t));
    data.dirty.retain(|(p, t)| dirty.get(p) == Some(t));
    let mut waits = Vec::new();
    if let Some((c, t)) = load.and_then(|c| loads.get(&c).map(|&t| (c, t))) {
        if data.loads.insert((c, t)) {
            waits.push(now.duration_since(t));
        }
    }
    if load.is_none() {
        for p in targets {
            if let Some(&t) = dirty.get(p) {
                if data.dirty.insert((*p, t)) {
                    waits.push(now.duration_since(t));
                }
            }
        }
    }
    drop(data);
    for wait in waits {
        duration("queue_wait_us", wait);
    }
}
pub struct Install {
    load: Option<Instant>,
    dirty: Vec<Instant>,
    start: Instant,
}
impl Install {
    pub fn new(
        load: Option<Column>,
        targets: &BTreeSet<ChunkPos>,
        loads: &BTreeMap<Column, Instant>,
        dirty: &BTreeMap<ChunkPos, Instant>,
    ) -> Self {
        Self {
            load: load.and_then(|c| loads.get(&c).copied()),
            dirty: targets
                .iter()
                .filter_map(|p| dirty.get(p).copied())
                .collect(),
            start: Instant::now(),
        }
    }
    pub fn ready(&self) {
        if let Some(t) = self.load {
            duration("request_ready_us", t.elapsed());
        }
    }
}
impl Drop for Install {
    fn drop(&mut self) {
        duration("main_install_us", self.start.elapsed());
        if let Some(t) = self.load {
            duration("request_cpu_visible_us", t.elapsed());
        }
        for &t in &self.dirty {
            duration("dirty_cpu_visible_us", t.elapsed());
        }
    }
}
#[cfg(target_os = "linux")]
fn cpu_us() -> f64 {
    #[repr(C)]
    struct Timespec {
        seconds: std::ffi::c_long,
        nanos: std::ffi::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, value: *mut Timespec) -> i32;
    }
    let mut t = Timespec {
        seconds: 0,
        nanos: 0,
    };
    // SAFETY: initialized writable timespec, Linux CLOCK_THREAD_CPUTIME_ID.
    assert_eq!(unsafe { clock_gettime(3, &mut t) }, 0);
    t.seconds as f64 * 1e6 + t.nanos as f64 / 1e3
}
#[cfg(not(target_os = "linux"))]
fn cpu_us() -> f64 {
    f64::NAN
}
pub struct Worker {
    start: Instant,
    cpu: f64,
    heap: usize,
}
impl Worker {
    pub fn new() -> Self {
        let heap = LIVE.load(Ordering::Relaxed);
        PEAK.store(heap, Ordering::Relaxed);
        Self {
            start: Instant::now(),
            cpu: cpu_us(),
            heap,
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let wall = self.start.elapsed();
        let cpu = cpu_us() - self.cpu;
        let heap = PEAK.load(Ordering::Relaxed).saturating_sub(self.heap);
        let (hits, misses) = voxel_core::streaming::slice32_cache_stats();
        duration("worker_wall_us", wall);
        value("worker_cpu_us", cpu);
        value("worker_window_peak_requested_heap", heap as f64);
        value("cache_hits", hits as f64);
        value("cache_misses", misses as f64);
    }
}
pub fn report(scene: &str, radius: i32, repeat: usize) {
    let mut data = DATA.lock().unwrap();
    for (metric, samples) in &mut data.samples {
        for (i, v) in samples.iter().enumerate() {
            println!("S32_SAMPLE,{scene},{radius},{repeat},{metric},{i},{v:.6}");
        }
    }
}
