# Slice 33 — Rendering and meshing optimization

Draft evidence: final matched repetitions and release validation are still running.

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
3. **Keep retained render entities structurally stable.** Replacing a populated
   surface updates its existing asset through the same handle. Do not remove
   `Aabb` or rewrite unchanged `Visibility`. Bevy's asset-change path updates
   bounds before visibility; real engine-system tests verify the publication
   frame. Worker buffers already move into Bevy attributes without application
   cloning; no additional intermediate copy or content hash was added.
4. **Release empty legacy opaque surfaces.** Both legacy and streamed paths now
   despawn/remove empty surfaces, preserving water and plants where present.
   Handles persist while a surface stays populated; an emptied/refilled surface
   legitimately receives a new entity/handle. This closes a legacy-only hidden
   entity/resource retention case; it does not change streamed world authority.
5. **Bounded F3 diagnostics.** At the existing maximum5Hz visible refresh, report
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

Slice32's explicit bound removal worked: Bevy added a fresh bound before culling.
There was no demonstrated missing-bounds bug. The new path keeps the bound and
uses the existing asset-change update, avoiding redundant ECS commands and
archetype transitions. No custom culling, fixed full-chunk bounds, `NoAutoAabb`,
`NoFrustumCulling` or GPU worker mutation is introduced.

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
edited water, torch cave, dense player construction and exposed cliff. Underground
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

<!-- RESULTS -->

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

<!-- VALIDATION -->

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
