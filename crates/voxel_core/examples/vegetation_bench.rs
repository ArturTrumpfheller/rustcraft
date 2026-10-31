// SPDX-License-Identifier: MIT OR Apache-2.0
//! Matched vegetation benchmark; use tools/benchmark_vegetation.py for stage timers.
//! Direct runs leave stage columns zero; optional generator argument selects output.
//! Allocation high-water uses requested heap bytes, excludes allocator metadata,
//! stacks and GPU/Bevy. Instrumentation is identical in both baseline/current.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
use voxel_core::{
    BlockPos, ChunkPos, World,
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
        .map(|(_, _, m)| {
            m.positions.len() * 56
                + m.plants.positions.len() * 56
                + m.plants.indices.len() * 4
                + m.indices.len() * 4
                + m.water.positions.len() * 48
                + m.water.indices.len() * 4
        })
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
fn identity(seed: u64, version: u32) -> voxel_core::regions::RegionIdentity {
    voxel_core::regions::RegionIdentity {
        seed,
        generator: version,
        nonce: [27; 16],
    }
}
fn prepare(g: &Generator, c: [i32; 2]) -> World {
    let mut w = World::regional(identity(g.config().seed, g.config().version));
    for z in c[1] - 1..=c[1] + 1 {
        for x in c[0] - 1..=c[0] + 1 {
            w.install_region(
                voxel_core::regions::RegionPos::of(BlockPos([x * 16, 0, z * 16])),
                Default::default(),
            );
            let s = g.sampler([x, z]);
            for y in g.world_height().chunks() {
                let p = ChunkPos([x, y, z]);
                w.insert_chunk(p, s.chunk(p).unwrap());
            }
        }
    }
    w
}
fn main() {
    let version = std::env::args().nth(1).map_or(9, |s| s.parse().unwrap());
    println!(
        "generator,scene,seed,x,z,raw_p50_us,raw_p95_us,raw_p99_us,tree_plan_p50_us,plant_plan_p50_us,cold_job_us,job_p50_us,job_p95_us,job_p99_us,mesh_p50_us,light_p50_us,install_p50_us,vertices,indices,draws,mesh_bytes,resident_bytes,terrain_cache_bytes,water_cache_bytes,vegetation_bytes,peak_worker_heap"
    );
    for (name, seed, x, z) in [
        ("empty-plains", 0, -2560_i32, -4096_i32),
        ("flower-plains", 0, -2432, -4096),
        ("sparse-forest", 0, -1856, -4096),
        ("dense-forest", 0, -2176, -4096),
        ("snow-spruce", 0, -4096, -4096),
        ("desert-shrubs", 2026, 3456, -3968),
        ("rocky", 0, -3776, -4096),
        ("shore", 42, -416, 48),
    ] {
        let c = [x.div_euclid(16), z.div_euclid(16)];
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        let (mut raw, mut trees, mut plants) = (Vec::new(), Vec::new(), Vec::new());
        let mut vegetation_bytes = 0;
        for _ in 0..48 {
            let start = Instant::now();
            let s = g.sampler(c);
            vegetation_bytes = vegetation_bytes.max(s.vegetation_bytes());
            for y in g.world_height().chunks() {
                black_box(s.chunk(ChunkPos([c[0], y, c[1]])));
            }
            raw.push(start.elapsed().as_secs_f64() * 1e6);
            let planning = [0u128; 2]; // PROFILE_TAKE
            trees.push(planning[0] as f64 / 1000.);
            plants.push(planning[1] as f64 / 1000.);
        }
        let mut w = prepare(&g, c);
        let edits = w.overlay_snapshot();
        let (mut jobs, mut installs, mut meshes, mut lighting) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut cache = TerrainCache::default();
        let (mut heap, mut bytes, mut vertices, mut indices, mut draws) = (0, 0, 0, 0, 0);
        let mut cold_job_us = 0.;
        for i in 0..28 {
            let base = reset();
            let start = Instant::now();
            let r = streaming::build_column_cached(
                &g,
                World::default(),
                edits.clone(),
                c,
                0,
                &mut cache,
            );
            let elapsed = start.elapsed().as_secs_f64() * 1e6;
            if i == 0 {
                cold_job_us = elapsed;
            }
            let stage = [0u128; 2]; // STAGE_TAKE
            if i >= 4 {
                jobs.push(elapsed);
                lighting.push(stage[0] as f64 / 1000.);
                meshes.push(stage[1] as f64 / 1000.);
            }
            heap = heap.max(peak(base));
            bytes = mesh_bytes(&r);
            vertices = 0;
            indices = 0;
            draws = 0;
            for (_, _, m) in &r.chunks {
                vertices += m.positions.len() + m.water.positions.len() + m.plants.positions.len();
                indices += m.indices.len() + m.water.indices.len() + m.plants.indices.len();
                draws += usize::from(!m.indices.is_empty())
                    + usize::from(!m.water.indices.is_empty())
                    + usize::from(!m.plants.indices.is_empty());
            }
            let mut install = World::regional(identity(seed, version));
            for (p, e) in edits.regions() {
                install.install_region(p, e);
            }
            let t = Instant::now();
            for (p, ch, _) in r.chunks {
                install.insert_chunk(p, ch);
            }
            if i >= 4 {
                installs.push(t.elapsed().as_secs_f64() * 1e6);
            }
            black_box(install);
        }
        println!(
            "{version},{name},{seed},{x},{z},{:.2},{:.2},{:.2},{:.2},{:.2},{cold_job_us:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{vertices},{indices},{draws},{bytes},{},{},{},{vegetation_bytes},{heap}",
            percentile(&mut raw, 50),
            percentile(&mut raw, 95),
            percentile(&mut raw, 99),
            percentile(&mut trees, 50),
            percentile(&mut plants, 50),
            percentile(&mut jobs, 50),
            percentile(&mut jobs, 95),
            percentile(&mut jobs, 99),
            percentile(&mut meshes, 50),
            percentile(&mut lighting, 50),
            percentile(&mut installs, 50),
            w.allocated_voxel_bytes(),
            cache.allocated_voxel_bytes(),
            g.water_planning_bytes()
        );
        // Exploration/eviction/regeneration use actual cached worker inputs.
        for dx in 0..8 {
            let at = [c[0] + dx, c[1]];
            let r = streaming::build_column_cached(
                &g,
                World::default(),
                edits.clone(),
                at,
                0,
                &mut cache,
            );
            black_box(r);
            w.evict_column(at);
        }
        assert_eq!(
            g.sampler(c).chunk(ChunkPos([c[0], 0, c[1]])),
            g.sampler(c).chunk(ChunkPos([c[0], 0, c[1]]))
        );
    }
    black_box(thread_cpu_us());
}
