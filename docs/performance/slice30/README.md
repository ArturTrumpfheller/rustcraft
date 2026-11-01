# Slice 30 — integration audit, matched baseline and acceptance

Base: merged Slice29 **f9f027daffae8571ce90358685b7e3eff7c2d122**.
**Generator v9 output did not change. Generators1–9 remain frozen; no v10.**
No dependency, asset, renderer, save envelope, registry ID, record/journal budget
or automatic migration change. The owner accepted Slice29; Slice30 native
acceptance is a separate step. All measurements here are cloud CPU measurements,
not i3-13100 / RX9060 XT results or a promise of native frame rate.

## Audit and findings

The initial internal audit preceded changes. It covered the actual core worldgen
entry point and all planners, world storage, streaming, lighting, fluid relaxation,
opaque/water/plant meshing, regional/player/journal persistence, client streaming,
scene/material/shader and session teardown. Existing deterministic, support,
registry, journal-fault, lighting, stale-result, floating-origin and resource tests
were inspected before adding focused combined coverage.

| Priority | Confirmed defect | Repair and evidence |
| --- | --- | --- |
| Moderate: regression validation | Two optimized AO tests compared equivalent floating-point brightness formulas bit-for-bit. Release main reproduced `0.044478588` vs `0.04447858` (two positive-f32 ULP), while integer lighting and topology agreed. | Only the two affected assertions now allow at most two ULP in finite nonnegative RGB and require exact alpha. All occlusion patterns, visibility, integer levels, greedy equivalence, monotonic strength and documented endpoints remain checked. Production lighting/rendering is unchanged. Linux/Windows CI now run optimized AO checks explicitly. |
| Moderate: diagnostic correctness | `sampled_border_cells` counted only3 slabs per newly generated halo column in extended-height worlds. A v9 nine-column halo reported110,592 instead of589,824 logical cells. | Full, water-only and sparse-result counters use the source world's actual height. A focused v2/v9 regression checks all paths and warm cache reuse. This is logical generated volume, including compact uniform descriptors, not a count of executed per-cell queries. |

No critical corruption, item duplication, procedural determinism or unbounded
residency defect was reproduced in the inspected/tested workloads. This is a
bounded audit, not a proof over every world coordinate or filesystem failure.
Unknown neighboring support overlays correctly defer edits; this is deliberate
fail-closed behavior, not a defect. A pending region can be read without loading
its chunk data. Existing full-inventory and full-drop behavior is preserved.

## Integration coverage and historical compatibility

- Existing v1–v8 golden fixtures remain unchanged. New v9 CRC pins are obtained
  from the exact merged Slice29 source with the same read-only audit harness.
- All released generators1–9 exercise adjacent signed-column generation on two
  workers, reverse request order and regeneration after eviction for seeds0/42/2026.
- The consolidated v9 audit covers45 seed/site combinations, all five biomes,
  full vertical extent, neighboring sampler halos, point/chunk samples, foundation,
  horizontal limits, plant support, nearby species trunks and quiet natural water.
- Combined generators3/6/8/9 lifecycle: edit one side while the other is pending,
  save, load the other region, remove generated tree/plant/Stone, build using new
  materials in older worlds, save a resident-free snapshot, evict, reverse reload,
  restart and verify explicit Air, inventory and unchanged manifest/identity.
- Existing natural-water bucket/construction tests now replay each journal commit
  interruption in both v8 and v9. All older format/unknown-version/registry and
  backup/recovery tests remain. Full drop capacity keeps a flower intact; after
  a slot is freed, retry creates exactly one item. A full inventory cannot collect
  away the capacity constraint.
- A real v9 shoreline ECS test observes opaque/water/cutout surfaces, moves away,
  verifies every former entity and mesh asset is gone, returns and recreates all
  surfaces with bounded cache/light/mesh residency. Existing tests retain coverage
  of plant cutouts, water-only publication, stale worker batches, day/night/torch
  lighting, floating-origin alignment and complete world/session cleanup.

## Bounded read-only world audit

```sh
cargo run --release --locked -p voxel_core --example worldgen_audit
cargo run --release --locked -p voxel_core --example worldgen_audit -- 9 42 -28 1
# Historical local generator, same bounded seed/site survey:
cargo run --release --locked -p voxel_core --example worldgen_audit -- 8
```

