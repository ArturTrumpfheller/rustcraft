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
Dirty summaries start when Update ingests invalidations and use the oldest
ingested time in each published component; pre-Update edit time is not captured. The
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

Thirteen ordinary terrain scenarios at each radius cover stationary loading,4.317-block/s
walking,5.612 sprinting, normalized diagonal movement, rapid reversals, repeated
teleports, returns after eviction, forest, coast, cavern, concurrent repeated edits,
water at a chunk edge and radius changes. Positions/velocities follow fixed240Hz
deadlines, with actual sleep only until the next benchmark frame. This is a
scripted headless streaming workload, not a physics traversal or rendered FPS test.
Cold preparation and full convergence are included; both builds must finish with
identical resident blocks and mesh payloads. Two extra controlled scenarios hold a job before regional reads while moving
away, and keep dirty invalidations eligible across twelve completed service slots.
They isolate early-abandonment CPU and actual class-service behavior rather than
claiming those interleavings are typical native workloads. A separate scenario
measures twelve A/B session switches during requested terrain work at each radius,
verifying exact menu asset/entity reclamation. Deterministic barrier tests supply
interleavings independent of benchmark host timing.

The optional global allocator measures peak **additional requested process heap
within each worker window**. It excludes allocator metadata, stacks, prior caches
and GPU/driver allocations, and may include concurrent main-thread allocations.
RSS/HWM are process-wide; HWM carries earlier workloads. Neither is mislabeled as
isolated worker RSS. Logical resident/cache/mesh payloads are reported separately.
Worker CPU uses Linux thread CPU time over the worker body; final captured-input
destruction/thread teardown is outside that probe window. Request/session timings
include the completed handoff/drain. Worker wall includes scheduling. CPU publication
includes asset replacement and queued entity commands, not deferred command flush,
GPU upload or first displayed pixels. Whole ECS-update wall captures more main
work but remains a headless measurement. Utilization can be calculated from summed
worker wall / full observation duration; it is not a CPU occupancy percentage.

## Measured results

The reproducible gains are finite load service under dirty pressure, safe early abandonment and shorter active session switches. **No general exploration-latency or native FPS improvement is established.** Ordinary cases include slower observations; all samples, maxima and source identities are retained.

Final runs use the same immutable source-verified executables, three repetitions per scene/radius/build, **reverse order first, then forward**, with no concurrent local build/test/renderer. There are 15 terrain scenarios ×3 radii ×3 repetitions and 9 session groups per build/order: **144 final authority/resource signatures match exactly** in each order. Each group fully converges; the faster path never omits final desired terrain. Frame-paced movement lasts 0.5 s per repetition and samples individual chunk-boundary crossings, rather than establishing long native exploration endurance. Cold preparation is included in event distributions.

Full tables: [forward](raw/forward/comparison.csv), [reverse](raw/reverse/comparison.csv). Complete events: [forward CSV.gz](raw/forward/samples.csv.gz), [reverse CSV.gz](raw/reverse/samples.csv.gz). Source/binary provenance, final signatures and the earlier unfavorable run are under [raw](raw/). The compressed CSVs contain every event; `gzip -dc` or Python `gzip.open` reads them.

Request-to-CPU-publication **P50 /P95 /P99 milliseconds**, Slice31 →Slice32. These include cold/startup requests; they are not first displayed pixels.

