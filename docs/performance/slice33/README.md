# Slice 33 — Rendering and meshing optimization

Baseline: merged Slice32 `d1d4c699ec07bc7afe1563494e7b4b15719b3e32`.
Rust1.99.1, Bevy0.20.0, generator9, one terrain worker and maximum radius3.
This slice preserves the renderer, shaders, textures, world generation, save
formats, light/fluid rules and connected publication units.

## Findings and accepted changes

The pre-change synthetic profile separates halo construction, visibility-plane
initialization, visibility/AO/key preparation and greedy merge/emission. For an
isolated solid chunk, visibility/AO preparation was93.2µs of115.6µs measured
stages; for a flat slab26.2µs of38.5µs. Mixed/cavity/tree fixtures demonstrate
that greedy emission can also dominate; one isolated timing is insufficient.
[Original baseline stages](raw/baseline-profile/stages.csv).

1. **Exact six-direction visibility masks per X row.** Alongside the unchanged
   18³ Boolean AO halo, retain324 temporary `u32` rows. Six shifts/Boolean
   intersections determine the visible unit faces of16 cells together. Buried
   cubes skip face preparation; only visible faces sample lighting and AO.
   The old per-voxel six-neighbor checks are removed. This relies on the existing
   equality of cube and opaque occupancy, covered across all current block types.
   Bit positions1–16 are local cells; positions0/17 preserve the halo, including
   signed/vertical/diagonal boundaries and pending barriers. No unsafe code.
2. **Uniform halo fills and empty opaque fast path.** Known uniform chunks and
   pending/missing neighbors fill Boolean spans and row bits without thousands
   of repeated block/property lookups. Uniform Air or fluid slabs return after
   water meshing, because they contain no cubes, torches or cutout plants.
   Compact descriptors never acquire a dense voxel allocation.
3. **Release empty legacy opaque surfaces.** Both legacy and streamed paths now
   despawn/remove empty surfaces, preserving water and plants where present.
   Handles persist while a surface stays populated; an emptied/refilled surface
   legitimately receives a new entity/handle. This closes a legacy-only hidden
   entity/resource retention case; it does not change streamed world authority.
4. **Bounded F3 diagnostics.** At the existing maximum5Hz visible refresh, report
   previous completed world-camera visible opaque/water/cutout mesh counts and
   resident triangle counts by surface. The world-camera list excludes UI and
   viewmodel cameras. These are **not draw calls** or submitted triangle counts.

Greedy mask traversal, rectangle compatibility, exact integer affine AO checks,
AO diagonal choice, light channels, texture identities, repeat UVs, normals,
winding and output order remain unchanged. The extra row scratch is1296 bytes
per mesher invocation (bounded stack space), with no persistent cache or heap.
The complete output vectors, allocation counts and logical mesh payload match
baseline; this is a CPU optimization, not a geometry-size reduction.

## Bevy visibility and asset lifecycle

The pinned `bevy_camera`0.20.0 `VisibilityPlugin` orders `CalculateBounds` after
transform propagation and `AssetEventSystems`, and before `CheckVisibility`.
`calculate_bounds` handles both `Without<Aabb>` new entities and existing bounds
selected by `AssetChanged<Mesh3d>`/`Changed<Mesh3d>`. `MeshAabb::get_aabb` uses
`Mesh::final_aabb` when present, otherwise scans Float32x3 positions.

Slice32's explicit bound removal works: Bevy adds a fresh bound before culling.
There is no demonstrated missing-bounds bug. The final implementation retains
that lifecycle. Worker buffers already move into Bevy attributes; populated
surfaces retain their asset handles and entities. No new clone/hash is added.

An attempted optimization kept `Aabb` and used asset-change refresh, saving most
command-application cost. Its matched whole-publication results did not establish
a consistent gain and exhibited large adverse wall-time tails. It was rejected;
see the retained candidate evidence below. The engine source distinguishes its
serial missing-bounds loop from its parallel existing-bounds update. These
observations do not diagnose a general Bevy bug or a native GPU stall.
No custom culling, fixed full-chunk bounds, `NoAutoAabb`, `NoFrustumCulling` or
GPU worker mutation is introduced.