Each of seeds **0,42,2026** uses the following chunk X/Z locations (multiply by16
for world-block origins). Names refer to survey locations; actual biomes/counts
are reported per seed, rather than assuming one coordinate has the same biome in
every seed. One site at a time retains at most nine columns /144 descriptors.
At most16 actionable findings are retained per site. The audit opens no save.

| Site | Chunk X/Z |
| --- | --- |
| Open Plains | -160,-256 |
| Flowering Plains | -152,-256 |
| Dense Forest / density transitions | -136,-256 |
| Snow / Spruce | -256,-256 |
| Desert | 216,-248 |
| Rocky Highlands | -236,-256 |
| Lake shoreline | -28,1 |
| River | -7,-30 |
| Ocean / coastline | 6,6 |
| Small cave system | -4,-9 |
| Large cavern | 3,-4 |
| Underground lake | -7,-2 |
| Signed region seam | -4,-1 |
| Negative / positive supported corners | -2048,2047 /2047,-2048 |

Reports: [merged Slice29](raw/audit-slice29.csv) and [Slice30](raw/audit-slice30.csv) are byte-identical for all45 rows. The same harness also audits [270 historical local sites](raw/audit-historical.csv) across generators3–8.
Failures include seed, generator, exact block position, feature, expected/actual
behavior and a single-site reproduction command. Crown checks look for nearby
species trunks in the released crown footprint; they do not prove arbitrary tree
connectivity. The audit does not grade aesthetic irregularities or prove global
cave connectivity. Existing planner topology tests supply complementary coverage.

## Reproducible performance baseline

Run the harnesses **alone**, after ordinary builds/tests finish:

```sh
python3 tools/benchmark_stabilization.py --output work/slice30-core
python3 tools/benchmark_stabilization_client.py --output work/slice30-client
```

The core harness exports exact merged Slice29 and current core into disposable
source copies. Both use generator9, identical seeds/coordinates/settings, source
instrumentation and a requested-heap meter. Both are compiled before measurement.
Each of11 full-height scenes has4 warmups and32 retained samples. Raw generation,
worker wall/CPU, tree/plant planning, light solve, opaque/water/plant meshing and
core installation are recorded. The two cave cases are full-height column jobs,
not isolated camera/GPU views. Percentiles use `floor((N-1)*p/100)`.

Combined mesh timing includes its normal dispatch/scan overhead. Opaque timing is
combined minus water and per-plant append timing; instrumentation overhead remains
in that number and is identical on both builds. Core installation means descriptor
publication, returned-CPU-mesh disposal and fluid lifecycle work, not Bevy mesh installation or GPU upload.
Worker CPU time uses Linux thread CPU clock; wall time also includes scheduling.
Peak heap is additional **requested** bytes during the worker call, excluding
allocator metadata, stacks, preexisting caches and GPU/driver allocations.

Every matched sample verifies identical authoritative block CRC, vertex/index
counts, surface count and mesh/voxel/cache bytes. Comparing different generated
scenes as equivalent workloads is explicitly avoided. A separate24-sample real
bucket/edit/32-fluid-tick/save/reload workload uses seed2026, columns6/-16 and7/-16;
it blocks/reopens a channel and verifies inventory, overlays and no duplicate
items after restart/eviction. A paired vegetation workload removes actual plants
and persists placed vegetation. The client harness uses exact historical/current
production code with identical headless ECS tests for continuous eight-visit
exploration/eviction and twelve A/B world session switches.

Cloud environment: [machine](raw/machine.txt), [method](raw/method.txt),
[source hashes](raw/source-sha256.json), [all percentiles](raw/percentiles.csv),
[Slice29 samples](raw/slice29.csv), [Slice30 samples](raw/slice30.csv).
The exposed EPYC model has a **two-CPU quota /8 GiB RAM**; it is not the owner's CPU.

Full-height v9 columns, milliseconds, **Slice29 → Slice30**:

