// SPDX-License-Identifier: MIT OR Apache-2.0
//! Matched core-only benchmark. Run in release mode; optional generator argument.
//! Allocation high-water uses requested heap bytes, excludes allocator metadata,
//! stacks and GPU/Bevy. Instrumentation is identical in both baseline/current.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::BTreeSet,
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
use voxel_core::{
    ChunkPos, World,
    regions::EditSnapshot,
    streaming::{self, TerrainCache},
    worldgen::{GenerationConfig, Generator},
};
struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn allocated(n: usize) {
    let live = LIVE.fetch_add(n, Ordering::Relaxed) + n;
    PEAK.fetch_max(live, Ordering::Relaxed);
}
// SAFETY: every operation delegates its unchanged pointer/layout to System;
// counters neither allocate nor touch the allocation contents.
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            allocated(l.size());
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
            allocated(n);
        }
        q
    }
}
#[global_allocator]
static ALLOC: Meter = Meter;
fn reset() -> usize {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    base
}
fn peak(base: usize) -> usize {
    PEAK.load(Ordering::Relaxed).saturating_sub(base)
}
fn percentile(v: &mut [f64], p: usize) -> f64 {
    v.sort_by(f64::total_cmp);
    v[(v.len() - 1) * p / 100]
}
fn mesh_bytes(r: &streaming::ColumnResult) -> usize {
    r.chunks
        .iter()
        .map(|(_, _, m)| m.positions.len() * 56 + m.indices.len() * 4)
        .sum()
}
#[cfg(target_os = "linux")]
fn thread_cpu_us() -> f64 {
    #[repr(C)]
    struct Timespec {
        seconds: i64,
        nanos: i64,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, value: *mut Timespec) -> i32;
    }
    let mut value = Timespec {
        seconds: 0,
        nanos: 0,
    };
    // SAFETY: CLOCK_THREAD_CPUTIME_ID writes one initialized, correctly aligned
    // Linux x86-64 timespec. The pointer is valid for the duration of this call.
    if unsafe { clock_gettime(3, &mut value) } == 0 {
        value.seconds as f64 * 1e6 + value.nanos as f64 / 1000.
    } else {
        f64::NAN
    }
}
#[cfg(not(target_os = "linux"))]
fn thread_cpu_us() -> f64 {
    f64::NAN
}
fn main() {
    let version = std::env::args().nth(1).map_or(3, |s| s.parse().unwrap());
    println!(
        "generator,seed,scene,x,z,setup_ns,raw_p50_us,raw_p95_us,raw_p99_us,lit_p50_us,lit_p95_us,lit_p99_us,mesh_bytes,peak_raw_heap,peak_lit_heap,spawn_us,travel_p50_us,travel_p95_us,travel_p99_us,cache_columns,cache_payload_bytes,travel_peak_heap,plan_bytes,light_p50_us,mesh_p50_us,worker_cpu_p50_us,worker_cpu_p95_us,worker_cpu_p99_us,regen_us"
    );
    for (seed, scenes) in [
        (
            0,
            [
                ("plains", [-170, -256]),
                ("forest", [-16, -256]),
                ("desert", [144, -252]),
                ("snow", [-256, -256]),
                ("highlands", [-232, -256]),
                ("forest_boundary", [-66, -188]),
                ("desert_boundary", [-83, -188]),
            ],
        ),
        (
            42,
            [
                ("plains", [-98, -256]),
                ("forest", [-256, -256]),
                ("desert", [-136, -256]),
                ("snow", [-52, -256]),
                ("highlands", [-232, -256]),
                ("forest_boundary", [-184, -188]),
                ("desert_boundary", [-65, -188]),
            ],
        ),
        (
            2026,
            [
                ("plains", [-222, -256]),
                ("forest", [-74, -256]),
                ("desert", [-116, -256]),
                ("snow", [-238, -256]),
                ("highlands", [-256, -256]),
                ("forest_boundary", [-104, -188]),
                ("desert_boundary", [80, -188]),
            ],
        ),
    ] {
        for (scene, c) in scenes {
            let config = GenerationConfig { seed, version };
            let start = Instant::now();
            for _ in 0..1000 {
                black_box(Generator::new(black_box(config)).unwrap());
            }
            let setup_ns = start.elapsed().as_secs_f64() * 1e9 / 1000.;
            let g = Generator::new(config).unwrap();
            let mut raw = Vec::with_capacity(120);
            let mut lit = Vec::with_capacity(30);
            let mut cpu = Vec::with_capacity(30);
            let mut raw_heap = 0;
            let mut lit_heap = 0;
            let mut bytes = 0;
            for i in 0..123 {
                let base = reset();
                let start = Instant::now();
                let s = g.sampler(c);
                let chunks: [_; 3] =
                    std::array::from_fn(|i| s.chunk(ChunkPos([c[0], i as i32 - 1, c[1]])).unwrap());
                black_box(chunks);
                let elapsed = start.elapsed().as_secs_f64() * 1e6;
                raw_heap = raw_heap.max(peak(base));
                if i >= 3 {
                    raw.push(elapsed);
                }
            }
            for i in 0..33 {
                let base = reset();
                let start = Instant::now();
                let cpu_start = thread_cpu_us();
                let r = streaming::build_column(&g, EditSnapshot::default(), c, 0);
                let cpu_elapsed = thread_cpu_us() - cpu_start;
                let elapsed = start.elapsed().as_secs_f64() * 1e6;
                lit_heap = lit_heap.max(peak(base));
                bytes = mesh_bytes(&r);
                black_box(r);
                if i >= 3 {
                    lit.push(elapsed);
                    cpu.push(cpu_elapsed);
                }
            }
            // Separate generation from light/mesh cost on the same 3x3 source.
            let mut source = World::default();
            for z in c[1] - 1..=c[1] + 1 {
                for x in c[0] - 1..=c[0] + 1 {
                    let sampler = g.sampler([x, z]);
                    for y in -1..2 {
                        let p = ChunkPos([x, y, z]);
                        source.insert_chunk(p, sampler.chunk(p).unwrap());
                    }
                }
            }
            let targets: BTreeSet<_> = (-1..2).map(|y| ChunkPos([c[0], y, c[1]])).collect();
            let mut light_times = Vec::with_capacity(30);
            let mut mesh_times = Vec::with_capacity(30);
            for i in 0..33 {
                let start = Instant::now();
                let light = voxel_core::lighting::Skylight::for_chunks(&source, &targets);
                let light_us = start.elapsed().as_secs_f64() * 1e6;
                let start = Instant::now();
                for &p in &targets {
                    black_box(voxel_core::mesh_chunk_lit(&source, p, &light));
                }
                if i >= 3 {
                    light_times.push(light_us);
                    mesh_times.push(start.elapsed().as_secs_f64() * 1e6);
                }
            }
            // Construct each baseline/new world normally, then time the same
            // three-chunk spawn preparation. No disk or first-run compilation.
            let mut world = World::regional(voxel_core::regions::RegionIdentity {
                seed,
                generator: version,
                nonce: [1; 16],
            });
            let start = Instant::now();
            streaming::prepare_column(&mut world, [0, 0]).unwrap();
            let spawn_us = start.elapsed().as_secs_f64() * 1e6;
            let mut travel = Vec::with_capacity(32);
            let mut cache = TerrainCache::default();
            let mut max_columns = 0;
            let mut travel_heap = 0;
            for step in 0..32 {
                let p = [c[0] + step, c[1]];
                let base = reset();
                let start = Instant::now();
                black_box(streaming::build_column_cached(
                    &g,
                    World::default(),
                    EditSnapshot::default(),
                    p,
                    0,
                    &mut cache,
                ));
                travel.push(start.elapsed().as_secs_f64() * 1e6);
                travel_heap = travel_heap.max(peak(base));
                // Sliding radius-two desired footprint, identical to client policy.
                let desired: BTreeSet<_> = streaming::desired_with_limit(p, 2, g.chunk_limit());
                cache.retain(&desired);
                max_columns = max_columns.max(cache.columns());
            }
            cache.retain(&streaming::desired_with_limit(
                [c[0] + 128, c[1]],
                2,
                g.chunk_limit(),
            ));
            let start = Instant::now();
            black_box(streaming::build_column_cached(
                &g,
                World::default(),
                EditSnapshot::default(),
                c,
                0,
                &mut cache,
            ));
            let regen_us = start.elapsed().as_secs_f64() * 1e6;
            println!(
                "{version},{seed},{scene},{},{},{setup_ns:.1},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{bytes},{raw_heap},{lit_heap},{spawn_us:.2},{:.2},{:.2},{:.2},{max_columns},{},{travel_heap},{},{:.2},{:.2},{:.2},{:.2},{:.2},{regen_us:.2}",
                c[0],
                c[1],
                percentile(&mut raw, 50),
                percentile(&mut raw, 95),
                percentile(&mut raw, 99),
                percentile(&mut lit, 50),
                percentile(&mut lit, 95),
                percentile(&mut lit, 99),
                percentile(&mut travel, 50),
                percentile(&mut travel, 95),
                percentile(&mut travel, 99),
                max_columns * 3 * 4096,
                g.planning_bytes(),
                percentile(&mut light_times, 50),
                percentile(&mut mesh_times, 50),
                percentile(&mut cpu, 50),
                percentile(&mut cpu, 95),
                percentile(&mut cpu, 99)
            );
        }
    }
}