| Scenario / radius | Forward | Reverse |
| --- | --- | --- |
|stationary /1|25.801 / 40.217 / 43.012 → 28.806 / 42.129 / 43.861|26.230 / 37.959 / 41.764 → 26.019 / 41.787 / 47.418|
|stationary /2|41.857 / 93.365 / 98.258 → 53.175 / 104.559 / 110.611|41.998 / 94.872 / 102.036 → 41.867 / 96.255 / 104.608|
|stationary /3|84.975 / 173.251 / 187.053 → 92.054 / 176.787 / 182.632|88.678 / 176.921 / 188.494 → 88.760 / 184.951 / 199.609|
|walking /3|70.632 / 177.844 / 204.162 → 78.648 / 187.094 / 206.445|79.370 / 189.622 / 200.731 → 71.275 / 169.121 / 182.408|
|sprinting /3|74.521 / 175.966 / 187.949 → 81.980 / 189.728 / 211.531|73.103 / 171.951 / 190.245 → 77.900 / 173.588 / 192.195|
|diagonal /3|77.655 / 264.726 / 286.332 → 65.285 / 176.627 / 186.293|70.095 / 217.743 / 249.303 → 69.424 / 178.470 / 191.815|
|reversal /1|15.866 / 41.593 / 49.914 → 16.991 / 172.587 / 190.160|15.337 / 40.030 / 45.090 → 15.183 / 36.748 / 39.665|
|reversal /3|33.351 / 165.798 / 179.001 → 38.473 / 192.090 / 203.712|37.479 / 197.305 / 239.630 → 33.126 / 170.512 / 186.075|
|teleport /3|33.069 / 168.349 / 187.445 → 34.997 / 177.187 / 200.276|32.717 / 159.108 / 179.050 → 31.205 / 156.785 / 176.900|
|forest /3|84.400 / 182.583 / 203.845 → 73.680 / 159.065 / 174.589|78.888 / 172.024 / 184.947 → 74.177 / 166.386 / 179.132|
|coast /3|77.702 / 202.959 / 213.428 → 88.650 / 212.547 / 233.084|95.250 / 220.862 / 234.480 → 83.860 / 208.605 / 225.889|
|cavern /3|79.535 / 203.581 / 232.808 → 84.150 / 218.238 / 265.406|95.305 / 217.323 / 246.650 → 86.352 / 207.268 / 224.136|
|editing /2|35.350 / 87.557 / 91.326 → 38.463 / 104.215 / 124.687|37.225 / 92.051 / 95.757 → 46.035 / 124.706 / 136.956|
|editing /3|76.718 / 174.108 / 186.292 → 76.728 / 180.702 / 205.843|75.074 / 175.624 / 197.175 → 88.554 / 254.910 / 275.086|
|water /3|69.407 / 162.722 / 174.394 → 82.495 / 216.906 / 259.520|72.636 / 170.732 / 184.241 → 76.952 / 170.097 / 182.225|
|radius /3|71.523 / 196.599 / 234.386 → 69.553 / 185.830 / 271.092|71.290 / 170.360 / 181.988 → 75.091 / 173.834 / 186.817|

Ordinary results do not support a uniform speedup. For example, editing/radius2 load P50 worsens in **both** orders; radius3 editing and water tails also regress in individual orders. Walking/radius3 changes sign across order. The forward reversal/radius1 P95 spike is retained. Class fairness can defer one class while serving the other; these observations do not isolate that tradeoff from cloud variation. Native repeated profiling remains necessary; unfavorable cases are not dismissed as noise.

Stationary/radius3 separates the latency components, **P50 /P95 /P99 ms**. Counts are baseline/current observations; queue wait includes first load and dirty-target dispatches, whereas ready/publication include successful loads only.

| Metric | Forward count | Forward31 →32 | Reverse count | Reverse31 →32 |
| --- | --- | --- | --- | --- |
|queue_wait|219/219|52.462 / 166.874 / 182.822 → 58.793 / 172.894 / 179.315|219/219|56.905 / 170.235 / 182.830 → 57.688 / 179.566 / 191.440|
|request_ready|171/171|84.960 / 173.230 / 187.037 → 92.042 / 176.774 / 182.610|171/171|88.661 / 176.899 / 188.473 → 88.747 / 184.938 / 199.591|
|request_cpu_visible|171/171|84.975 / 173.251 / 187.053 → 92.054 / 176.787 / 182.632|171/171|88.678 / 176.921 / 188.494 → 88.760 / 184.951 / 199.609|
|worker_cpu|174/174|2.297 / 5.479 / 6.884 → 2.334 / 5.219 / 6.497|174/174|2.223 / 5.596 / 6.988 → 2.269 / 5.246 / 6.963|
|worker_wall|174/174|2.310 / 5.525 / 6.884 → 2.349 / 5.346 / 6.503|174/174|2.222 / 5.900 / 7.470 → 2.268 / 5.411 / 6.971|
|prepare|174/174|0.084 / 0.244 / 0.705 → 0.074 / 0.156 / 0.251|174/174|0.068 / 0.339 / 0.613 → 0.068 / 0.126 / 0.259|
|region_read|36/36|0.018 / 0.039 / 0.040 → 0.017 / 0.032 / 0.041|36/36|0.019 / 0.042 / 0.048 → 0.018 / 0.024 / 0.042|
|main_install|174/174|0.103 / 0.548 / 0.852 → 0.125 / 0.621 / 0.785|174/174|0.114 / 0.616 / 0.781 → 0.121 / 0.540 / 0.812|
|ecs_update|360/360|0.081 / 0.197 / 0.606 → 0.084 / 0.281 / 0.453|360/360|0.246 / 0.909 / 1.580 → 0.125 / 0.582 / 1.088|