Actual Bevy CPU-system tests cover all three surfaces in the same asset publication
frame, negative/vertical coordinates, expanding/moving geometry, falling water,
plant extents, water-only ownership, empty cleanup, six camera directions, extreme
pitch, near-plane intersection and a32000-block render-origin translation.
Bevy's perspective CPU path intentionally skips the far plane (its intersection
calls pass `false` for far culling). A separate explicit far-plane intersection
rejects the test geometry while Bevy's default visibility retains it. This is
recorded behavior, not a claimed far-culling improvement. Radius3 residency is
far inside the default1000-block projection distance. Floating origin changes
transforms, never local bounds or mesh data; normal transform propagation applies.

One shared opaque, one blended-water and one masked-plant material remain per
session. Material updates for daylight/fog and water UV animation do not remesh.
Water remains double-sided/blended and plants double-sided/alpha-mask. No per-plant
entities, shader changes, extra pipeline variants or cross-chunk mesh batches.

Replacing a `Mesh` still marks the asset modified and can trigger render-world
extraction/preparation/upload. Retaining a handle does not mean avoiding an
upload. Bevy owns interleaving, render extraction and GPU buffer allocation.
CPU attributes remain56 bytes/terrain-or-plant vertex,48/water vertex and4/index.
GPU reserved allocation/staging/driver overhead is not inferred from those sizes.

## Measurement scope and methodology

Run builds/tests first, then benchmarks serially without compiler or renderer
contention. Both exact source exports are built before either measurement.
The core harness copies identical scene/probe code to both exports, checks all
block/light/full-geometry CRCs, counts and payloads, and retains every warm sample.
The client harness checks runtime core/client fingerprints and identical final
world/resource signatures. Source hashes, executable hashes, order and machine
metadata accompany the raw output. Percentiles use index `floor((N-1)*p/100)`.

Core cases: open plains, dense forest, flowering plains, snow/spruce, rocky
highlands, desert, shoreline, ocean, river, small/large cave, underground lake,
edited water, torch cave, dense player construction and exposed cliff. The two
synthetic construction/cliff columns explicitly clear decorative plants before
ground edits through the ordinary public edit API; support validation stays enabled. Underground
column cases still include the complete column; they are not isolated camera views.
Instrumented opaque time excludes separately measured water and plant append
calls but includes scan/key/clock overhead. Worker thread CPU and wall time are
reported separately. Independent AO sampling excludes halo/input preparation;
its microbenchmark is not a whole-worker estimate.

The real headless client runs stationary, walking, sprinting, diagonal/reversing,
teleport/return/eviction, forest/coast/cavern traversal, repeated building/mining,
water edits, radius changes and active world switching at radii1–3. Scripted
240Hz deadlines are pacing for CPU/ECS tests, **not displayed frames**.

A separate retained-install probe publishes1/9/49 changed chunks atomically,
including all three surfaces, using real Bevy asset, transform and visibility
systems. It measures CPU surface conversion/replacement, command application,
ECS/bounds/visibility update, total publication and cleanup separately. Worker
output fixture cloning and alternating1/8-block vertical shifts are outside the
timed intervals. Repeated per-chunk fixture geometry measures CPU lifecycle costs,
not a new coherent generated world or transparency ordering. These batch sizes are
measurement fixtures, not new limits or a radius expansion. Spawn/eviction and
worker-handoff timings also remain in the exploration traces.

No physical GPU/device or reliable hardware GPU profiler is available on this
cloud executor. GPU execution time, actual draw submissions, hardware upload
activity, GPU memory and fragment/overdraw costs were **not measured**. Headless
mesh/visibility/resource counts and CPU update distributions cannot establish an
FPS, native P50/P95/P99 frame-time or Radeon speedup. Rapid rotation/underwater
visual acceptance remains native work, with the reproducible procedure below.

## Matched results

### Core, forward order

Times are µs;128 warm samples/scene/build,8 warmups. All geometry/light/block/resource checks match.

