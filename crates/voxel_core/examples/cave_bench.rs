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
    Block, BlockPos, ChunkPos, World,
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
            streaming::prepare_column(&mut w, [x, z]).unwrap();
        }
    }
    w
}
fn main() {
    let version = std::env::args().nth(1).map_or(7, |s| s.parse().unwrap());
    println!(
        "generator,scene,seed,cx,cz,cy,raw_column_p50_us,raw_chunk_p50_us,cold_job_us,job_p50_us,job_p95_us,job_p99_us,thread_cpu_p50_us,light_p50_us,mesh_p50_us,mesh_bytes,resident_mixed,resident_uniform_stone,resident_voxel_bytes,cache_bytes,peak_requested_job_heap,core_install_p50_us"
    );
    for (name, seed, c, cy, torch) in [
        ("surface", 42, [0, 0], 0, false),
        ("same_content", 42, [65, 65], 0, false),
        ("solid_underground", 42, [128, 128], -4, false),
        ("narrow_tunnel", 42, [-4, -9], -2, false),
        ("medium_chamber", 0, [-4, -8], -2, false),
        ("large_cavern", 42, [3, -5], -2, false),
        ("intersection", 42, [-5, -9], -2, false),
        ("vertical_connection", 42, [-11, -10], -3, false),
        ("large_cavern_torch", 42, [3, -5], -2, true),
    ] {
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        let mut raw = Vec::new();
        let mut raw_chunk = Vec::new();
        for _ in 0..32 {
            let start = Instant::now();
            let s = g.sampler(c);
            for y in g.world_height().chunks() {
                black_box(s.chunk(ChunkPos([c[0], y, c[1]])));
            }
            raw.push(start.elapsed().as_secs_f64() * 1e6);
            let start = Instant::now();
            black_box(s.chunk(ChunkPos([c[0], cy, c[1]])));
            raw_chunk.push(start.elapsed().as_secs_f64() * 1e6);
        }
        if name == "solid_underground" {
            assert_eq!(
                g.chunk(ChunkPos([c[0], cy, c[1]])).unwrap().uniform(),
                Some(Block::Stone)
            );
        }
        let mut w = prepare(&g, c);
        if torch {
            let floor = (-59..-30)
                .rev()
                .find(|&y| w.block(BlockPos([51, y, -66])).is_opaque())
                .unwrap();
            w.set_block(BlockPos([51, floor + 1, -66]), Block::Torch)
                .unwrap();
        }
        let edits = w.overlay_snapshot();
        let mut cache = TerrainCache::default();
        let mut jobs = Vec::new();
        let mut cpus = Vec::new();
        let mut installs = Vec::new();
        let mut heap = 0;
        let mut mesh = 0;
        let mut cold = 0.;
        for i in 0..101 {
            let base = reset();
            let start = Instant::now();
            let cpu = thread_cpu_us();
            let r = streaming::build_column_cached(
                &g,
                World::default(),
                edits.clone(),
                c,
                0,
                &mut cache,
            );
            let us = start.elapsed().as_secs_f64() * 1e6;
            let cpu = thread_cpu_us() - cpu;
            if i == 0 {
                cold = us;
            }
            heap = heap.max(peak(base));
            mesh = mesh_bytes(&r);
            let start = Instant::now();
            let mut installed = World::regional(identity(seed, version));
            for (p, m) in edits.regions() {
                installed.install_region(p, m);
            }
            for (p, ch, _) in r.chunks {
                installed.insert_chunk(p, ch);
            }
            let install = start.elapsed().as_secs_f64() * 1e6;
            black_box(installed);
            if i >= 5 {
                jobs.push(us);
                cpus.push(cpu);
                installs.push(install);
            }
        }
        let targets: BTreeSet<_> = g
            .world_height()
            .chunks()
            .map(|y| ChunkPos([c[0], y, c[1]]))
            .collect();
        let mut lights = Vec::new();
        let mut meshes = Vec::new();
        for _ in 0..32 {
            let start = Instant::now();
            let l = voxel_core::lighting::Skylight::for_chunks(&w, &targets);
            lights.push(start.elapsed().as_secs_f64() * 1e6);
            let start = Instant::now();
            for &p in &targets {
                black_box(voxel_core::mesh_chunk_lit(&w, p, &l));
            }
            meshes.push(start.elapsed().as_secs_f64() * 1e6);
        }
        let mixed = w.chunks().filter(|(_, c)| c.uniform().is_none()).count();
        let stone = w
            .chunks()
            .filter(|(_, c)| c.uniform() == Some(Block::Stone))
            .count();
        println!(
            "{version},{name},{seed},{},{},{cy},{:.2},{:.2},{cold:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{mesh},{mixed},{stone},{},{},{heap},{:.2}",
            c[0],
            c[1],
            percentile(&mut raw, 50),
            percentile(&mut raw_chunk, 50),
            percentile(&mut jobs, 50),
            percentile(&mut jobs, 95),
            percentile(&mut jobs, 99),
            percentile(&mut cpus, 50),
            percentile(&mut lights, 50),
            percentile(&mut meshes, 50),
            w.allocated_voxel_bytes(),
            cache.allocated_voxel_bytes(),
            percentile(&mut installs, 50)
        );
    }
    for (name, z) in [("above_ground", 0), ("underground", -5)] {
        let g = Generator::new(GenerationConfig { seed: 42, version }).unwrap();
        let mut cache = TerrainCache::default();
        let mut times = Vec::new();
        let mut peak_columns = 0;
        for x in -32..32 {
            let start = Instant::now();
            black_box(streaming::build_column_cached(
                &g,
                World::default(),
                Default::default(),
                [x, z],
                0,
                &mut cache,
            ));
            times.push(start.elapsed().as_secs_f64() * 1e6);
            cache.retain(&streaming::desired_with_limit([x, z], 2, g.chunk_limit()));
            peak_columns = peak_columns.max(cache.columns());
        }
        let start = Instant::now();
        let a = streaming::build_column_cached(
            &g,
            World::default(),
            Default::default(),
            [-32, z],
            0,
            &mut cache,
        );
        let regeneration = start.elapsed().as_secs_f64() * 1e6;
        assert_eq!(
            a.chunks,
            streaming::build_column(
                &g,
                voxel_core::regions::EditSnapshot::default(),
                [-32, z],
                0
            )
            .chunks
        );
        println!(
            "TRAVEL generator={version} path={name} p50_us={:.2} p95_us={:.2} p99_us={:.2} peak_cache_columns={peak_columns} cache_bytes={} eviction_regeneration_us={regeneration:.2}",
            percentile(&mut times, 50),
            percentile(&mut times, 95),
            percentile(&mut times, 99),
            cache.allocated_voxel_bytes()
        );
    }
}