Worker execution is much shorter than cold queue/request tails here. Completion polling, per-job handoff, snapshot preparation and whole-batch installation remain. No persistent worker-thread/handoff redesign or GPU installation budget was added.

Dirty-to-CPU-publication **P50 /P95 /P99 ms**, per target from first ingested invalidation, including coalescing/retries and initial mesh requests in each workload:

| Scenario / radius | Forward31 →32 | Reverse31 →32 |
| --- | --- | --- |
|editing /1|4.287 / 8.439 / 8.765 → 4.243 / 8.574 / 13.827|4.237 / 12.605 / 58.361 → 4.305 / 8.625 / 41.742|
|editing /2|4.220 / 8.385 / 8.644 → 4.238 / 8.510 / 16.789|4.233 / 8.445 / 9.445 → 4.256 / 8.545 / 11.629|
|editing /3|4.276 / 8.567 / 21.537 → 4.282 / 8.509 / 12.684|4.282 / 8.439 / 8.837 → 4.242 / 8.362 / 8.702|
|water /1|4.187 / 7.966 / 15.781 → 4.179 / 12.321 / 12.614|4.175 / 6.384 / 6.501 → 4.204 / 15.052 / 18.744|
|water /2|4.196 / 9.075 / 9.426 → 4.198 / 10.928 / 11.785|4.189 / 7.713 / 7.987 → 4.190 / 10.095 / 13.020|
|water /3|4.195 / 7.305 / 7.829 → 4.192 / 12.165 / 13.216|4.211 / 8.779 / 18.208 → 4.190 / 11.768 / 12.871|
|dirty_pressure /3|57.740 / 80.093 / 80.096 → 47.272 / 60.517 / 60.520|52.411 / 58.304 / 58.307 → 43.169 / 54.422 / 54.425|

Controlled dirty pressure keeps edits eligible across twelve worker-completion/dispatch slots. Slice31 dispatches **0 loads** in all nine groups per order; Slice32 dispatches **3 /4 /4 loads at radii1 /2 /3**, in every repetition and both orders. Both later drain to the identical desired world. This demonstrates the starvation fix, not an FPS gain or an assertion that every ordinary edit burst behaves this way.

The gated teleport moves away while a worker is held before regional reads. CPU in that one obsolete worker, **P50 µs**,3 samples per cell:

| Radius | Forward31 →32 | Reverse31 →32 |
| --- | --- | --- |
|1|10524.711 → 25.878|9367.500 → 49.483|
|2|7849.968 → 15.152|7724.866 → 13.111|
|3|9050.261 → 15.523|8772.539 → 15.185|

This is roughly 99.5–99.8% less CPU for an explicitly obsolete-before-start job, including its controlled gate overhead. It is **not** a whole-workload saving. Only 3 gated samples per radius/order mean P95/P99 under the floor estimator coincide with the median; the full raw samples/maxima are the evidence, not reliable native tail estimates. Cancellation during an already-running light solve still waits for the next safe boundary.

Active A/B session switch **P50 /P95 /P99 ms**,36 switches per radius/build/order:

| Radius | Forward31 →32 | Reverse31 →32 |
| --- | --- | --- |
|1|6.997 / 10.990 / 11.116 → 2.394 / 6.644 / 6.740|5.580 / 7.556 / 8.108 → 2.549 / 6.446 / 6.819|
|2|5.496 / 10.925 / 15.053 → 2.690 / 3.885 / 4.637|5.674 / 8.241 / 9.459 → 1.970 / 3.456 / 6.000|
|3|6.483 / 14.592 / 21.247 → 2.700 / 4.237 / 4.682|5.690 / 7.666 / 7.929 → 2.770 / 5.171 / 6.220|

Session P50 improves in both orders at all radii, from 5.5–7.0 ms to 2.0–2.8 ms in these samples. Every switch restores the exact tracked menu asset baseline and removes all session entities. This includes load/save/drain/cleanup, not a guarantee about a native GUI transition.