| Scene | Opaque P50 32→33 | Worker CPU P50 32→33 | CPU Δ | Worker wall P99 32→33 |
| --- | ---: | ---: | ---: | ---: |
|open-plains|551.2→202.9|1272.1→869.4|-31.7%|2033.8→1501.0|
|dense-forest|500.1→187.4|1000.1→678.0|-32.2%|2009.1→2025.6|
|snow-spruce|523.9→375.8|1309.2→1163.3|-11.1%|2171.9→2164.8|
|desert|538.5→293.7|1202.3→999.6|-16.9%|2316.1→3088.9|
|rocky|567.8→237.8|1197.6→931.6|-22.2%|2328.9→1600.5|
|lake-shore|458.0→184.8|1161.5→930.7|-19.9%|1712.7→1757.3|
|river|733.2→405.3|1514.6→1225.6|-19.1%|2255.1→2429.9|
|ocean|382.8→181.8|1216.5→1096.1|-9.9%|2186.3→1977.5|
|small-cave|478.8→280.0|1282.5→1132.9|-11.7%|3478.3→1990.2|
|large-cavern|435.4→365.7|1323.7→1315.6|-0.6%|2126.4→1984.5|
|underground-lake|319.6→153.8|1291.2→1119.3|-13.3%|2301.3→1718.8|
|edited-water|388.6→183.2|1310.8→1150.5|-12.2%|2362.2→2199.0|
|torch-cave|521.5→307.5|1450.6→1294.8|-10.7%|3480.4→1980.8|
|flower-plains|601.7→320.2|1136.2→885.0|-22.1%|1938.4→1709.6|
|dense-structure|2672.8→2887.3|3967.4→4137.8|+4.3%|5925.2→7091.6|
|exposed-cliff|702.4→347.1|1673.1→1301.5|-22.2%|2667.8→2195.2|

### Core, reverse order

Times are µs;128 warm samples/scene/build,8 warmups. All geometry/light/block/resource checks match.

| Scene | Opaque P50 32→33 | Worker CPU P50 32→33 | CPU Δ | Worker wall P99 32→33 |
| --- | ---: | ---: | ---: | ---: |
|open-plains|537.8→207.4|1287.1→906.4|-29.6%|2368.5→1886.0|
|dense-forest|504.1→189.1|1000.3→685.3|-31.5%|1710.4→1354.1|
|snow-spruce|526.9→362.2|1291.7→1159.2|-10.3%|2116.2→2412.9|
|desert|592.2→277.8|1290.0→984.0|-23.7%|2965.5→2302.0|
|rocky|561.7→236.9|1178.2→885.6|-24.8%|2432.9→1561.7|
|lake-shore|489.7→187.0|1227.8→917.0|-25.3%|2075.4→2127.4|
|river|712.6→401.7|1496.2→1208.7|-19.2%|3011.1→2914.9|
|ocean|410.3→179.7|1293.1→1040.8|-19.5%|3814.9→1694.0|
|small-cave|484.2→276.6|1252.3→1067.9|-14.7%|2024.9→2475.3|
|large-cavern|427.3→344.7|1285.4→1262.5|-1.8%|2348.9→2921.3|
|underground-lake|316.5→154.3|1240.0→1100.3|-11.3%|2670.9→1678.6|
|edited-water|385.1→188.1|1318.3→1181.5|-10.4%|2153.5→2097.3|
|torch-cave|502.7→298.0|1516.5→1266.1|-16.5%|2955.6→2187.4|
|flower-plains|573.8→327.7|1108.6→919.3|-17.1%|2196.4→2257.6|
|dense-structure|2562.7→2781.0|3792.3→3970.0|+4.7%|6957.5→6599.8|
|exposed-cliff|670.1→338.2|1625.8→1291.2|-20.6%|2722.2→2199.4|

### CPU publication with real Bevy visibility

Three repetitions per build/order;128 retained publications/repetition/case. Values are µs, P50 / P95 / P99.

