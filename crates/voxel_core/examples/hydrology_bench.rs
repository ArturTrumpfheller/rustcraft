// SPDX-License-Identifier: MIT OR Apache-2.0
//! Matched core-only benchmark. Run in release mode; optional generator argument.
//! Allocation high-water uses requested heap bytes, excludes allocator metadata,
//! stacks and GPU/Bevy. Instrumentation is identical in both baseline/current.
use std::{
    alloc::{GlobalAlloc, Layout, System},
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
    let version = std::env::args().nth(1).map_or(8, |s| s.parse().unwrap());
    println!(
        "generator,scene,seed,cx,cz,raw_p50_us,raw_p95_us,raw_p99_us,cold_job_us,job_p50_us,job_p95_us,job_p99_us,cpu_p50_us,install_p50_us,mesh_bytes,mixed,uniform,resident_bytes,terrain_cache_bytes,water_cache_bytes,peak_job_heap,water_cells,initial_queue,idle_processed,edit_peak_queue,edit_processed,edit_changes,settle_ticks,max_changes_tick,save_us,reload_us,edit_records"
    );
    for (name, seed, x, z) in [
        ("dry", 2026, 0i32, 10i32),
        ("pond", 2026, -97, -466),
        ("lake", 42, -434, 31),
        ("ocean", 42, 100, 100),
        ("river", 2026, 107, -243),
        ("coast", 2026, 448, 35),
        ("underground", 2026, -200, -670),
    ] {
        let c = [x.div_euclid(16), z.div_euclid(16)];
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        let mut raw = Vec::new();
        for _ in 0..64 {
            let start = Instant::now();
            let s = g.sampler(c);
            for y in g.world_height().chunks() {
                black_box(s.chunk(ChunkPos([c[0], y, c[1]])));
            }
            raw.push(start.elapsed().as_secs_f64() * 1e6);
        }
        let mut w = prepare(&g, c);
        let edits = w.overlay_snapshot();
        let initial = w.fluid_updates().pending();
        let mut changes = Vec::new();
        let idle = w.fluid_updates().total_processed;
        for _ in 0..120 {
            w.tick_fluids(voxel_core::fluid::FluidConfig::default(), &mut changes);
        }
        let idle = w.fluid_updates().total_processed - idle;
        let (mut jobs, mut cpus, mut installs) = (Vec::new(), Vec::new(), Vec::new());
        let mut cache = TerrainCache::default();
        let (mut cold, mut heap, mut mesh) = (0., 0, 0);
        for i in 0..36 {
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
            let mut installed = World::regional(identity(seed, version));
            for (p, m) in edits.regions() {
                installed.install_region(p, m);
            }
            let start = Instant::now();
            for (p, ch, _) in r.chunks {
                installed.insert_chunk(p, ch);
            }
            let install = start.elapsed().as_secs_f64() * 1e6;
            black_box(installed);
            if i >= 4 {
                jobs.push(us);
                cpus.push(cpu);
                installs.push(install);
            }
        }
        let mixed = w.chunks().filter(|(_, c)| c.uniform().is_none()).count();
        let uniform = w.chunk_count() - mixed;
        let resident = w.allocated_voxel_bytes();
        let water = w.water_cells();
        // Open a supported bed pocket using the ordinary edit path; compare v7
        // at the same coordinates. V8 exercises real generated feeders.
        let bed = if name == "underground" {
            (-60..=-52)
                .find(|y| !w.block(BlockPos([x, *y, z])).is_opaque())
                .unwrap()
                - 1
        } else {
            g.height(x, z).unwrap()
        };
        let p = BlockPos([x, bed, z]);
        w.set_block(p, Block::Air).unwrap();
        let (mut peak_queue, mut ticks, mut changed, mut max_changed) =
            (w.fluid_updates().pending(), 0, 0, 0);
        let before = w.fluid_updates().total_processed;
        while w.fluid_updates().pending() > 0 && ticks < 4096 {
            w.tick_fluids(voxel_core::fluid::FluidConfig::default(), &mut changes);
            peak_queue = peak_queue.max(w.fluid_updates().pending());
            changed += changes.len();
            max_changed = max_changed.max(changes.len());
            ticks += 1;
        }
        assert_eq!(w.fluid_updates().pending(), 0, "edited body must settle");
        let processed = w.fluid_updates().total_processed - before;
        let path = std::env::temp_dir().join(format!(
            "rustcraft-hydro-bench-{}-{version}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        let (store, _, _) = voxel_core::persistence::SaveStore::open_with(&path, || {
            Ok(World::regional(identity(seed, version)))
        })
        .unwrap();
        let inv = store.load_player(&mut w).unwrap();
        let start = Instant::now();
        store.save_player_session(&w, &inv, true).unwrap();
        let save = start.elapsed().as_secs_f64() * 1e6;
        drop(store);
        let start = Instant::now();
        let (store, mut restored, _) =
            voxel_core::persistence::SaveStore::open_existing(&path).unwrap();
        let _ = store.load_player(&mut restored).unwrap();
        let id = restored.region_identity().unwrap();
        for (p, _) in w.overlay_snapshot().regions() {
            restored.install_region(p, store.read_region(id, p).unwrap());
        }
        streaming::prepare_column(&mut restored, c).unwrap();
        let reload = start.elapsed().as_secs_f64() * 1e6;
        assert_eq!(restored.block(p), w.block(p));
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
        println!(
            "{version},{name},{seed},{},{},{:.1},{:.1},{:.1},{cold:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{mesh},{mixed},{uniform},{resident},{},{},{heap},{water},{initial},{idle},{peak_queue},{processed},{changed},{ticks},{max_changed},{save:.1},{reload:.1},{}",
            c[0],
            c[1],
            percentile(&mut raw, 50),
            percentile(&mut raw, 95),
            percentile(&mut raw, 99),
            percentile(&mut jobs, 50),
            percentile(&mut jobs, 95),
            percentile(&mut jobs, 99),
            percentile(&mut cpus, 50),
            percentile(&mut installs, 50),
            cache.allocated_voxel_bytes(),
            g.water_planning_bytes(),
            w.edit_count()
        );
    }
    // Repeated bounded exploration: residency and caches must plateau after
    // forward/back travel; no water propagation can request an extra column.
    for (name, start) in [("river_travel", [6, -16]), ("coast_travel", [28, 2])] {
        let g = Generator::new(GenerationConfig {
            seed: 2026,
            version,
        })
        .unwrap();
        let mut world = World::regional(identity(2026, version));
        let mut cache = TerrainCache::default();
        let mut samples = Vec::new();
        let mut queue_peak = 0;
        for dx in (0..12).chain((0..12).rev()) {
            let center = [start[0] + dx, start[1]];
            let desired = streaming::desired_with_limit(center, 1, g.chunk_limit());
            let evict: std::collections::BTreeSet<_> = world
                .chunks()
                .map(|(p, _)| [p.0[0], p.0[2]])
                .filter(|c| !desired.contains(c))
                .collect();
            for c in evict {
                world.evict_column(c);
            }
            cache.retain(&desired);
            for &c in &desired {
                if world.chunk(ChunkPos([c[0], 0, c[1]])).is_some() {
                    continue;
                }
                let targets = g
                    .world_height()
                    .chunks()
                    .map(|y| ChunkPos([c[0], y, c[1]]))
                    .collect();
                for p in streaming::lighting_regions(&targets) {
                    world.install_region(p, Default::default());
                }
                let start = Instant::now();
                let result = streaming::build_column_cached(
                    &g,
                    streaming::mesh_snapshot(&world, &targets),
                    world.overlay_snapshot(),
                    c,
                    world.revision(),
                    &mut cache,
                );
                samples.push(start.elapsed().as_secs_f64() * 1e6);
                for (p, ch, _) in result.chunks {
                    world.insert_chunk(p, ch);
                }
                queue_peak = queue_peak.max(world.fluid_updates().pending());
            }
            assert!(world.chunk_count() <= 9 * 16);
            assert!(cache.columns() <= 25);
            world.retain_regions(&streaming::lighting_regions(
                &desired
                    .iter()
                    .flat_map(|c| g.world_height().chunks().map(|y| ChunkPos([c[0], y, c[1]])))
                    .collect(),
            ));
        }
        println!(
            "TRAVEL generator={version} scene={name} jobs={} p50_us={:.1} p95_us={:.1} p99_us={:.1} resident={} resident_bytes={} cache_columns={} cache_bytes={} water_cache_bytes={} queue_peak={queue_peak} edits={}",
            samples.len(),
            percentile(&mut samples, 50),
            percentile(&mut samples, 95),
            percentile(&mut samples, 99),
            world.chunk_count(),
            world.allocated_voxel_bytes(),
            cache.columns(),
            cache.allocated_voxel_bytes(),
            g.water_planning_bytes(),
            world.edit_count()
        );
    }
}