Representative job outcomes over all repeats of each terrain scene, **installed / completed-discarded / cooperatively-stopped**, Slice31 →Slice32:

| Order / scene / radius | Slice31 | Slice32 |
| --- | --- | --- |
|forward /teleport /3|533 / 57 / 0|517 / 50 / 7|
|forward /reversal /3|279 / 0 / 0|273 / 0 / 1|
|forward /editing /3|285 / 0 / 0|285 / 0 / 0|
|forward /gated_teleport /3|468 / 3 / 0|468 / 0 / 3|
|forward /dirty_pressure /3|219 / 33 / 0|219 / 24 / 0|
|reverse /teleport /3|530 / 57 / 0|528 / 53 / 4|
|reverse /reversal /3|279 / 0 / 0|279 / 0 / 0|
|reverse /editing /3|285 / 0 / 0|285 / 0 / 0|
|reverse /gated_teleport /3|468 / 3 / 0|468 / 0 / 3|
|reverse /dirty_pressure /3|219 / 33 / 0|219 / 24 / 0|

All terrain groups report 0 failed jobs. Successful request percentiles exclude requests evicted/cancelled without installation; those are counted separately, never assigned zero latency. Rapid-movement sample populations differ, so a small conditional percentile difference does not prove a request-level speedup. Summed worker-body CPU, worker wall, first queue waits, ready/publication, dirty latency, cache hits/misses, peaks, exploration/observation durations and every unfavorable tail remain in the CSVs.

Summed worker-body CPU and utilization, across three repetitions at radius3. Busy wall is summed worker-body wall / full observation wall, **not CPU occupancy**; cache hit rate counts only pristine lookups (resident source reuse bypasses it).

| Order / build / scenario | Worker CPU total ms | Worker wall / observation ms | Busy wall % | Pristine hit rate % |
| --- | --- | --- | --- | --- |
|forward /slice31 /stationary|467.137|468.648 /2178.217|21.5|69.6|
|forward /slice32 /stationary|483.000|487.612 /2196.367|22.2|69.5|
|forward /slice31 /teleport|1582.877|1586.484 /2565.549|61.8|56.2|
|forward /slice32 /teleport|1615.578|1621.273 /2607.823|62.2|58.7|
|forward /slice31 /dirty_pressure|773.623|776.294 /2517.872|30.8|70.8|
|forward /slice32 /dirty_pressure|644.810|645.578 /2362.283|27.3|70.7|
|forward /slice31 /session|248.101|249.664 /345.260|72.3|0.0|
|forward /slice32 /session|22.957|23.451 /103.993|22.6|0.0|
|reverse /slice31 /stationary|502.016|503.938 /2223.676|22.7|69.6|
|reverse /slice32 /stationary|481.775|485.325 /2217.598|21.9|69.5|
|reverse /slice31 /teleport|1626.476|1630.944 /2576.693|63.3|56.2|
|reverse /slice32 /teleport|1581.391|1585.359 /2533.931|62.6|58.8|
|reverse /slice31 /dirty_pressure|722.050|724.144 /2466.471|29.4|70.8|
|reverse /slice32 /dirty_pressure|609.014|611.184 /2311.204|26.4|70.7|
|reverse /slice31 /session|140.598|142.342 /215.398|66.1|0.0|
|reverse /slice32 /session|22.963|23.016 /111.306|20.7|0.0|

Cancellation changes the amount of obsolete work/lookup activity, so the hit-rate denominator and completed-request populations differ. These sums include retained outliers; they do not estimate total process CPU or rendered-frame cost.

Stationary final resident resources, identical between builds and orders (repeat0):

| Radius | Ready descriptors | Voxel bytes | Bevy Mesh assets | Mesh payload bytes | Pristine cache columns /bytes |
| --- | --- | --- | --- | --- | --- |
|1|144|98304|41|1130936|25 /278528|
|2|400|278528|119|3622208|49 /524288|
|3|784|524288|231|6737848|81 /847872|

Process-wide memory and requested-heap peaks across the full terrain matrix; these are **not** GPU memory or isolated worker RSS:

| Order / build | Max observed resident RSS KiB | Process HWM KiB | Largest additional requested heap in a worker window bytes |
| --- | --- | --- | --- |
|forward /slice31|51380|51380|2903856|
|forward /slice32|57592|57596|2901232|
|reverse /slice31|50512|52764|2880968|
|reverse /slice32|55336|55336|2897624|

Snapshots/captured inputs allocated before worker entry are outside that incremental heap window; final RSS/HWM are process-wide and allocator retention persists across cases. Every final signature matches, including mesh assets/payload, and tests enforce cache/resident bounds and complete three-surface eviction. The added constant-space summaries, two bounded first-dispatch sets and a per-job token are real metadata overhead; there is no claim that total memory is byte-identical.

Max observed process RSS is 4.7–6.1 MiB higher for Slice32 in these runs; process
HWM rises 2.5–6.1 MiB, while the largest incremental worker heap window is similar.
Allocator/layout effects and the exact cause were not isolated. This is an
unfavorable memory observation, not evidence of byte-neutral metadata overhead
or a demonstrated unbounded leak. Native repeated memory profiling is still needed.

The preliminary 13-scene [initial run](raw/initial/comparison.csv) showed broad slower current-client values (for example stationary/radius3 P50/P95 77.086/173.210→95.780/295.693ms). It is retained with every event and its earlier source/probe identities. It is not pooled with the final suite, which adds the two controlled scenarios and uses the final test sources. The final reverse stationary/radius3 P50 is nearly unchanged, while forward is slower; no general load-latency win is inferred.

An additional unchanged-scene core check uses 8 warmups +64 samples per build for all 13 Slice31 scenes, with identical stage probes and separate targets: [forward](raw/core-forward/comparison.csv), [same-binary reverse](raw/core-reverse/comparison.csv). All block/light/geometry/cache invariants pass. The broad client CPU regression does not reproduce uniformly in core: reverse P50 varies from about−18% to+5% depending on scene. The first three forward baseline scenes are much slower than their reverse repeats despite unchanged code. No compute speedup is attributed to scheduling or to unchanged lighting/meshing from that variation.

## Validation completed

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace`:207 client +319 core +3 integration/example tests passed;6 intentional benchmark tests ignored.
- `cargo test --release --locked -p voxel_core --lib`:319 passed, including optimized AO, frozen generators and cancellation/cache oracles.
- `cargo test --release --locked -p rustcraft --bin rustcraft streaming:: -- --test-threads=1`:40 passed,2 intentional benchmarks ignored.
- `cargo test --release --locked -p rustcraft --bin rustcraft session:: -- --test-threads=1`:11 passed,1 intentional benchmark ignored.
- `cargo test --release --locked -p voxel_core --test worldgen_integration`:2 passed.
- `cargo build --release --locked -p rustcraft` and `python3 tools/generate_textures.py --check`: passed.
- Matched final client runs, both core orders, all source/final-state assertions and both optional native trace builds: passed. Trace summarization was checked against actual recorded worker samples.
- [Ubuntu and Windows CI on the final production/test commit](https://github.com/ArturTrumpfheller/rustcraft/actions/runs/41315352820): passed, including the unchanged optimized AO check plus complete core, streaming and session release checks. The final documentation/tooling commit has its own PR checks.

The first normal local debug link exhausted the 8-GiB scratch disk. Only rebuildable outputs were removed. Final local workspace tests disable incremental compilation and use a temporary rustc wrapper to strip executable DWARF at link time; test code, assertions and optimization are unchanged. CI uses ordinary repository settings. The optimized core run uses an isolated target directory so benchmark source cleaning cannot remove its executable.

A preexisting dropped-item lifecycle fixture compared all mesh counts during active terrain and fluid updates. It now pauses simulation and waits for actual queue convergence before testing exact asset reuse, retaining exact saved-drop counts and complete teardown assertions. New stop tests use channels/controlled checkpoints, not timing sleeps to establish their interleavings. No Horizon source/dependency was needed or adapted. Native Void X11/Wayland, Radeon FPS/frame/GPU and Windows GUI acceptance remain unmeasured owner work.

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
    env CARGO_TARGET_DIR="$bench_root/$version/target" CARGO_BUILD_JOBS=4 \
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
python3 tools/benchmark_streaming.py --output "$bench_root/trace-summary" \
    --trace "$bench_root/logs/31-trace.txt" "$bench_root/logs/32-trace.txt"
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