| Order / changed chunks | Total 32 | Total 33 | Command application 32 | Command application 33 |
| --- | ---: | ---: | ---: | ---: |
|forward / 1|192.26 / 425.41 / 1097.56|206.73 / 417.55 / 656.35|1.55 / 2.35 / 4.70|1.78 / 2.36 / 3.70|
|forward / 9|409.88 / 604.17 / 760.57|434.04 / 767.31 / 1073.93|9.98 / 11.54 / 72.94|9.16 / 11.19 / 77.61|
|forward / 49|1385.03 / 2571.94 / 4788.49|1398.25 / 2446.02 / 3292.62|31.20 / 82.70 / 156.83|37.02 / 71.46 / 135.18|
|reverse / 1|216.28 / 542.39 / 830.08|200.21 / 499.90 / 1423.23|1.91 / 2.50 / 4.60|1.61 / 2.64 / 4.21|
|reverse / 9|445.18 / 1263.53 / 2119.34|410.49 / 764.92 / 1295.33|9.35 / 11.92 / 45.91|9.80 / 11.31 / 38.10|
|reverse / 49|1353.51 / 2472.16 / 4639.31|1358.01 / 2224.74 / 4147.27|36.88 / 116.44 / 193.52|30.98 / 70.58 / 166.50|

All per-surface construction, asset replacement, creation, visibility update, cleanup, exploration, worker handoff, queue latency, memory and resource observations remain in the raw CSVs and compressed complete event logs. No headless update timing is labelled GPU/frame time.


### Interpretation, memory and limits

In both core orders,15/16 scenes have lower worker CPU medians. Natural and cliff
cases generally improve about10–32%; the large cavern changes only0.6–1.8%, so
no meaningful general cavern gain is established. Dense construction is a real
tradeoff: opaque P50 worsens8.0–8.5% and total worker CPU4.3–4.7% in both orders.
The change is retained for the broad measured full-height terrain benefit, with
this adverse workload explicitly carried into native acceptance and Slice34
budgets. It is not a universal mesher speedup or a frame-time guarantee.

The small synthetic mesh-only fixtures also vary: solid124.7→67.7µs, mixed
87.6→92.8µs, trees47.5→70.5µs and the27-chunk stream batch2492.8→2090.6µs.
Their same geometry is retained in the raw profile CSVs. Sparse synthetic trees
must not be mistaken for the full generator9 dense-forest column, whose entire
worker improves in both orders. Greedy/emission costs can rise even though their
semantics are unchanged. No density fallback, additional mesher implementation
or relaxed merge key is introduced to hide these cases.

Isolated packed AO sampling spans about6.5–8.9ns/face baseline and6.3–15.4ns/current
across these fixtures. Halo/input preparation is excluded, repeated sampling and
black-box/loop overhead remain. The AO kernel and visible-face evaluation count
are unchanged; no AO-per-face speedup is claimed. Savings come from occupancy
classification and uniform/opaque-empty preparation. The initial source-identical
13-scene A/A control varied worker medians from−6.6% to+8.0%, illustrating host
variation. It does not cancel the separately repeated dense-construction regression.
The earlier13-scene candidate, including a+6.4% snow/spruce observation, is retained.

All 4096 matched core sample pairs (2048/order) have exactly equal allocation calls,
requested bytes, peak requested heap, geometry/light/block CRCs, vertices, indices,
surface counts and logical mesh payloads. Payloads range44,392–2,944,504 bytes per
column fixture. Plains remain69,440 bytes/236 allocation calls; dense construction
2,944,504 bytes/549 calls. Requested heap excludes allocator metadata, stacks,
preexisting caches and GPU resources; the additional1296 stack bytes are explicit.
See [complete equality checks](raw/core-equivalence.json).

Each final client order checks144 identical final authority/resource signatures
(288 matched signatures across both orders), including repeated world switches.
The 49-chunk installation fixture has147 resident meshes,99 visible and48 culled
by the actual Bevy CPU visibility pass in both builds. Its resident triangle
counts are31,360 opaque,51,940 water and16,856 cutout; these are fixture instance
counts, not renderer submissions or GPU draws.

With the final baseline bounds path, no general publication speedup is claimed.
Worst49-chunk CPU publication is5.60→6.29ms forward and17.09→6.08ms reverse;
small-batch P50/tails also vary. These results preserve the favorable and adverse
observations, and do not establish native frame-time stability. Process-RSS P50
differences across the45 workload/radius groups range−5144..+3240KiB forward and
−4056..+6744KiB reverse. This includes process allocator/probe/cache history; final
mesh/cache/voxel authority signatures remain identical. No RSS reduction is claimed.

