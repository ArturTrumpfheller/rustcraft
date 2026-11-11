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
static CALLS: AtomicUsize = AtomicUsize::new(0);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn allocated(n: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    REQUESTED.fetch_add(n, Ordering::Relaxed);
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
    CALLS.store(0, Ordering::Relaxed);
    REQUESTED.store(0, Ordering::Relaxed);
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
    let samples: usize = std::env::var("RUSTCRAFT_BENCH_SAMPLES")
        .map_or(Ok(32), |s| s.parse())
        .expect("positive sample count");
    let warmups: usize = std::env::var("RUSTCRAFT_BENCH_WARMUPS")
        .map_or(Ok(4), |s| s.parse())
        .expect("warmup count");
    assert!((1..=10000).contains(&samples) && warmups <= 1000);
    println!(
        "scene,seed,chunk_x,chunk_z,sample,raw_us,worker_us,worker_cpu_us,tree_plan_us,plant_plan_us,light_us,opaque_us,water_us,plant_mesh_us,core_install_us,mesh_bytes,vertices,indices,surfaces,voxel_bytes,cache_bytes,water_cache_bytes,peak_heap,block_crc,span_us,source_us,frontier_us,sky_flood_us,block_flood_us,sky_propagated,block_propagated,light_cells,fluid_install_us,allocation_calls,requested_bytes,geometry_crc,light_crc,water_only_us,light_cache_bytes"
    );
    let mut scenes = vec![
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
        ("edited-water", 42, [6, 6]),
        ("torch-cave", 42, [-4, -9]),
    ];
    if std::env::var_os("RUSTCRAFT_BENCH_RENDERING").is_some() {
        scenes.extend([
            ("flower-plains", 0, [-152, -256]),
            ("dense-structure", 42, [-28, 1]),
            ("exposed-cliff", 0, [-236, -256]),
        ]);
    }
    for (name, seed, c) in scenes {
        let g = Generator::new(GenerationConfig { seed, version: 9 }).unwrap();
        let mut world = prepare(&g, c);
        if name == "edited-water" || name == "torch-cave" {
            let p = (g.world_height().min + 1..0)
                .flat_map(|y| {
                    (0..16).flat_map(move |z| {
                        (0..16).map(move |x| BlockPos([c[0] * 16 + x, y, c[1] * 16 + z]))
                    })
                })
                .find(|&p| {
                    if name == "edited-water" {
                        world.block(p) == voxel_core::Block::WaterSource
                    } else {
                        world.block(p) == voxel_core::Block::Air
                            && world.block(p.offset([0, -1, 0]).unwrap()).is_cube()
                    }
                })
                .expect("deterministic edited fixture");
            if name == "edited-water" {
                world.set_block(p, voxel_core::Block::Air).unwrap();
                world
                    .set_block(p.offset([1, 0, 0]).unwrap(), voxel_core::Block::Stone)
                    .unwrap();
            } else {
                world.set_block(p, voxel_core::Block::Torch).unwrap();
            }
        }
        if name == "dense-structure" || name == "exposed-cliff" {
            // Retain the ordinary support-validation contract: clear existing
            // decorations before replacing their ground in this edited fixture.
            for y in g.world_height().min..g.world_height().max {
                for z in 0..16 {
                    for x in 0..16 {
                        let p = BlockPos([c[0] * 16 + x, y, c[1] * 16 + z]);
                        if world.block(p).is_plant() {
                            world.set_block(p, voxel_core::Block::Air).unwrap();
                        }
                    }
                }
            }
            for y in -32..64 {
                for z in 0..16 {
                    for x in 0..16 {
                        let p = BlockPos([c[0] * 16 + x, y, c[1] * 16 + z]);
                        let b = if name == "exposed-cliff" {
                            if x < 8 {
                                voxel_core::Block::Stone
                            } else {
                                voxel_core::Block::Air
                            }
                        } else if y.rem_euclid(4) == 0 || (x % 4 == 0 && z % 4 == 0) {
                            if y % 8 == 0 {
                                voxel_core::Block::Wood
                            } else {
                                voxel_core::Block::Stone
                            }
                        } else {
                            voxel_core::Block::Air
                        };
                        world.set_block(p, b).unwrap();
                    }
                }
            }
        }
        let edits = world.overlay_snapshot();
        let mut cache = TerrainCache::default();
        for i in 0..warmups + samples {
            let raw_start = Instant::now();
            let s = g.sampler(c);
            for y in g.world_height().chunks() {
                black_box(s.chunk(ChunkPos([c[0], y, c[1]])));
            }
            let raw = raw_start.elapsed().as_secs_f64() * 1e6;
            let planning = [0u128; 16]; // PROFILE_TAKE
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
            let calls = CALLS.load(Ordering::Relaxed);
            let requested = REQUESTED.load(Ordering::Relaxed);
            let stage = [0u128; 16]; // STAGE_TAKE
            let bytes = mesh_bytes(&r);
            let (mut vertices, mut indices, mut surfaces) = (0, 0, 0);
            let mut crc = crc32fast::Hasher::new();
            let mut geometry = crc32fast::Hasher::new();
            let mut lights = crc32fast::Hasher::new();
            for l in r.lights.values() {
                lights.update(&l.0);
            }
            for (_, ch, m) in &r.chunks {
                vertices += m.positions.len() + m.water.positions.len() + m.plants.positions.len();
                indices += m.indices.len() + m.water.indices.len() + m.plants.indices.len();
                surfaces += usize::from(!m.indices.is_empty())
                    + usize::from(!m.water.indices.is_empty())
                    + usize::from(!m.plants.indices.is_empty());
                for values in [
                    &m.positions,
                    &m.normals,
                    &m.water.positions,
                    &m.water.normals,
                    &m.plants.positions,
                    &m.plants.normals,
                ] {
                    for value in values.iter().flatten() {
                        geometry.update(&value.to_bits().to_le_bytes());
                    }
                }
                for values in [
                    &m.uvs,
                    &m.atlas_uvs,
                    &m.water.uvs,
                    &m.plants.uvs,
                    &m.plants.atlas_uvs,
                ] {
                    for value in values.iter().flatten() {
                        geometry.update(&value.to_bits().to_le_bytes());
                    }
                }
                for values in [&m.colors, &m.water.colors, &m.plants.colors] {
                    for value in values.iter().flatten() {
                        geometry.update(&value.to_bits().to_le_bytes());
                    }
                }
                for values in [&m.indices, &m.water.indices, &m.plants.indices] {
                    for value in values {
                        geometry.update(&value.to_le_bytes());
                    }
                }
                for j in 0..4096 {
                    let lp = voxel_core::LocalPos::new([j % 16, j / 256, j / 16 % 16]).unwrap();
                    crc.update(&(ch.get(lp) as u16).to_le_bytes());
                }
            }
            let light_cache_bytes: usize = r.lights.values().map(|l| l.0.len()).sum();
            let water_targets = r.lights.keys().copied().collect();
            let water_start = Instant::now();
            let water_result = streaming::remesh_water(
                &g,
                World::default(),
                edits.clone(),
                &water_targets,
                &mut cache,
                &r.lights,
            );
            let water_only = water_start.elapsed().as_secs_f64() * 1e6;
            for (p, m) in &water_result.meshes {
                assert_eq!(
                    m.water,
                    r.chunks.iter().find(|(cp, _, _)| cp == p).unwrap().2.water
                );
            }
            drop(water_result);
            let mut install = World::regional(identity(seed, 9));
            for (p, e) in edits.regions() {
                install.install_region(p, e);
            }
            let _ = [0u128; 16]; // INSTALL_RESET
            let start = Instant::now();
            for (p, ch, _) in r.chunks {
                install.insert_chunk(p, ch);
            }
            let install_us = start.elapsed().as_secs_f64() * 1e6;
            let installed = [0u128; 16]; // INSTALL_TAKE
            if i >= warmups {
                println!(
                    "{name},{seed},{},{},{},{raw:.3},{worker:.3},{cpu:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{install_us:.3},{bytes},{vertices},{indices},{surfaces},{},{},{},{heap},{},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{:.3},{calls},{requested},{},{},{water_only:.3},{light_cache_bytes}",
                    c[0],
                    c[1],
                    i - warmups,
                    planning[0] as f64 / 1000.,
                    planning[1] as f64 / 1000.,
                    stage[2] as f64 / 1000.,
                    stage[3].saturating_sub(stage[4] + stage[5]) as f64 / 1000.,
                    stage[4] as f64 / 1000.,
                    stage[5] as f64 / 1000.,
                    world.allocated_voxel_bytes(),
                    cache.allocated_voxel_bytes(),
                    g.water_planning_bytes(),
                    crc.finalize(),
                    stage[6] as f64 / 1000.,
                    stage[7] as f64 / 1000.,
                    stage[8] as f64 / 1000.,
                    stage[9] as f64 / 1000.,
                    stage[10] as f64 / 1000.,
                    stage[11],
                    stage[12],
                    stage[13],
                    installed[14] as f64 / 1000.,
                    geometry.finalize(),
                    lights.finalize()
                );
            }
            black_box(install);
        }
    }
}