| Scene | Raw generation P50 | Worker P50 | Worker P95 | Worker P99 |
| --- | ---: | ---: | ---: | ---: |
| open-plains | 0.311 → 0.309 | 2.045 → 1.902 | 2.446 → 2.226 | 2.782 → 2.419 |
| dense-forest | 0.924 → 0.832 | 1.571 → 1.469 | 2.229 → 2.019 | 2.967 → 2.589 |
| snow-spruce | 0.747 → 0.722 | 1.936 → 1.809 | 2.562 → 2.719 | 2.593 → 2.913 |
| desert | 0.406 → 0.433 | 1.759 → 1.808 | 2.176 → 2.489 | 2.564 → 2.501 |
| rocky | 0.507 → 0.501 | 1.802 → 1.728 | 2.215 → 2.364 | 2.360 → 2.416 |
| lake-shore | 0.355 → 0.344 | 1.889 → 1.977 | 2.455 → 2.433 | 3.450 → 2.622 |
| river | 0.496 → 0.483 | 2.007 → 1.899 | 2.457 → 2.333 | 2.667 → 2.358 |
| ocean | 0.352 → 0.368 | 2.065 → 2.402 | 2.815 → 2.894 | 2.836 → 2.922 |
| small-cave | 0.440 → 0.431 | 1.864 → 1.967 | 2.236 → 2.389 | 2.237 → 2.680 |
| large-cavern | 0.528 → 0.490 | 2.209 → 2.090 | 2.704 → 2.893 | 2.710 → 3.521 |
| underground-lake | 0.296 → 0.314 | 2.492 → 2.359 | 2.963 → 3.118 | 3.042 → 3.266 |

Stage P50 in **microseconds**, Slice30; heap is peak additional requested KiB.
These are independent percentile columns, so their medians need not add up.

| Scene | Lighting | Opaque | Water | Plant quads | Core install | Worker heap KiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| open-plains | 1348.08 | 383.81 | 0.60 | 0.00 | 8.25 | 593.92 |
| dense-forest | 880.50 | 367.40 | 0.55 | 6.02 | 8.24 | 431.17 |
| snow-spruce | 1192.12 | 421.00 | 0.61 | 2.08 | 8.97 | 694.18 |
| desert | 1157.78 | 447.21 | 0.66 | 3.27 | 8.45 | 567.37 |
| rocky | 1174.58 | 416.48 | 0.62 | 4.06 | 8.55 | 523.80 |
| lake-shore | 1183.38 | 348.12 | 254.15 | 0.00 | 268.78 | 519.36 |
| river | 1171.79 | 533.23 | 34.10 | 0.00 | 74.70 | 653.18 |
| ocean | 1295.20 | 291.20 | 555.52 | 0.00 | 728.48 | 547.30 |
| small-cave | 953.01 | 385.92 | 415.36 | 0.00 | 468.52 | 589.18 |
| large-cavern | 1039.50 | 365.55 | 458.26 | 0.00 | 597.70 | 717.18 |
| underground-lake | 1152.35 | 251.73 | 814.08 | 0.00 | 975.84 | 539.30 |

Geometry/resource values are **identical in both builds and all32 samples per
scene**. Each voxel/cache column below is a separate nine-column owner; uniform
slabs contribute zero array bytes. Mesh bytes cover only the target column.

| Scene | Vertices / indices | Surfaces | Mesh KiB | Nine-column voxel / pristine-cache KiB |
| --- | ---: | ---: | ---: | ---: |
| open-plains | 1120 / 1680 | 4 | 67.81 | 104 / 104 |
| dense-forest | 1632 / 2448 | 4 | 98.81 | 72 / 72 |
| snow-spruce | 3244 / 4866 | 6 | 196.41 | 136 / 136 |
| desert | 2604 / 3906 | 4 | 157.66 | 116 / 116 |
| rocky | 1716 / 2574 | 5 | 103.90 | 100 / 100 |
| lake-shore | 2268 / 3402 | 4 | 129.54 | 96 / 96 |
| river | 3860 / 5790 | 7 | 232.68 | 132 / 132 |
| ocean | 1832 / 2748 | 4 | 102.92 | 124 / 124 |
| small-cave | 3404 / 5106 | 5 | 198.10 | 136 / 136 |
| large-cavern | 4100 / 6150 | 5 | 240.24 | 140 / 140 |
| underground-lake | 1628 / 2442 | 5 | 90.35 | 116 / 116 |