Final client counters, process RSS, visibility, resident triangles, asset cleanup,
worker handoff, installation and ECS tails are retained per scene/radius/order.
[Matched client comparison](raw/client-comparison.csv) joins worker CPU, install,
ECS update, handoff, request-to-publication, session and RSS observations with
P50/P95/P99/max and sample counts. Source/executable hashes and complete event
logs accompany the full per-order percentile CSVs.
Resource identity/count/payload equality does not establish a process RSS or GPU
memory reduction. CPU asset conversion and actual GPU preparation/upload remain
separate costs. No native frame-time improvement is inferred from the headless
update/publication distributions.


## Correctness and automated validation

The old merged-Slice32 opaque traversal and its scalar AO implementation are frozen
in test-only reference modules. They share unchanged face definitions and torch
model emission, but no changed halo/visibility code. Compare every output vector
exactly for uniform Air/Stone/water, complex cavities, mixed materials, torches,
plants, flowing/falling water, signed/vertical seams and missing/pending neighbors.
A separate row-mask test checks every face bit against scalar halo occupancy.
Existing independent unit-face, winding, repeat UV, affine AO and light tests stay.

Bevy visibility tests run with its real plugins and asset events, not a simulated
copy of the culler. Additional tests verify F3 world-view selection versus overlay
views, retained populated handles, empty asset/entity release and water-only
surface preservation. Existing streaming fairness, cancellation, stale-result,
atomic seam, generator1–9, save/journal, explicit Air, fluid, inventory, eviction
and session teardown regressions remain required.

All required commands pass, including locked workspace Clippy, workspace tests,
optimized core/AO and focused release streaming/session/scene tests, normal release
client build and original-texture verification. Local full-workspace invocation before the bounds decision:
534 passed plus one subsequently added focused F3 test;6 optional measurements
ignored. After rejecting the bounds attempt, Clippy, focused debug scene tests,
release streaming/session/scene tests and the normal release client build were
repeated on the final production source. Full final-source workspace validation
also runs on both CI platforms. Optimized core:322; integration/lifecycle:3; focused streaming:40;
sessions:11; scene/visibility:10; explicit AO:11. The ignored workload probes are
run separately by the matched harness.

