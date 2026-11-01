# Slice 32 — Chunk Streaming v2

Baseline: merged Slice31 `1b7e405` (generator9). One terrain worker, radius1–3
and all existing generators, block/item IDs, shaders, save formats, fluid/light
rules and collision semantics remain. There is no worker pool or LOD.

## Findings and scope

The Slice31 column benchmark was reproduced before changing scheduling, using
its existing source probes and exact Slice30 reference. The lower worker CPU
cost reproduced, with substantial host-dependent tails. These measurements are
not evidence of a scheduler gain. The original client reproduction build ran
out of scratch disk before measuring; the new harness subsequently builds the
exact merged Slice31 client rather than approximating its behavior.

The previous scheduler always selected connected dirty work before any load.
Under continuous invalidation this has no finite load-service bound. A worker
also completed every regional read, source generation, light solve and mesh,
even after its output became obsolete. The main thread only rejected it at
completion. Ordinary warm column CPU remains small; a pool would add duplicated
halos, memory and installation conflicts without addressing this priority rule.

The implementation targets service fairness and avoidable obsolete work. It does
not redesign Bevy mesh installation or split a connected light/geometry batch.
Large components can still delay the next job; preemption occurs at safe worker
boundaries, not by publishing a component in pieces.

## Scheduling policy

`client::streaming::policy::Scheduler` receives explicit queues, request times,
player state and a clock, making decisions repeatable for equal inputs.

- Only one job runs. If both classes are eligible, at most two consecutive jobs
  come from either loads or dirty components. Normal preference remains dirty
  work; after two dirty dispatches a load must run. Collision-urgent loads can
  take two turns, then an eligible dirty component gets service.
- Missing columns intersecting a horizontal0.6-block player envelope get the
  first load priority. This covers the0.3-block body half width plus the next
  fixed sprint step. It changes ordering, never physics or residency.
- Other loads use nearest Chebyshev rings, then the existing bounded directional
  preference inside the ring. Nonfinite/tiny velocity has no directional bias.
  Speed never promotes distant terrain over the safety envelope.
- Requests waiting250ms precede newer requests inside their nearest ring.
  Every fourth load service may select the oldest eligible request outside that
  ring, unless a collision-urgent column is missing. Equal timestamps use signed
  coordinate order. This explicit aging slot is the only far-before-near exception.
- Every fourth dirty service may start from the oldest250ms invalidation;
  otherwise the nearest3D seed is used. The entire six-connected component is
  selected, including light-only, AO/opaque and water invalidations. A failed
  member blocks the entire component during backoff, while other components
  and loads can proceed.
- First request/invalidation times survive coalescing, revision retries and
  cancellation. Evicted requests lose their age. Dispatches, including attempts
  later stopped, count toward class fairness.

For a persistently desired, eligible request whose jobs can finish, these rules
provide eventual service. They cannot promise completion under an infinite stream
of superseding edits, permanent I/O failure or continuous eviction. Aging bounds
service turns after the time threshold; it is not a wall-time deadline for a large
component. Safety/proximity are the usual order, with an explicit bounded aging
exception rather than an unrestricted velocity prediction.

The actual desired square remains centered on the player. There is no predicted
residency center, velocity extrapolation, hysteretic unloading or retained visited
world cache. Existing modest motion bias is retained; the measurements do not
establish a benefit from an additional prediction mechanism.

## Cooperative stop and ownership

Each job owns its immutable authoritative snapshot, required regional overlays,
previous exact light, forced geometry and `TerrainCache`. The main thread shares
only a safe `Arc<AtomicBool>` stop token. It requests a stop when any target is
no longer desired, a captured mesh token changes, or the session leaves/drains.
An unrelated edit does not invalidate the job.

The worker checks before/between regional reads, between complete generated
source columns, before lighting, between target meshes and before returning a
complete payload. A source cache entry is inserted only after a full pristine
column has been built. A stopped batch returns **no** partial mesh/chunk payload.
A completed light solve or individual chunk mesh is disposable worker scratch.
No unsafe Rust implements cancellation.

Every normal outcome, including a stop or regional read error, returns the owned
cache for reuse. Main-thread completion still joins the finished thread and
validates all targets before publishing. A late stop never authorizes installation;
a job already completed before the stop is counted as completed-but-discarded.
The token is monotonic, so leaving and returning during one job cannot revive it.
No thread is detached. Drop requests a stop and joins; the ordinary save-and-drain
barrier retains the world and SaveStore lock until save and terrain cleanup finish.
Cancellation does not cancel the final save. An OS read or a running light solve
is not interrupted internally; shutdown still waits for that bounded stage under
ordinary conditions. No arbitrary timeout drops an owning worker.

Failures keep a one-second per-column backoff and visible F5 retry. Panic and
thread-start failures now also back off. A panic/start failure may drop reusable
pristine cache, never authoritative edits. Invalid regional data cannot become
procedural fallback. Returned overlays only fill absent regional entries;
newer resident/dirty maps remain authoritative. Save/journal budgets are unchanged.

