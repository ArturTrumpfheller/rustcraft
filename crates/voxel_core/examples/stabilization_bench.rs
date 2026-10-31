// SPDX-License-Identifier: MIT OR Apache-2.0
//! Matched v9 stabilization benchmark; run through tools/benchmark_stabilization.py.
//! Stage instrumentation is injected only into disposable benchmark copies.
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
    println!(
        "scene,seed,chunk_x,chunk_z,sample,raw_us,worker_us,worker_cpu_us,tree_plan_us,plant_plan_us,light_us,opaque_us,water_us,plant_mesh_us,core_install_us,mesh_bytes,vertices,indices,surfaces,voxel_bytes,cache_bytes,water_cache_bytes,peak_heap,block_crc"
    );
    for (name, seed, c) in [
        ("open-plains", 0, [-160, -256]),
        ("dense-forest", 0, [-136, -256]),
        ("snow-spruce", 0, [-256, -256]),
        ("desert", 2026, [216, -248]),
        ("rocky", 0, [-236, -256]),
        ("lake-shore", 42, [-28, 1]),
        ("river", 2026, [-7, -30]),
        ("ocean", 42, [6, 6]),
        ("small-cave", 42, [-4, -9]),
        ("large-cavern", 42, [3, -4]),
        ("underground-lake", 0, [-7, -2]),
    ] {
        let g = Generator::new(GenerationConfig { seed, version: 9 }).unwrap();
        let world = prepare(&g, c);
        let edits = world.overlay_snapshot();
        let mut cache = TerrainCache::default();
        for i in 0..36 {
            let raw_start = Instant::now();
            let s = g.sampler(c);
            for y in g.world_height().chunks() {
                black_box(s.chunk(ChunkPos([c[0], y, c[1]])));
            }
            let raw = raw_start.elapsed().as_secs_f64() * 1e6;
            let planning = [0u128; 6]; // PROFILE_TAKE
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
            let cpu = thread_cpu_us() - cpu;
            let worker = start.elapsed().as_secs_f64() * 1e6;
            let heap = peak(base);
            let stage = [0u128; 6]; // STAGE_TAKE
            let bytes = mesh_bytes(&r);
            let (mut vertices, mut indices, mut surfaces) = (0, 0, 0);
            let mut crc = crc32fast::Hasher::new();
            for (_, ch, m) in &r.chunks {
                vertices += m.positions.len() + m.water.positions.len() + m.plants.positions.len();
                indices += m.indices.len() + m.water.indices.len() + m.plants.indices.len();
                surfaces += usize::from(!m.indices.is_empty())
                    + usize::from(!m.water.indices.is_empty())
                    + usize::from(!m.plants.indices.is_empty());
                for j in 0..4096 {
                    let lp = voxel_core::LocalPos::new([j % 16, j / 256, j / 16 % 16]).unwrap();
                    crc.update(&(ch.get(lp) as u16).to_le_bytes());
                }
            }
            let mut install = World::regional(identity(seed, 9));
            for (p, e) in edits.regions() {
                install.install_region(p, e);
            }
            let start = Instant::now();
            for (p, ch, _) in r.chunks {
                install.insert_chunk(p, ch);
            }
            let install_us = start.elapsed().as_secs_f64() * 1e6;
            if i >= 4 {
                println!(
                    "{name},{seed},{},{},{},{raw:.3},{worker:.3},{cpu:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{install_us:.3},{bytes},{vertices},{indices},{surfaces},{},{},{},{heap},{}",
                    c[0],
                    c[1],
                    i - 4,
                    planning[0] as f64 / 1000.,
                    planning[1] as f64 / 1000.,
                    stage[2] as f64 / 1000.,
                    stage[3].saturating_sub(stage[4] + stage[5]) as f64 / 1000.,
                    stage[4] as f64 / 1000.,
                    stage[5] as f64 / 1000.,
                    world.allocated_voxel_bytes(),
                    cache.allocated_voxel_bytes(),
                    g.water_planning_bytes(),
                    crc.finalize()
                );
            }
            black_box(install);
        }
    }
}