[Ubuntu and Windows CI on production commit f9fe8c1](https://github.com/ArturTrumpfheller/rustcraft/actions/runs/41345842257)
both pass the complete workflow:535 ordinary tests on Linux and532 on Windows
(the difference is Unix-only filesystem tests), plus the retained optimized checks. [Commands/environment/counts](raw/validation.txt),
[machine-readable local results](raw/validation.json), [CI record](raw/ci-production.json).
No Linux result substitutes for native Windows/Void GUI acceptance. Debug example
symbols were stripped to fit scratch space; source/assertions and optimization
profiles were not weakened. Final documentation/evidence commits preserve the
measured production code.

## Reproduction

```sh
python3 tools/profile_meshing.py --revision d1d4c699ec07bc7afe1563494e7b4b15719b3e32 --ao-detail --output work/s33-profile32
python3 tools/profile_meshing.py --revision WORKTREE --ao-detail --output work/s33-profile33
python3 tools/benchmark_stabilization.py --baseline d1d4c699ec07bc7afe1563494e7b4b15719b3e32 --labels slice32 slice33 --rendering-scenarios --samples 128 --warmups 8 --output work/s33-core-forward
python3 tools/benchmark_stabilization.py --baseline d1d4c699ec07bc7afe1563494e7b4b15719b3e32 --labels slice32 slice33 --rendering-scenarios --samples 128 --warmups 8 --reverse --output work/s33-core-reverse
python3 tools/benchmark_streaming.py --baseline d1d4c699ec07bc7afe1563494e7b4b15719b3e32 --labels slice32 slice33 --rendering-install --repeats 3 --frames 120 --output work/s33-client-forward
# Preserve forward observations, reuse exact executables in the opposite order:
cp -a work/s33-client-forward work/s33-client-reverse
python3 tools/benchmark_streaming.py --baseline d1d4c699ec07bc7afe1563494e7b4b15719b3e32 --labels slice32 slice33 --rendering-install --repeats 3 --frames 120 --reverse --reuse --output work/s33-client-reverse
```

The copy retains the exact executables and source IDs; reuse writes reverse logs
into the separate directory. The committed raw directories retain both orders independently. Use a new output directory for each core run.
All worlds in the harness are disposable; no owner save path is opened.

## Native Void Linux glibc / Fish acceptance

Target acceptance machine: i3-13100, RX9060 XT16GB,32GB DDR5,
1920×1080,240Hz, Void glibc/Wayland/MangoWM. Repeat X11 and Windows separately.
Use the normal supported graphics backend; do not force an API or compare to the
historical pre-generator9 ~240FPS observation.

```fish
git fetch origin
mkdir -p work
git worktree add --detach work/s33-native-base d1d4c699ec07bc7afe1563494e7b4b15719b3e32
git worktree add --detach work/s33-native-new origin/feat/slice-33-rendering-meshing-optimization
cargo build --release --locked -p rustcraft --manifest-path work/s33-native-base/Cargo.toml --target-dir work/s33-target32
cargo build --release --locked -p rustcraft --manifest-path work/s33-native-new/Cargo.toml --target-dir work/s33-target33
set native_root (mktemp -d /tmp/rustcraft-slice33.XXXXXX)
env RUSTCRAFT_DATA_DIR="$native_root/closed-base" ./work/s33-target32/release/rustcraft
# Create seed42/generator9 world; set 1920x1080, FOV78, radius2, VSync off.
# Close normally before copying the complete directory, including settings.
cp -a "$native_root/closed-base" "$native_root/run32"
cp -a "$native_root/closed-base" "$native_root/run33"
env RUSTCRAFT_DATA_DIR="$native_root/run32" RUSTCRAFT_START_CHUNK="6,6" RUSTCRAFT_LOAD_RADIUS=2 ./work/s33-target32/release/rustcraft 2>&1 | tee "$native_root/base.log"
env RUSTCRAFT_DATA_DIR="$native_root/run33" RUSTCRAFT_START_CHUNK="6,6" RUSTCRAFT_LOAD_RADIUS=2 ./work/s33-target33/release/rustcraft 2>&1 | tee "$native_root/new.log"
```

Restore fresh run copies from the same **closed** base between trials. Match
seed/generator, player position/yaw/pitch, FOV, world clock/daylight, resolution,
radius, VSync/present mode, backend/driver, warmup, route and capture duration.
Record full commit SHAs, GPU/driver/backend, OS/session and F3 coordinates/time.
New processes start yaw0/pitch-0.12; arbitrary mouse movement invalidates the
stationary comparison. Warm30 seconds, capture60 seconds, repeat at least three
times in32→33 and33→32 order. Never open the owner's actual save directory.

If installed, MangoHud can record per-frame presentation intervals. Use identical
settings, e.g. `env MANGOHUD_CONFIG="autostart_log=1,log_duration=60,log_interval=0,output_folder=$native_root" RUSTCRAFT_DATA_DIR="$native_root/run33" mangohud ./work/s33-target33/release/rustcraft`.
Verify that the installed version actually exports per-frame samples; calculate
P50/P95/P99/max from raw frame intervals and retain every sample. Sampled average
FPS and smoothed F3 values do not supply frame percentiles. Presentation interval
is not GPU execution time. Record process RSS separately (`/usr/bin/time -v`
includes startup/menus and gives process maximum RSS, not steady-state usage).

Use RenderDoc's Launch Application dialog with the exact binary, repository
working directory and disposable `RUSTCRAFT_DATA_DIR`. Capture matched frames
with the same camera/time, inspect actual indexed draw/indirect submissions and
pipeline/material switches for opaque/water/cutout passes, GPU buffer contents,
resource sizes and fragment workload. Do not treat API indirect-call count as
one draw if it executes multiple commands. Use a driver-supported GPU profiler
for execution timing; label capture/profiling overhead and keep normal runs
separate. F3's world-view mesh counts and resident triangles are cross-checks,
not substitutes for capture statistics.

Native checklist (radii1/2/3 separately):

- [ ] Static open/flowering plains, forest, snow/spruce, rocky highlands, desert,
  shoreline/ocean/river, cave and underground lake. Use exact seed/chunk coordinates
  in the core CSVs; record actual camera pose and isolated underground viewpoints.
- [ ] Dense construction and exposed cliffs, exact same copied edited world;
  compare texture repetition, AO corners, torch light and day/night fog.
- [ ] Rapid360° rotations, looking nearly straight up/down, seams and near views;
  no disappearing terrain, water or plants after edits or origin rebasing.
- [ ] Continuous walking/sprinting, diagonal/reversing, repeated eviction and
  regeneration, mining/building, radius1→3→1 during active work. Keep route/time.
- [ ] Buckets/flow/falling water near the player and across seams; swim underwater;
  check heights, colors, transparency, tint, fog and stable retained opaque meshes.
- [ ] Dense grass/flowers: original crossed cutouts, no opaque backgrounds or
  density changes; inspect capture overdraw rather than inferring it from quads.
- [ ] Repeated A→B→A, saving/shutdown while streaming, reopen Air/water/inventory
  edits; no obsolete meshes/items or resource growth. Test X11/Wayland input and
  Windows GUI separately from their CI builds.
- [ ] Record worker/install/update and presentation stalls, per-frameP50/P95/P99/max,
  GPU time if supported, actual draws, visible/culled/resident surfaces, CPU/RSS/GPU
  memory and upload activity. Keep unfavorable observations and state limitations.

## Rejected/deferred changes and Slice 34 plan

The retained-bounds attempt is **not shipped**. With identical fixture geometry,
command application dropped from about1.7/8.8/36.8µs to0.04/0.09/0.24µs for
1/9/49 changed chunks in the forward run, but complete publication medians varied
by order. Worst49-chunk publication increased6.07→35.92ms forward and
6.19→37.59ms reverse; the reverse9-chunk case recorded5.00→182.78ms.
These are CPU/ECS wall intervals, not rendered/GPU frame times. All samples and
source/executable IDs remain in the [forward](raw/rejected-retained-bounds-forward/percentiles.csv)
and [reverse](raw/rejected-retained-bounds-reverse/percentiles.csv) candidate directories. Since
frame-tail stability matters more than an isolated command microbenchmark, final
publication keeps the baseline missing-bounds path. New visibility regressions
prove that path is correct; final accepted-source comparisons are separate.

No giant combined mesh, custom renderer, new occlusion system or GPU upload worker:
no measured GPU/submission bottleneck justifies sacrificing per-chunk culling,
atomic edits and independent eviction. Materials are already shared. No geometry
hash on every output: forced edits and exact light-cache checks already bound work;
no redundant-output cost has been demonstrated to exceed comparison cost.

Packing/quantization and16-bit conversion remain deferred. They require native
upload/shader/compatibility measurements, handle overflow safely, and must retain
exact positions/light/UV behavior. The present change demonstrates CPU savings
with byte-identical output and unchanged vertex/index layouts instead.

Water interior/opaque-neighbor suppression and plant chunk batching already work.
Their shaders/geometry/solver are unchanged. Blended water still sorts per mesh,
with the existing intersecting/chunk transparency limitations; underwater fill
and cutout overdraw require GPU captures. No water LOD/density reduction was used.

For Slice34, first complete the native radius1/2/3 matrix and set explicit CPU
publication, GPU memory and frame-tail budgets. Separate desired render residency
from simulation readiness, retaining collision safety, one bounded worker/cache
and coherent connected publication. Add a single measured radius step behind a
validated cap; measure visible versus resident surfaces, generation/light/mesh
queues, main-thread batch tails and GPU uploads on fixed traversal routes before
increasing it again. Preserve local bounds/floating origin and per-chunk eviction.
Evaluate hierarchical visibility, reduced distant detail or bounded upload budgets
only when those measurements identify the relevant limit. No radius or LOD change
is part of Slice33; native acceptance and owner review precede merging.
