# Slice 31 — measured voxel processing optimization

Baseline: merged Slice30 `571a728b43bb115604fdb94de0b79b1d92a170fb`.
The production changes are confined to lighting source collection, water mesh
sampling and fluid lifecycle inspection. Generators1–9, saves, registry IDs,
renderer/material/shader contracts, propagation and fluid relaxation rules are
unchanged. No dependency, persistent cache, retained scratch pool, scheduler,
render-distance or visual-quality change is introduced.

These are managed Linux CPU measurements, not performance results from the
owner's i3-13100 / RX9060 XT16GB /32GB DDR5 /1080p240Hz Void Linux system.

## Baseline and methodology

The initial checkout was Slice29. The current Slice30 commit was retrieved through
GitHub because the workspace HTTP proxy refused connections. Every changed file
was applied locally and the resulting Git tree verified against the upstream
`9a8dab9c80d6f82c05d047e56737c6faeaa40a57`; the original signed commit was
reconstructed with its exact upstream SHA. The feature branch starts there.
No historical performance report was edited.

The existing `benchmark_stabilization.py` and client harness now accept an exact
`--baseline`. Their original Slice29 comparison remains available. Disposable
source copies receive identical probes; production functions contain no clocks,
allocation meters, histograms or benchmark branches. Both binaries compile before
measurement. Client test executables are copied aside so the shared build cache
cannot overwrite a binary before it is measured. Each source explicitly rebuilds
`voxel_core`, refreshes local input mtimes and asserts a source fingerprint from
inside the executable before measurement. The fingerprint function exists only
in disposable benchmark source.

**Rejected measurement:** the first client comparison reused the baseline core
through Cargo's shared cache even though source copies differed. Matching water
mesher binary symbols exposed the problem. Those observations are excluded from
all performance claims and retained as [invalid forward](raw/rejected-client/)
and [invalid reverse](raw/rejected-client-reverse/) evidence. Core jobs have
separate target directories and were unaffected. Corrected builds explicitly
recompile core; [binary checks](raw/client-binary-check.txt) distinguish the
original and optimized water routines.

The first unmodified production comparison reproduces the11 Slice30 natural
column workloads, with4 warmups and32 retained samples. A subsequent probe-only
comparison separates lighting span classification, source collection, frontier
seeding, skylight flood and block-light flood. It also separates fluid wake time
from total core installation. The isolated lighting-only experiment records its
own source hashes before the water and lifecycle changes are included.
[Reproduction](raw/reproduction/), [profiling](raw/profile/),
[lighting-only experiment](raw/lighting-only/).

The complete matched workload retains those11 exact seed/coordinate scenes and
adds two edited fixtures. All are complete full-height column jobs:

| Scene | Seed | Chunk X/Z |
| --- | ---: | --- |
| Open Plains |0|-160,-256|
| Dense Forest |0|-136,-256|
| Snowy Plains / Spruce |0|-256,-256|
| Desert |2026|216,-248|
| Rocky Highlands |0|-236,-256|
| Lake shoreline |42|-28,1|
| River |2026|-7,-30|
| Ocean |42|6,6|
| Small cave |42|-4,-9|
| Large cavern |42|3,-4|
| Underground lake |0|-7,-2|
| Player-edited water |42|6,6|
| Torch cave |42|-4,-9|

The edited-water fixture removes the first deterministic below-Y0 target source
and inserts neighboring Stone. The torch fixture inserts a supported torch at
the first deterministic below-Y0 dry cave floor. These repeat exactly in both
builds and exercise authoritative edit overlays; the separate24-sample bucket benchmark
also covers inventory,32 fluid ticks, channel obstruction/reopening, save,
restart and eviction. Water-only jobs reuse the exact returned light caches and
must produce exactly the full job's water buffers. They are timed separately
from the full worker and do not contaminate its time or allocation window.

Worker CPU uses Linux `CLOCK_THREAD_CPUTIME_ID`; worker wall time includes
scheduling. Stage times are wall times inside that worker. Core installation
includes descriptor publication, fluid lifecycle work and returned-mesh disposal.
Client installation means Bevy CPU asset/entity publication, without GPU upload
measurement. Opaque time is combined meshing minus separately timed water and
plant appends, retaining loop/clock overhead. No hardware-counter or sampled-stack results are claimed; this is source-stage
instrumentation plus controlled single-change comparison.