The small-cave, cavern and underground-lake coordinates can also include surface
water; these are full-height pipeline jobs, not isolated underground-only views.
The lake-shore target itself has no small plants; nearby vegetation is exercised
by the ECS shoreline test and the dedicated vegetation workload. Source-pool
cache sizes and all CRCs are in the CSV. There is no between-build scene change.

Water editing/recovery,24 samples, **P50 /P95 /P99 µs**, Slice29 → Slice30:

| Operation | Slice29 | Slice30 |
| --- | ---: | ---: |
| bucket_us | 16.75 / 17.80 / 18.45 | 16.68 / 21.30 / 27.68 |
| edit_and_32_fluid_ticks_us | 130.50 / 186.56 / 230.33 | 135.35 / 353.81 / 399.94 |
| save_us | 75.09 / 151.70 / 211.56 | 75.85 / 173.03 / 220.21 |
| reload_us | 1422.02 / 1614.37 / 1655.06 | 1352.64 / 1986.32 / 1989.66 |
| evict_reload_us | 1439.12 / 1801.91 / 1814.04 | 1333.45 / 1669.20 / 1929.41 |

Every water sample in both builds processes369 queued cells, settles to0 pending,
persists3 edits and creates0 drops; the captured bucket and final overlay agree
exactly after restart/eviction. [Raw baseline](raw/slice29-water.csv),
[current](raw/slice30-water.csv). This is a32-tick deterministic workload, not a
wall-clock-gated simulation. Vegetation break/save/reload percentiles are retained
in [baseline](raw/slice29-vegetation-edits.csv) and
[current](raw/slice30-vegetation-edits.csv).

Headless release ECS measurements, exact Slice29/current sources, same harness:
[initial baseline](raw/slice29-client.txt), [initial current](raw/slice30-client.txt),
[reverse baseline](raw/reverse/slice29-client.txt),
[reverse current](raw/reverse/slice30-client.txt).
Exploration follows columns48/49/50/51 at Z=-248 and returns to evicted columns,
seed42/radius1; A/B session switching uses two disposable seed2026 worlds.

| Run / build | Exploration visit P50 /P95 /P99 ms | Main install max ms | Queue-to-install max ms | World switch P50 /P95 /P99 ms |
| --- | ---: | ---: | ---: | ---: |
| Initial / Slice29 |9.668 /30.082 /30.082|0.584|38.337|4.131 /5.227 /5.227|
| Initial / Slice30 |9.636 /32.347 /32.347|0.624|43.092|6.164 /6.784 /6.784|
| Reverse / Slice29 |9.892 /34.677 /34.677|0.670|43.774|6.322 /8.000 /8.000|
| Reverse / Slice30 |8.735 /34.452 /34.452|0.556|36.631|4.402 /4.924 /4.924|

All four v9 exploration runs end with144 ready descriptors,81,920 resident voxel
bytes,36 mesh assets/surfaces,7 cutout entities,1,076,568 mesh payload bytes,
25 pristine columns /278,528 cache bytes and4 regional overlays. Peak queued/dirty
requests are24; final pending/fluid work and discarded jobs are0. Each of12 world
switch samples returns to the same asset baseline `(2 meshes,2 standard materials,
0 terrain materials,0 images)` and no session entities. These are observed workload
sizes, not the configured maximum. Main installation is CPU asset/entity publication,
not a measured GPU upload stall. Session switching includes startup/save/worker-drain
and cleanup, before a full render-radius steady state is required.


The initial Ocean worker P50 rose2.065→2.402 ms (16.3%), and Rocky raw P99 rose
0.857→1.970 ms. Both were investigated with a second identical core run in the
**opposite order**. Ocean then measured1.803→1.817 ms (0.8%); Rocky raw P99 was
0.489→0.465 ms. Dense Forest's apparent initial raw-generation improvement also
shrunk from0.924→0.832 ms to0.743→0.740 ms. Session switching changed sign with
order (initial4.131→6.164 ms, reverse6.322→4.402 ms). The large differences did not
repeat and do not establish a code regression or an optimization win. Both runs
retain all observations rather than selecting the faster run. Small sample counts,
thread polling, filesystem caches and cloud host variation limit conclusions;
native repeated profiling is the next phase.

