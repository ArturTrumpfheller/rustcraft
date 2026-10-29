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
fn id(version: u32) -> voxel_core::regions::RegionIdentity {
    voxel_core::regions::RegionIdentity {
        seed: 42,
        generator: version,
        nonce: [26; 16],
    }
}
fn heights(w: &World) -> std::ops::Range<i32> {
    w.bounds().0[1] / 16..w.bounds().1[1] / 16
}
fn prepare(version: u32) -> World {
    let mut w = World::regional(id(version));
    for x in -1..=1 {
        for z in -1..=1 {
            w.install_region(
                voxel_core::regions::RegionPos::of(BlockPos([x * 16, 0, z * 16])),
                Default::default(),
            );
            streaming::prepare_column(&mut w, [x, z]).unwrap();
        }
    }
    w
}
fn change(w: &mut World, p: [i32; 3], b: Block) {
    w.set_block(BlockPos(p), b).unwrap();
}
fn main() {
    let version: u32 = std::env::args().nth(1).map_or(6, |s| s.parse().unwrap());
    println!(
        "generator,scene,ready_descriptors,nonempty_chunks,voxel_bytes,cache_columns,cache_voxel_bytes,mesh_bytes,light_cells,light_us,job_p50_us,job_p95_us,job_p99_us,worker_cpu_p50_us,install_p50_us,peak_job_heap,spawn_us,fluid_us,fluid_queue"
    );
    for scene in [
        "surface",
        "empty_upper",
        "deep_mined",
        "tower",
        "vertical_water",
        "heavy_column",
    ] {
        let mut w = prepare(version);
        let (lo, hi) = (w.bounds().0[1], w.bounds().1[1]);
        if scene == "deep_mined" {
            for y in lo + 3..12 {
                for x in 6..10 {
                    for z in 6..10 {
                        change(&mut w, [x, y, z], Block::Air);
                    }
                }
            }
            change(&mut w, [8, lo + 2, 8], Block::Stone);
            change(&mut w, [8, lo + 3, 8], Block::Torch);
        }
        if scene == "tower" {
            for y in 16..hi - 8 {
                for x in 6..10 {
                    for z in 6..10 {
                        if x == 6 || x == 9 || z == 6 || z == 9 {
                            change(&mut w, [x, y, z], Block::Wood);
                        }
                    }
                }
            }
        }
        if scene == "heavy_column" {
            for y in lo..hi {
                for x in 0..16 {
                    for z in 0..16 {
                        change(
                            &mut w,
                            [x, y, z],
                            if (x + y + z).rem_euclid(2) == 0 {
                                Block::Stone
                            } else {
                                Block::Air
                            },
                        );
                    }
                }
            }
        }
        let mut fluid_us = 0.;
        if scene == "vertical_water" {
            for y in lo..hi {
                change(&mut w, [8, y, 8], Block::Air);
                for [x, z] in [[7, 8], [9, 8], [8, 7], [8, 9]] {
                    change(&mut w, [x, y, z], Block::Stone);
                }
            }
            change(&mut w, [8, hi - 1, 8], Block::WaterSource);
            let start = Instant::now();
            let mut changes = Vec::new();
            for _ in 0..2000 {
                w.tick_fluids(Default::default(), &mut changes);
                if w.fluid_updates().pending() == 0 {
                    break;
                }
            }
            fluid_us = start.elapsed().as_secs_f64() * 1e6;
        }
        let g = Generator::new(GenerationConfig { seed: 42, version }).unwrap();
        let mut cache = TerrainCache::default();
        let edits = w.overlay_snapshot();
        let mut jobs = Vec::new();
        let mut cpus = Vec::new();
        let mut installs = Vec::new();
        let mut heap = 0;
        let mut mesh = 0;
        let mut cells = 0;
        for i in 0..33 {
            let base = reset();
            let start = Instant::now();
            let cpu = thread_cpu_us();
            let r = streaming::build_column_cached(
                &g,
                World::default(),
                edits.clone(),
                [0, 0],
                0,
                &mut cache,
            );
            let elapsed = start.elapsed().as_secs_f64() * 1e6;
            let cpu = thread_cpu_us() - cpu;
            heap = heap.max(peak(base));
            mesh = mesh_bytes(&r);
            cells = r.lighting.cells;
            let start = Instant::now();
            let mut installed = World::regional(id(version));
            installed.install_region(
                voxel_core::regions::RegionPos([0, 0]),
                edits.regions().find(|(p, _)| p.0 == [0, 0]).unwrap().1,
            );
            for (p, c, _) in r.chunks {
                installed.insert_chunk(p, c);
            }
            let install = start.elapsed().as_secs_f64() * 1e6;
            black_box(installed);
            if i >= 3 {
                jobs.push(elapsed);
                cpus.push(cpu);
                installs.push(install);
            }
        }
        let targets: BTreeSet<_> = heights(&w).map(|y| ChunkPos([0, y, 0])).collect();
        let mut lights = Vec::new();
        for _ in 0..30 {
            let start = Instant::now();
            black_box(voxel_core::lighting::Skylight::for_chunks(&w, &targets));
            lights.push(start.elapsed().as_secs_f64() * 1e6);
        }
        let start = Instant::now();
        let mut spawn = World::regional(id(version));
        streaming::prepare_spawn(&mut spawn).unwrap();
        let spawn_us = start.elapsed().as_secs_f64() * 1e6;
        if scene == "empty_upper" {
            let upper = BTreeSet::from([ChunkPos([0, if version == 6 { 8 } else { 1 }, 0])]);
            let r = streaming::remesh_lit(
                &g,
                streaming::mesh_snapshot(&w, &upper),
                edits,
                &upper,
                &mut cache,
                &Default::default(),
                &upper,
            );
            println!(
                "UPPER_PROBE generator={version} lights={} cells={} meshes={} elapsed_us={:.2}",
                r.lights.len(),
                r.lighting.cells,
                r.meshes
                    .iter()
                    .filter(|(_, m)| !m.indices.is_empty())
                    .count(),
                r.elapsed.as_secs_f64() * 1e6
            );
        }
        let ready = w.chunks().count();
        let nonempty = w.nonempty_chunk_count();
        let voxels = w.allocated_voxel_bytes();
        let cache_bytes = cache.allocated_voxel_bytes();
        println!(
            "{version},{scene},{ready},{nonempty},{voxels},{},{cache_bytes},{mesh},{cells},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{heap},{spawn_us:.2},{fluid_us:.2},{}",
            cache.columns(),
            percentile(&mut lights, 50),
            percentile(&mut jobs, 50),
            percentile(&mut jobs, 95),
            percentile(&mut jobs, 99),
            percentile(&mut cpus, 50),
            percentile(&mut installs, 50),
            w.fluid_updates().pending()
        );
    }
    let g = Generator::new(GenerationConfig { seed: 42, version }).unwrap();
    let mut cache = TerrainCache::default();
    let mut times = Vec::new();
    let mut peak_columns = 0;
    for x in 0..64 {
        let start = Instant::now();
        black_box(streaming::build_column_cached(
            &g,
            World::default(),
            Default::default(),
            [x, 0],
            0,
            &mut cache,
        ));
        times.push(start.elapsed().as_secs_f64() * 1e6);
        cache.retain(&streaming::desired_with_limit([x, 0], 2, g.chunk_limit()));
        peak_columns = peak_columns.max(cache.columns());
    }
    println!(
        "TRAVEL generator={version} p50_us={:.2} p95_us={:.2} p99_us={:.2} peak_cache_columns={peak_columns} cache_voxel_bytes={}",
        percentile(&mut times, 50),
        percentile(&mut times, 95),
        percentile(&mut times, 99),
        cache.allocated_voxel_bytes()
    );
    // Isolated normal save/reopen, including inventory and both extreme edit heights.
    let path = std::env::temp_dir().join(format!(
        "rustcraft-vertical-bench-{}-{version}",
        std::process::id()
    ));
    assert!(!path.exists());
    let (mut store, mut w, _) =
        voxel_core::persistence::SaveStore::open_with(&path, || Ok(World::regional(id(version))))
            .unwrap();
    let mut inventory = store.load_player(&mut w).unwrap();
    store.prepare_regional_spawn(&mut w).unwrap();
    streaming::prepare_spawn(&mut w).unwrap();
    let mut saves = Vec::new();
    let mut reloads = Vec::new();
    let low = w.bounds().0[1] + 1;
    let high = w.bounds().1[1] - 1;
    for n in 0..20 {
        change(
            &mut w,
            [8, low, 8],
            if n % 2 == 0 { Block::Air } else { Block::Dirt },
        );
        change(
            &mut w,
            [8, high, 8],
            if n % 2 == 0 {
                Block::Wood
            } else {
                Block::WaterSource
            },
        );
        inventory.select(n % 9);
        let start = Instant::now();
        store.save_player_session(&w, &inventory, true).unwrap();
        saves.push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        drop(store);
        let (reopened, mut loaded, _) =
            voxel_core::persistence::SaveStore::open_with(&path, || {
                panic!("never reinitialize existing benchmark world")
            })
            .unwrap();
        let got = reopened.load_player(&mut loaded).unwrap();
        reopened.prepare_regional_spawn(&mut loaded).unwrap();
        streaming::prepare_spawn(&mut loaded).unwrap();
        reloads.push(start.elapsed().as_secs_f64() * 1e6);
        assert_eq!(got.slots(), inventory.slots());
        assert_eq!(got.selected(), inventory.selected());
        assert_eq!(
            loaded.block(BlockPos([8, low, 8])),
            w.block(BlockPos([8, low, 8]))
        );
        assert_eq!(
            loaded.block(BlockPos([8, high, 8])),
            w.block(BlockPos([8, high, 8]))
        );
        store = reopened;
        w = loaded;
    }
    println!(
        "SAVE_RELOAD generator={version} edits={} save_p50_us={:.2} save_p95_us={:.2} save_p99_us={:.2} reload_p50_us={:.2} reload_p95_us={:.2} reload_p99_us={:.2}",
        w.edit_count(),
        percentile(&mut saves, 50),
        percentile(&mut saves, 95),
        percentile(&mut saves, 99),
        percentile(&mut reloads, 50),
        percentile(&mut reloads, 95),
        percentile(&mut reloads, 99)
    );
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}