Percentiles use `floor((N-1)*p/100)`. Initial32-sample runs exposed noisy Rocky
worker/opaque P99 and reverse-order torch P99, including raw-generation spikes
in unchanged code. All observations are retained. Larger repeated runs in both
orders investigate those tails; no outlier is silently removed.

## Findings and implemented changes

1. **Lighting reads dominate, rather than flood propagation.** In the probe-only
   baseline, source collection consumes roughly96–99% of light time in the
   natural scenes; span classification costs about4–5µs. Most skylight floods
   are0–10µs. Resolve each chunk's vertical descriptor stack once, borrowing at
   most16 descriptors on the stack. Uniform opaque/transparent slabs fill their
   existing light/solid slices directly. Mixed slabs and uniform emitters retain
   per-cell evaluation. Pending data remains opaque; ready missing legacy data
   retains Air semantics. Source arrays, integer flood algorithm, active-span
   proof and exact applied-light cache representation remain unchanged.
2. **Water repeatedly samples the same chunk through the world map.** Neighbor
   and corner samples inside the current borrowed chunk now read it directly;
   boundary/diagonal samples retain `World::block`. Cache each visible cell's
   two-channel sample and brightness with its existing lazy corner heights.
   Preserve floating-point operation order, face order, winding, UVs, colors and
   every vertex/index. No geometry is merged, dropped or approximated.
3. **Wet installation spends nearly all its time inspecting fluid lifecycle
   dependencies.** The probe-only Ocean baseline spends about606 of609µs inside
   wake processing; underground lake about848 of851µs. Clip loops to the existing
   two-cell vertical/four-cell horizontal neighborhood before visiting cells;
   retain its diamond predicate and Y/Z/X iteration order. Reject dry cells
   before coordinate work and read same-chunk source consumers directly.
   Cross-chunk readiness checks, route invalidation and queue order remain exact.
   Naturally generated source interiors remain quiet.

The isolated lighting-only run already cuts total worker CPU substantially,
with identical peak requested heap. The final wet-stage reductions are attributable
to the water sampling and lifecycle changes, which do not alter generation or
opaque/plant meshing code.

Candidates **not prototyped** after profiling: a different propagation queue
(flooding is a small fraction here), persistent neighbor/halo caches, retained
mesh scratch buffers and additional parallelism. The accepted changes need none
of these memory/ownership tradeoffs. No failed speedup experiment was retained
or presented as an accepted optimization. More advanced surface merging, scheduler
cancellation, culling and distant water belong to later slices.
Horizon source was not needed or copied; no external code attribution is added.

## Exact matched results

Final core runs: **8 warmups +128 samples per scene/build**, forward and reverse,
with no concurrent builds/tests/renderers. Total-worker CPU P50 falls **31.1–57.2%**
across these13 matched scenes and both orders. This is a workload-specific CPU
result, not an FPS estimate. [Machine](raw/machine.txt),
[all forward percentiles/comparisons](raw/confirmed/comparison.csv),
[all reverse comparisons](raw/confirmed-reverse/comparison.csv).
The earlier32-sample [initial](raw/initial/) and [reverse](raw/initial-reverse/)
observations remain available, including their outliers.

Worker CPU **µs**, exact P50 /P95 /P99; reverse column shows P50 before→after:

| Scene | Slice30 | Slice31 | Reverse P50 |
| --- | ---: | ---: | ---: |
|open-plains|1615.686 / 1842.593 / 1924.144|912.804 / 1254.942 / 1412.625|1627.654→822.600|
|dense-forest|1207.651 / 1451.573 / 1528.266|737.464 / 972.092 / 1103.777|1248.702→677.585|
|snow-spruce|1464.762 / 1794.261 / 2622.540|962.908 / 1173.720 / 1231.526|1535.498→937.811|
|desert|1376.982 / 1591.260 / 1795.923|887.116 / 1266.238 / 1754.001|1377.683→795.379|
|rocky|1440.937 / 1661.784 / 1866.879|885.023 / 1145.699 / 1309.482|1480.927→855.289|
|lake-shore|1575.767 / 1849.864 / 1998.614|913.996 / 1479.815 / 1682.586|1725.088→864.141|
|river|1599.432 / 1924.635 / 2125.221|1102.635 / 1341.680 / 1600.263|1731.498→1123.145|
|ocean|1840.239 / 2060.025 / 2162.908|923.629 / 1100.823 / 1187.231|1946.477→946.313|
|small-cave|1590.629 / 1828.772 / 1980.357|956.769 / 1202.403 / 1252.368|1633.873→962.438|
|large-cavern|1688.474 / 1982.230 / 2427.430|1142.554 / 1457.231 / 2706.484|1942.290→1052.480|
|underground-lake|1991.594 / 2324.988 / 2546.747|983.879 / 1348.319 / 1834.571|2029.790→868.237|
|edited-water|1813.980 / 2130.189 / 2259.321|994.195 / 1241.682 / 1331.805|1800.721→938.392|
|torch-cave|1727.572 / 2097.721 / 2241.414|1108.174 / 1316.833 / 1457.522|1673.442→1095.595|

Forward stage **P50 µs**, Slice30→Slice31; both orders and every tail are in
the comparison CSVs above. Worker wall, raw generation, opaque/plant mesh,
water-only, span/frontier/flood and fluid-install metrics are included there.

| Scene | Lighting | Water mesh | Core installation |
| --- | ---: | ---: | ---: |
|open-plains|1144.985→416.537|0.552→0.721|8.133→8.142|
|dense-forest|781.608→251.459|0.531→0.710|8.322→8.433|
|snow-spruce|999.656→472.331|0.560→0.780|8.893→8.894|
|desert|900.516→350.598|0.522→0.730|8.332→8.443|
|rocky|970.392→381.415|0.542→0.710|8.322→8.323|
|lake-shore|958.354→371.710|207.022→87.092|215.615→102.104|
|river|965.414→460.093|32.389→23.707|74.392→66.561|
|ocean|976.321→395.937|452.051→169.124|636.308→217.137|
|small-cave|790.320→397.739|328.815→128.353|434.164→153.341|
|large-cavern|823.771→471.961|399.511→162.546|543.599→193.472|
|underground-lake|894.408→363.268|743.128→220.784|892.364→315.535|
|edited-water|948.959→410.729|460.323→171.518|635.698→220.802|
|torch-cave|841.128→468.927|353.392→140.001|431.971→154.012|

**Tails and small regressions:** the forward large-cavern worker CPU P99 rises
2427.430→2706.484µs; reverse falls5067.455→1432.935µs. Reverse lake-shore
P99 rises2358.107→2609.159µs; forward falls1998.614→1682.586µs. These do
not repeat across order. Unchanged raw generation and opaque paths also show
substantial cloud tail variation. Dry water dispatch P50 grows by roughly
0.02–0.14µs in these runs; no useful dry-water speedup is claimed. River and
some dry-stage P99 values regress in individual runs. All values are retained;
these samples do not prove native frame-time stability.

Memory/geometry invariants are exact across matched samples in both orders.
The following full-worker values are **unchanged**, with no resident growth:

| Scene | Allocation calls | Requested bytes | Peak additional requested heap | Pristine voxel cache bytes |
| --- | ---: | ---: | ---: | ---: |
|open-plains|235|938328|608176|106496|
|dense-forest|185|797256|441520|73728|
|snow-spruce|335|1292904|710836|139264|
|desert|263|1052152|580988|118784|
|rocky|270|882144|536368|102400|
|lake-shore|226|904568|531824|98304|
|river|352|1209064|668860|135168|
|ocean|225|905480|560432|126976|
|small-cave|289|1076112|603316|139264|
|large-cavern|308|1334160|734388|143360|
|underground-lake|244|872880|552240|118784|
|edited-water|230|905696|560432|126976|
|torch-cave|298|1245340|770480|139264|

Every scene also retains exact vertex/index/surface counts, mesh bytes, source
pool-cache bytes, resident voxel bytes and applied-light-cache bytes. They are
listed per sample in the CSVs; the source pool cache is distinct from pristine
voxel cache. Core installation gains do not depend on waking natural water.
All24 player-water samples still process369 cells, finish with0 pending work,
persist3 edits and create0 drops; inventory and overlays survive reload/eviction.