## Main thread, cache and diagnostics

All Bevy entity/asset changes remain in the main Update, after validating the
whole output unit. Loaded chunks become collision-ready before CPU mesh
publication in the same Update. GPU upload and display/presentation are not timed.
Water-only work still reuses applied light and opaque/cutout handles. Empty and
evicted surfaces remove their entities/assets. Compact Air/Stone/WaterSource
storage is unchanged. Source cache retention remains the desired square plus its
one-column halo, at most81 full-height columns at radius3; no history cache grows
with exploration. Cache hits mean a pristine source lookup, excluding a resident
source-column reuse. Hits/misses are cumulative per session/cache owner, not an
inferred overall generation hit rate.

Job creation reuses one target set for previous-light selection and restricts
forced-geometry copies to that job's targets. Source snapshots still copy their
bounded full-height authoritative halo; broad snapshots and large light solves
remain latency costs. The worker cache is reclaimed before another worker starts.

Constant-space summaries record counts, totals and maxima for preparation,
regional-input time, total worker wall, computation, first queue wait,
request-to-ready, request-to-CPU-publication, dirty-to-CPU-publication, installation
and eviction. First queue wait stops **before** preparation at the first dispatch;
it does not include worker processing/retry time. Request latency includes retries.
Dirty summaries use the oldest invalidation in each published component. The
benchmark additionally records per-target dirty events with identical probes in
both versions. Regional-input time includes cache checks; detailed disk-read calls
are separately measured by optional probes.

F3 adds two retained lines: pending load/dirty targets and current job/age, then
installed/stale/stopped/failed counts. Job age includes preparation/thread handoff
and worker time. Old worker totals exclude stopped jobs; the new execution total
includes every returned outcome. Idle logs expose the new summaries and cache
counts. There are no production event vectors, allocation meters or CPU-clock FFI.

## Matched measurement procedure

```sh
# After all ordinary checks/builds have stopped, on an otherwise idle host:
python3 tools/benchmark_streaming.py --output work/slice32-forward --repeats 3 --frames 120
# Both executables already exist; reuse exactly those source-verified binaries:
python3 tools/benchmark_streaming.py --output work/slice32-forward --reuse --reverse --repeats 3 --frames 120
```

Copy the forward output directory before the reverse run so neither report is
lost. The script exports exact sources, injects identical probes in disposable
copies and builds both binaries before measuring. Core is explicitly cleaned and
recompiled for each client; runtime core/client fingerprints guard shared-target
artifact reuse. It keeps raw event samples, final authority/resource signatures,
P50/P95/P99, counts, totals, maxima and machine/method metadata. Percentiles use
`floor((N-1)*p/100)`. No observations are trimmed.

Thirteen terrain scenarios at each radius cover stationary loading,4.317-block/s
walking,5.612 sprinting, normalized diagonal movement, rapid reversals, repeated
teleports, returns after eviction, forest, coast, cavern, concurrent repeated edits,
water at a chunk edge and radius changes. Positions/velocities follow fixed240Hz
deadlines, with actual sleep only until the next benchmark frame. This is a
scripted headless streaming workload, not a physics traversal or rendered FPS test.
Cold preparation and full convergence are included; both builds must finish with
identical resident blocks and mesh payloads. A fourteenth scenario separately
measures twelve A/B session switches during requested terrain work at each radius,
verifying exact menu asset/entity reclamation. Deterministic barrier tests supply
interleavings independent of benchmark host timing.

The optional global allocator measures peak **additional requested process heap
within each worker window**. It excludes allocator metadata, stacks, prior caches
and GPU/driver allocations, and may include concurrent main-thread allocations.
RSS/HWM are process-wide; HWM carries earlier workloads. Neither is mislabeled as
isolated worker RSS. Logical resident/cache/mesh payloads are reported separately.
Worker CPU uses Linux thread CPU time; wall includes scheduling. CPU publication
includes asset replacement and queued entity commands, not deferred command flush,
GPU upload or first displayed pixels. Whole ECS-update wall captures more main
work but remains a headless measurement. Utilization can be calculated from summed
worker wall / full observation duration; it is not a CPU occupancy percentage.

## Results and validation

Measured tables and exact validation results are added after the final runs.
Native Void/Radeon, Wayland and Windows GUI results require owner acceptance.

## Void Linux glibc / Fish native comparison

Owner hardware: i3-13100 (4 physical/8 logical CPUs), RX9060 XT16GB,32GB DDR5,
1920×1080/240Hz. Use the same driver/backend/compositor and release profile.
The old approximately240FPS observation predates current generation and is not
a baseline. Worker CPU savings do not imply an FPS improvement.