[Reversed core percentiles](raw/reverse/percentiles.csv),
[baseline samples](raw/reverse/slice29.csv), [current samples](raw/reverse/slice30.csv),
[source hashes](raw/reverse/source-sha256.json), [method](raw/reverse/method.txt).
CRC, geometry, cache, pool-cache and peak requested heap values remain exact in
both orders. Repeat on another machine with:

```sh
python3 tools/benchmark_stabilization.py --reverse --output work/slice30-core-reverse
python3 tools/benchmark_stabilization_client.py --reverse --output work/slice30-client-reverse
```


Priorities for the performance phase, supported by these measurements:

- Lighting consumes roughly0.88–1.35 ms of warm-job P50 in these current scenes;
  it remains the largest shared cost. Preserve exact darkness, torch and seam
  requirements when profiling its volume/propagation and allocation work.
- Wet full-height columns add0.25–0.81 ms P50 water meshing. Surface-water scans
  can dominate a nominal underground case; investigate them separately from
  opaque greedy meshing in Slice33.
- Core installation is about8–9 µs in dry scenes but0.27–0.98 ms in wet scenes,
  including returned-mesh disposal and resident fluid lifecycle scans. Pristine
  water still has no queued simulation work. Profile installation scans and
  snapshot ownership before the Slice32 redesign.
- CPU cutout append work is only0–6 µs P50 in these targets. These results do not
  measure cutout fill rate, overdraw or GPU costs. Do not infer a GPU bottleneck
  from CPU work alone.
- Requested worker heap and all measured geometry/cache payloads remain exact
  across builds. No memory regression or resource accumulation is observed in
  these workloads; allocator/driver/native endurance remain separate questions.


Frame time, CPU frame time of a rendered game, GPU frame time, hardware counters,
driver memory and native RX9060 XT performance are **not measured by these CPU
harnesses**. ECS update/install wall times are not substitutes for those metrics.
The owner's i3-13100, RX9060 XT16GB,32GB RAM, Void glibc,1080p/240Hz system needs
its own matched captures after owner acceptance. No arbitrary FPS target is set.