Corrected headless ECS release measurements, with a distinct runtime-asserted
source fingerprint for each core binary: [forward](raw/client/),
[reverse](raw/client-reverse/). Earlier corrected repeats are also retained:
[forward repeat](raw/client-replicate/), [reverse repeat](raw/client-replicate-reverse/).
These include real continuous exploration/eviction and A/B session teardown,
without a renderer or GPU. The following values are generator9.

| Order / build | Visit P50 /P95 /P99 ms | Total worker wall ms | Session switch P50 /P95 /P99 ms |
| --- | ---: | ---: | ---: |
|forward / slice30|8.586 / 28.883 / 28.883|115.083|4.255 / 4.847 / 4.847|
|forward / slice31|7.362 / 24.254 / 24.254|83.431|3.829 / 4.933 / 4.933|
|reverse / slice30|8.599 / 28.722 / 28.722|113.338|4.490 / 4.768 / 4.768|
|reverse / slice31|7.411 / 22.883 / 22.883|85.053|3.781 / 4.548 / 4.548|

Installation and queue latency **µs**, P50 /P95 /P99;45 installs and44 queued
loads per build/order, including initial loading before the eight visits:

| Order / build | Main-thread installation | Queue-to-install |
| --- | ---: | ---: |
|forward / slice30|30.887 / 118.568 / 479.792|9916.139 / 29878.535 / 32427.759|
|forward / slice31|30.676 / 108.784 / 301.475|9346.111 / 25033.633 / 27597.818|
|reverse / slice30|31.557 / 74.091 / 370.258|9706.979 / 28620.060 / 30321.518|
|reverse / slice31|31.147 / 173.832 / 265.681|9405.435 / 24916.232 / 27250.471|

All final v9 runs retain144 descriptors,81,920 voxel bytes,36 mesh assets/
surfaces,7 plant-chunk entities,1,076,568 mesh bytes,25 source-cache columns/
278,528 cache bytes and4 region overlays. Queue peak is24; final pending/
fluid work and discarded jobs are0. Twelve switches return to the exact
menu asset baseline `(2 meshes,2 standard materials,0 terrain materials,
0 images)` with no session entities.

The load/install values include headless polling and CPU publication.
Session switching includes startup/save/drain/cleanup, before a full render
radius must be ready. Session/filesystem and maximum-update results vary
between repeats; report them separately from repeatable warm-worker savings.
[Client percentile table](raw/client-comparison.csv) and complete raw samples
retain these variations, including slower observations. In the earlier corrected
repeat, session P50 was4.248→5.295ms forward and4.348→4.622ms reverse;
the final repetitions above improve instead. These small session/filesystem
samples do not establish a consistent regression or a guaranteed startup gain.

All stage P50/P95/P99 values, CPU/wall distinctions, allocation calls/requested
bytes, logical mesh/cache payloads, propagation counts and authoritative/geometry/
light CRCs are retained in the CSVs. Compare the same scene across builds;
underground names do not mean an isolated underground camera view.

Requested heap excludes allocator headers/fragmentation, stacks, preexisting
caches, Bevy/GPU/driver memory and process RSS. Allocation calls count successful
alloc/realloc requests during the full worker; requested bytes sum their new
sizes. These counters add identical overhead in both builds. Stack descriptor
storage is bounded to16 borrowed entries per current column, with no resident
cache growth. Uniform Air/Stone/WaterSource chunks are never materialized by an
optimization. No allocation-reduction claim is made when the observed counts
remain unchanged.

## Correctness and validation

The original Slice30 collector/flood, water mesher and fluid lifecycle scan are
frozen in test-only reference modules, with no production sharing of the changed
traversal. Existing `full_reference` tests now use that independent full-height
collector instead of the optimized implementation.

- Exact packed sky/block-light fields across13 deterministic legacy/extended and
  biome/water/cave fixtures, followed by Air/Stone/water/plant/torch edits;
  signed horizontal and vertical seams, pending descriptors and uniform emitters.
  Existing cave darkness, day/night independence and dirty-remesh tests remain.
- Exact complete water positions, normals, UVs, colors and indices for uniform,
  mixed, flowing, falling, pending, opaque, plant and torch cases, including
  edited signed seams and cached-light water-only output.