```fish
git fetch origin
set bench_root (mktemp -d /tmp/rustcraft-slice32.XXXXXX)
git worktree add --detach "$bench_root/slice31" 1b7e405
git worktree add --detach "$bench_root/slice32" origin/feat/slice-32-chunk-streaming-v2
for version in slice31 slice32
    cargo build --release --locked --manifest-path "$bench_root/$version/Cargo.toml" -p rustcraft
end
mkdir -p "$bench_root/template" "$bench_root/run31" "$bench_root/run32" "$bench_root/logs"
# Create ONE disposable named world, seed42/gen9; set matching graphics/settings,
# F5, wait Saved, return to title and close normally before copying.
env RUSTCRAFT_DATA_DIR="$bench_root/template" "$bench_root/slice31/target/release/rustcraft"
cp -a "$bench_root/template/." "$bench_root/run31/"
cp -a "$bench_root/template/." "$bench_root/run32/"
# Run serially. Choose the identical named world in each catalog.
env RUSTCRAFT_DATA_DIR="$bench_root/run31" RUSTCRAFT_START_CHUNK="-28,1" RUSTCRAFT_LOAD_RADIUS=2 \
    /usr/bin/time -v -o "$bench_root/logs/31-time.txt" "$bench_root/slice31/target/release/rustcraft" > "$bench_root/logs/31.txt" 2>&1
env RUSTCRAFT_DATA_DIR="$bench_root/run32" RUSTCRAFT_START_CHUNK="-28,1" RUSTCRAFT_LOAD_RADIUS=2 \
    /usr/bin/time -v -o "$bench_root/logs/32-time.txt" "$bench_root/slice32/target/release/rustcraft" > "$bench_root/logs/32.txt" 2>&1
```

GNU time is optional (Void `time` package); use equivalent process CPU/RSS tooling
if unavailable. For frames, use an installed MangoHud/driver capture tool, save
its raw frame logs and compute the same percentiles in both runs. Record seed,
start/camera, route, radius1/2/3,1080p, monitor mode, VSync, driver, backend,
compositor and capture settings. Match run duration; repeat at least three times
in A/B and B/A order. Restore fresh complete copies from the closed template
between repetitions so edits, inventory, journals and cache warmness match.

For **per-request native traces**, build the optional source-instrumented copies
on the native host, after the ordinary comparison. This adds no game dependency:

```fish
python3 tools/benchmark_streaming.py --native --output work/slice32-native
# Use separate complete copies of the template for these two instrumented runs.
env RUSTCRAFT_DATA_DIR="$bench_root/run31" RUSTCRAFT_LOAD_RADIUS=2 RUSTCRAFT_STREAM_TRACE=1 \
    work/slice32-native/slice31-native > "$bench_root/logs/31-trace.txt" 2>&1
env RUSTCRAFT_DATA_DIR="$bench_root/run32" RUSTCRAFT_LOAD_RADIUS=2 RUSTCRAFT_STREAM_TRACE=1 \
    work/slice32-native/slice32-native > "$bench_root/logs/32-trace.txt" 2>&1
```

Trace lines are `S32_TRACE,metric,value`; times are microseconds. The executable
asserts its core fingerprint at startup. Request/dirty CPU-publication, first queue
wait, worker CPU/wall, preparation, regional reads and main installation can be
compared separately. Trace/allocator overhead affects both instrumented builds;
use the ordinary builds for the primary FPS comparison. Never interpret these
CPU events as GPU timestamps.

Manual acceptance:

- Walk/sprint diagonally and reverse across signed chunk/region edges; inspect
  near terrain readiness and F3 queue/job counts at radii1–3. Return to visited
  terrain and repeatedly enter/leave dense forests, coastlines and caves.
- Push/fall/swim toward pending columns and move vertically near their boundary.
  Unknown terrain remains closed; known ready sky has no invisible wall.
- Mine/build across edges while neighboring loads run. Repeatedly change a roof,
  torch and vegetation support; check coherent light/AO/opaque/cutout seams.
  Collect/place water with buckets at an edge; verify old opaque handles survive
  water-only changes and no phantom faces or duplicate items appear.
- Shrink/grow radius during loading; teleport using a closed-world restart/start
  override for distant coordinates. No old surface may reappear outside residency.
- F5/autosave/close/reopen after Air/water edits and cross-region changes; compare
  inventory and world identity. Fault/recovery checks use disposable copies only.
  A corrupt/unavailable region must show retry failure, never fresh terrain.
- Save to title during generation/remeshing, A→B→A repeatedly, then close/reopen.
  Check independent edits and stable entity/mesh counts, prompt ordinary drain and
  no old-world installation. Repeat X11/Wayland input/capture/window checks and
  Windows10/11 GUI acceptance separately from CI.

## Remaining bottlenecks and next slices

Slice33 should profile large light batches, snapshot copies and CPU mesh asset
replacement/command flush alongside actual native frame/GPU times. Then measure
opaque/water/cutout surface counts, draw submissions, visibility culling,
transparency/overdraw and safe batching. Keep seam/AO exactness as an oracle.
Slice34 can evaluate separate simulation/render budgets, larger bounded residency,
cache/light memory and distant representations/LOD. The current radius3 ceiling
remains enforced until that work is measured and reviewed.