## Validation

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace`:197 client +312 core +3 consolidated integration
  tests passed;6 explicit measurement tests remain ignored in the ordinary run.
- `cargo test --release --locked -p voxel_core --lib`: all312 passed, including
  all optimized AO and historical generator tests.
- `cargo build --release --locked -p rustcraft`: passed.
- `python3 tools/generate_textures.py --check`: passed; no art/atlas changes.
- Read-only audit:45 v9 sites, byte-identical baseline/current reports, plus270
  historical local sites (generators3–8), all without findings.
- Both matched core/interaction benchmarks and both headless ECS measurements
  passed in both execution orders, including their state/resource assertions.
- Actual release GUI smoke under Xvfb/Mesa software Vulkan: create seed42/gen9,
  load distant shoreline, inspect opaque/water/held textures and F3, F5, save to
  title, exit; identity/player files and released lock verified. [Exact scope](raw/gui-smoke.txt).
- Linux and Windows GitHub Actions run the ordinary workspace checks plus the
  optimized AO regression; their live result is attached to the PR checks.
  Headless CI does not replace the owner acceptance checklist below.

The8-GiB cloud scratch disk initially filled while linking large debug gallery
executables alongside release dependencies. Only rebuildable Cargo artifacts were
removed. The final workspace run was serialized with incremental compilation off
and a temporary local rustc wrapper that passes `--strip-debug` to the executable
linker. This removes DWARF only; test code, assertions and optimization settings
remain unchanged. The wrapper is not a project change and CI runs ordinary commands.
The unmodified baseline ordinary workspace run had already passed196 client /310
core tests; its optimized AO run reproduced the two documented failures.


The release AO assertion change is justified by the exact baseline reproduction
above; no golden fixtures or generation algorithms were edited to make tests pass.

## Remaining limitations and next phases

**Performance / streaming / rendering phases31–34:** investigate measured worker
light/mesh/generation costs and allocation tails, main-thread installation and
lifecycle scans, queue latency, dense regional COW/journal captures and distant
rendering. Current residency/radius/cache bounds remain. No LOD, radius expansion
or redesigned worker queue is included in this slice.

**Later worldgen overhaul:** more natural terrain/mountains, cave/entrance variety,
river/coast/ocean geography, biome distribution, underwater detail and additional
materials/species. Current source-filled lakes are not a finite-volume fluid
simulation. Existing per-chunk blended-water ordering and distant cutout shimmer
remain rendering limitations. Leaves remain opaque and do not decay after mining.

**Stability limits:** forced termination can lose edits since the last completed
paired save; redo journals provide save-level recovery, not per-click durability.
External deletion of an unbacked region cannot be distinguished from a never-edited
region. Keep complete closed-directory backups. Eight dirty regions and196,608
records per region intentionally impose edit/fluid backpressure. Audit sampling
cannot establish native compositor/backend behavior, exhaustive procedural quality
or hardware/disk-failure resilience beyond existing injected-failure coverage.

## Void Linux glibc / Fish acceptance

Use the owner's normal graphics driver/backend. Keep real worlds closed and
untouched. Start in a disposable absolute directory:

```fish
git fetch origin
git switch feat/slice-30-worldgen-stabilization
set accept_dir (mktemp -d /tmp/rustcraft-slice30.XXXXXX)
echo $accept_dir
cargo build --release --locked -p rustcraft
env RUSTCRAFT_DATA_DIR="$accept_dir" RUSTCRAFT_LOAD_RADIUS=2 cargo run --release --locked -p rustcraft
# After normal close, reuse the exact same disposable data directory:
env RUSTCRAFT_DATA_DIR="$accept_dir" RUSTCRAFT_LOAD_RADIUS=2 cargo run --release --locked -p rustcraft
# Optional distant shoreline visit, only after closing the previous session:
env RUSTCRAFT_DATA_DIR="$accept_dir" RUSTCRAFT_START_CHUNK="-28,1" RUSTCRAFT_LOAD_RADIUS=2 cargo run --release --locked -p rustcraft
```

1. Create a named world with seed42; confirm generator9 in catalog/F3. Test seeds0
   and2026 in additional disposable worlds. Never reuse a real save for fault tests.
2. Explore Plains, Forest, Snow/Spruce, Desert and Highlands using the survey and
   existing Slice29 exact camera references. Inspect Oak/Birch/Spruce roots/crowns,
   grass/flowers/ferns/bushes and chunk/region seams, including negative coordinates.
3. Swim in natural water; collect a source with a bucket, build into water, dig a
   pocket, block/reopen its channel and wash a plant. Check inventory/drop counts,
   full inventory refusal and eventual pickup without duplicate items.
4. Enter a surface cave and deeper chamber, check collision/headroom, mine Stone,
   place/remove torches, open/close a roof and inspect entrance skylight falloff.
5. Mine generated wood/plants and their supporting terrain; place blocks and new
   vegetation, especially across X/Z±16/±64 and vertical seams. Edits must update
   terrain, water and cutouts without holes, duplicate surfaces or square backgrounds.
6. F5, wait for SAVED, travel beyond residency, return and compare edits; close
   normally and restart. Removed procedural blocks must remain removed unless
   ordinary subsequent water flow legitimately reoccupies a collected source cell.
7. Inspect daylight/night lighting, torch independence, fog, water transparency and
   plant masks. Verify all three surfaces stay aligned through distant-origin rebases.
8. Toggle F3 and VSync, walk continuously at radii1/2/3, cross many chunks, revisit
   evicted columns and record frame/queue behavior. Record backend/driver,1080p,
   settings, seed/coordinates and P50/P95/P99; compare the same scenes/build modes.
9. Switch A→B→A several times; verify inventory, clock and edits stay per-world,
   no old mesh remains and asset/chunk counts stabilize. Exit cleanly, then reopen.
10. Test a **complete closed-directory copy** of a historical world in a separate
    disposable data root/import. Confirm its original generator, edits and inventory;
    do not use an old client on the owner's only world directory.
11. Repeat capture/unlock/focus/window behavior on X11 and Wayland. Windows10/11
    GUI acceptance remains separate from its headless CI checks.