- Exact fluid queue, deduplication set and route-cache equality/order against
  the old scan for streamed/legacy worlds, missing chunks, dry/source/mixed
  chunks, repeated events and prepopulated routes. Uniform source bodies stay
  queue-free and allocation-free. Existing routing, displacement, bucket and
  deferred-boundary tests remain.
- Matched samples check authoritative block CRCs, complete geometry/light CRCs,
  all geometry counts and payloads. Water editing checks the same processed,
  pending, edit and drop counts. Historical generator goldens, worldgen audit,
  regional journal recovery, explicit Air, inventory, stale worker results,
  eviction, all three surface assets and session cleanup remain required checks.

All required local checks pass: format, locked workspace Clippy with warnings
as errors, locked workspace tests, optimized core library, release client build,
texture check, release integration/lifecycle tests, release streaming tests and
explicit optimized AO tests. [Commands and counts](raw/validation.txt),
[machine-readable results](raw/validation.json).

Workspace: **517 passed,6 intentionally ignored benchmark tests**; optimized core:
**317 passed**; release integration/lifecycle: **3 passed**; release streaming:
**30 passed,2 benchmark tests ignored**. The matched harness separately ran the
ignored exploration/session measurements. The45-site three-seed v9 audit is
[byte-identical](raw/audit-slice31.csv) to the committed Slice30 audit, including
all CRCs and zero findings. Historical generator fixtures pass unchanged.
The measured optimized production source hashes for lighting, water and fluid
match the committed implementation exactly. CI results are recorded by the PR;
native GUI/GPU acceptance remains separate.

An initial local validation exhausted the writable disk while linking debug
Bevy gallery/test executables; that is an environment failure, not a test result.
Generated incremental caches were removed and finished executables had debug
symbols stripped. The final workspace test run uses
`CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0` to fit the managed disk, with
unchanged test optimization/assertions. Release validation uses the normal
release optimization level. CI keeps the repository's default profiles.
The initial Clippy warning in one new test was fixed with `is_multiple_of`;
no assertion or production behavior was weakened.

The Linux/Windows workflow retains the explicit optimized AO regression step
and now also runs the complete optimized core library, including the new oracles.
Native Void X11/Wayland, physical Radeon rendering and Windows GUI performance
require owner acceptance; headless tests do not establish those results.

## Reproduction

Run one command at a time after builds/tests finish; do not measure while another
benchmark, compiler, renderer or background stress task is active:

```sh
python3 tools/benchmark_stabilization.py --baseline 571a728b43bb115604fdb94de0b79b1d92a170fb --samples 128 --warmups 8 --output work/slice31-core
python3 tools/benchmark_stabilization.py --baseline 571a728b43bb115604fdb94de0b79b1d92a170fb --samples 128 --warmups 8 --reverse --output work/slice31-core-reverse
python3 tools/benchmark_stabilization_client.py --baseline 571a728b43bb115604fdb94de0b79b1d92a170fb --output work/slice31-client
python3 tools/benchmark_stabilization_client.py --baseline 571a728b43bb115604fdb94de0b79b1d92a170fb --reverse --output work/slice31-client-reverse
```

The CPU benchmark works without Bevy windowing or a GPU. The client benchmark
uses real headless ECS streaming and two saved worlds, rather than substituting
core worker timing for application behavior. It measures eight exploration/
eviction visits at radius1, plus12 A/B session switches. Disposable probes retain
per-install/per-load timings only in benchmark source copies. Neither this small
sample nor the three-seed audit is exhaustive procedural/endurance coverage.

## Native Void Linux glibc / Fish procedure

Use the normal supported Mesa/AMD backend and driver. The old approximately240FPS
measurement predates generator9 and is not a comparison baseline. Build both exact
revisions before measuring and record the complete commit SHAs:

```fish
git fetch origin
git worktree add --detach work/native-slice30 571a728b43bb115604fdb94de0b79b1d92a170fb
cargo build --release --locked -p rustcraft --manifest-path work/native-slice30/Cargo.toml --target-dir work/native-target30
cargo build --release --locked -p rustcraft
set native_root (mktemp -d /tmp/rustcraft-slice31-native.XXXXXX)
mkdir -p "$native_root/base"
env RUSTCRAFT_DATA_DIR="$native_root/base" RUSTCRAFT_LOAD_RADIUS=2 ./work/native-target30/release/rustcraft 2>&1 | tee "$native_root/setup.txt"
# Create a new seed42 world. Set 1920x1080, radius2 and VSync off in Options.
# Close normally before making identical copies, including settings and saves.
cp -a "$native_root/base" "$native_root/run30"
cp -a "$native_root/base" "$native_root/run31"
env RUSTCRAFT_DATA_DIR="$native_root/run30" RUSTCRAFT_START_CHUNK="6,6" RUSTCRAFT_LOAD_RADIUS=2 ./work/native-target30/release/rustcraft 2>&1 | tee "$native_root/slice30.txt"
# Close normally, then run the matched revision:
env RUSTCRAFT_DATA_DIR="$native_root/run31" RUSTCRAFT_START_CHUNK="6,6" RUSTCRAFT_LOAD_RADIUS=2 ./target/release/rustcraft 2>&1 | tee "$native_root/slice31.txt"
```

Select the same copied seed42/generator9 world. Avoid mouse movement for the
stationary case: each new process starts at yaw0/pitch-0.12 radians; spawn search
and the start column are deterministic. Record actual F3 coordinates and initial
view. Match FOV, clock/daylight, weather (none currently), resolution, radius,
VSync, build profile, window/compositor mode and driver/backend. Prepare the
closed base at the same in-game time. Time advances during play, so use the same
warmup/duration and record time in F3; arbitrary long waits change daylight.

Warm for30 seconds, then capture60 seconds stationary and a separate fixed
exploration route. Repeat30→31 then31→30 at least three times, restoring run
copies from the closed base between trials. Repeat the scene survey with the
corresponding seed/start coordinates, radius1/2/3 separately, X11 and Wayland.
No command edits real worlds or changes the supported graphics backend.

If MangoHud is installed, optionally launch either binary with `mangohud` and
identical `MANGOHUD_CONFIG="autostart_log=1,log_duration=60,log_interval=0,output_folder=..."`
(in Fish, use `env MANGOHUD_CONFIG="..." ... mangohud ./target/release/rustcraft`).
Check the installed MangoHud version's CSV cadence; only per-frame samples
support frame-time P50/P95/P99. Record FPS and frame intervals, process RSS and
GPU/backend/driver. Frame interval is presentation pacing, not GPU execution time.
Use a supported GPU profiler for GPU execution time if required; it is not exposed
reliably by RustCraft. `/usr/bin/time -v` can report process maximum RSS when
installed, but includes menus/startup and is not steady-state memory.

Built-in F3 is smoothed. Idle-transition logs show128 recent frame deltas,
job percentiles, main installation/update maxima and queue-to-install maximum;
they are sparse snapshots, not a continuous frame capture or isolated CPU/GPU
frame profile. Native queue/install percentiles and a stall timeline are not
currently exposed in production. Use the matched headless probe for CPU
installation distributions and an external profiler for native main-thread
stalls. Do not infer GPU frame time,240FPS or physical Radeon speedups from
worker CPU times. Test buckets, torch/cave changes, persistence and repeated
world switching on both builds' separate copies, and keep the modified Slice31
copy out of the baseline comparison afterward.

## Remaining work and priorities

- **Slice32:** queue waiting, stale work/cancellation, coherent edit batches,
  worker utilization, request age and resident ownership. Start with the retained
  queue distributions and exploration tails; keep a matched native frame trace.
- **Slice33:** opaque/AO scan cost is now a larger share of worker work. Measure
  rendering draw calls/culling, transparency/fill rate and GPU cost independently
  before changing geometry or renderer organization. Water faces are unchanged.
- **Slice34:** establish native radius1/2/3 CPU/RSS/GPU baselines, then evaluate
  distance budgets/LOD separately. Do not infer scalability from one-column jobs.
- Uniform slab reads still write dense light arrays for the active span; mixed
  chunks still scan their cells. Queue/cache allocation counts are unchanged.
  Retained buffers would require a separate measured memory/latency tradeoff.
- Filesystem save latency, cloud scheduling and thermal/cache effects remain
  noisy. All tails are retained; native endurance and physical-driver behavior
  remain unverified here.
