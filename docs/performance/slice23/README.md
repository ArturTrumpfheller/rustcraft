# Slice23 fluid behavior measurements and acceptance

Issue [#40](https://github.com/ArturTrumpfheller/rustcraft/issues/40).
Baseline: merged Slice22 **aed57cb42e4c7f949aa8a4812ab07cbeb89c090b**.
After: this PR's production core and identical `water_behavior_bench` harness.
Measured 2026-10-28, Rust1.99.1 / Bevy0.20.0, unchanged Cargo.lock/dependencies.
Linux cloud host reports AMD EPYC9V74; container quota is **two CPU cores**.
These are neither native Void nor RX9060 XT results.

## Core CPU workload

Release binaries run sequentially, nine independent repetitions per scene and
phase, without concurrent builds/rendering. The unchanged harness is copied to
the baseline checkout; only the production algorithm differs. Both use the same
147-chunk fixture and default budget256. Times are elapsed microseconds around
`tick_fluids` on the CPU, including its batch/overlay work, not GPU timings or
thread CPU counters. Fixture construction and initial source placement are
outside the tick timer. Drain-edit dispatch is measured separately. Cloud
scheduling can affect tails; all raw repetitions remain in [core/](core/).

Each table entry is the median of nine repetition results. P95 is the median
of each repetition's tick P95. Total covers expansion until the queue drains;
drain-edits includes initial fill plus **four open/close cycles**. Waterfall is
a **13-source curtain from Y=25**, ending at605 water cells. Holes has competing
two-/three-step drains; obstacles has a wall/detour; boundary drains across X=16.
The settled case is timed after a pool already converges.

| Scene | Total µs, Slice22 → 23 | Tick P95 µs, 22 → 23 | Processed cells, 22 → 23 | Peak pending, 22 → 23 |
| --- | ---: | ---: | ---: | ---: |
| Dry | 0.09 → 0.11 | 0.09 → 0.11 | 0 → 0 | 0 → 0 |
| Small pool | 11.17 → 20.43 | 3.98 → 8.54 | 52 → 52 | 24 → 24 |
| Several sources | 420.21 → 831.38 | 67.42 → 159.13 | 1997 → 1997 | 392 → 392 |
| Large waterfall | 497.26 → 1111.61 | 44.95 → 127.52 | 3064 → 3064 | 260 → 260 |
| Obstacles | 97.44 → 152.43 | 24.68 → 30.58 | 443 → 405 | 127 → 109 |
| Nearby holes | 120.76 → 7.12 | 27.69 → 3.50 | 560 → 28 | 140 → 7 |
| Repeated drainage edits | 130.05 → 2117.57 | 28.94 → 42.77 | 652 → 6744 | 140 → 251 |
| Chunk boundary | 123.06 → 11.87 | 33.05 → 5.23 | 564 → 35 | 140 → 7 |
| Completely settled | 0.04 → 0.04 | 0.04 → 0.04 | 0 → 0 | 0 → 0 |

**Remaining regressions:** initial several-source work is about2× and waterfall
work2.24× baseline, because the solver now searches for drops. Repeated drainage
costs16.3× overall, chiefly because Slice22 never redistributes the existing
horizontal body: the new behavior processes6744 cells over72 ticks versus652
over20. The new total is2.12ms across that whole workload, with median tick
P95=42.8µs and median maximum=72.7µs. Dispatching its eight terrain edits costs
41.4µs total versus3.0µs, including the new bounded dependency invalidation.
This is real additional work, not a claim of equivalent behavior in the baseline.

**Benefits and limits:** holes process95% fewer cells; boundary routing processes
94% fewer. Every scene finishes with **pending=0**; another10,000 idle ticks
process no cells and average3ns/call in both versions. Across every recorded raw
repetition, the maximum new tick is574.8µs (waterfall); the baseline has a4976.2µs
boundary outlier. These elapsed tails include OS scheduling and are not native
frame-time guarantees. The median maximum new waterfall tick is243.3µs.

The first exploratory per-tick-cache implementation showed excessive repeated
searches. Persisting lazily allocated byte-per-cell route arrays, invalidating
only relevant entries, remembering blocked search cells and returning immediately
for an empty queue reduced that overhead. A source/terrain edit still deliberately
recalculates affected routes. The retained cache has a strict602,112-byte payload
cap at radius3 plus map/allocator overhead, and frees evicted chunks. The process
RSS helper reports12,032KiB for both core executables in this run; its Python/fork
sampling floor cannot resolve such small cache differences. User/system CPU and
RSS for each scene are preserved alongside the raw CSVs.

## Software-rendered frame comparison

The existing unmodified production-shader gallery runs at960×640, radius2,
time6000, Xvfb X11 / Mesa lavapipe Vulkan, `LP_NUM_THREADS=2`. Both phases retain
the same atmosphere and assets. Runs wait at least120 warmup frames and5 seconds,
then collect60 frame intervals. [Raw frames/resources](frames/frames.csv) include
P50/P95/P99 and process CPU/RSS for dry, pool, waterfall, several and boundary.
These short debug/software runs can find large regressions but cannot establish
native FPS, compositor behavior or physical GPU performance. Core timings above
use release builds; do not directly combine their absolute times with these frames.

| Scene / run order | P50 ms, Slice22 → 23 | P95 ms, 22 → 23 | P99 ms, 22 → 23 |
| --- | ---: | ---: | ---: |
| Dry, 22 first | 56.65 → 57.95 | 73.70 → 75.23 | 106.19 → 118.12 |
| Pool, 22 first | 61.61 → 63.69 | 81.77 → 85.17 | 93.53 → 100.80 |
| Waterfall, 22 first | 64.53 → 71.28 | 81.18 → 84.06 | 84.04 → 88.74 |
| Several, 22 first | 67.88 → 72.25 | 86.65 → 90.36 | 90.98 → 114.57 |
| Boundary, 22 first | 62.93 → 70.06 | 81.20 → 85.00 | 82.14 → 90.78 |
| Dry control, 23 first | 59.78 → 59.56 | 75.42 → 74.11 | 82.05 → 142.18 |
| Boundary control, 23 first | 63.36 → 64.40 | 80.49 → 83.13 | 81.90 → 87.16 |
| Active several, 22 first | 72.72 → 77.14 | 95.89 → 97.85 | 104.53 → 110.73 |

The first static sweep is **2.3–11.3% slower at P50** in the new executable.
Because even dry frames differ while doing no fluid work, dry/boundary controls
were repeated in reversed order, not discarded: P50 then changes -0.4%/+1.6%,
but the new dry P99 is worse (142.2ms). All results remain visible in
[reversed controls](frames-reversed/frames.csv). These short cloud samples do
not isolate a renderer regression from scheduling/order variance; native matched
measurements remain required. No rendering code/assets were changed in this PR.

[Active several-source runs](frames-active/frames.csv) pulse sources every second
through the production simulation plus mesh worker, and show6.1% higher P50 in
Slice23. Raw `WATER_LIVE` reports14336→14592 processed cells, peak pending1107
in both,56→57 mesh jobs, and debug maximum fluid elapsed37.84→7.75ms. They cover
warmup plus measurement and real-time pulses, so update totals are not a fixed
equal-tick workload; the release core fixture above supplies that comparison.
All gallery process peaks lie around338–352MiB; per-run CPU/RSS is in the raw
files. This cannot establish the owner's approximately240 native FPS target.

## Reproduction

```sh
# From this PR checkout; work/ is ignored and never contains user worlds.
git worktree add --detach work/slice22 aed57cb42e4c7f949aa8a4812ab07cbeb89c090b
cp crates/voxel_core/examples/water_behavior_bench.rs work/slice22/crates/voxel_core/examples/
CARGO_TARGET_DIR="$PWD/work/target-before" cargo build --release --locked \
  --manifest-path work/slice22/Cargo.toml -p voxel_core --example water_behavior_bench
CARGO_TARGET_DIR="$PWD/work/target-after" cargo build --release --locked \
  -p voxel_core --example water_behavior_bench
python3 tools/benchmark_water_behavior.py \
  work/target-before/release/examples/water_behavior_bench \
  work/target-after/release/examples/water_behavior_bench work/slice23-core --repeats 9
```

For frame comparison, build each checkout's unchanged `water_gallery`, retain
both binaries and run `tools/measure_water.py BEFORE AFTER work/slice23-frames
--before-label slice22 --after-label slice23 --frames 60`. Use the same desktop
and driver for both; the cloud additionally selects its installed lavapipe ICD
explicitly because it has no physical GPU. Use `--active` for expansion/recession
while rendering. No gallery command accesses real saves.

## Tests and native acceptance

The new16 routing tests cover flat spreading, left/right drains, equal and merged
paths, shorter-vs-longer routes, immediate falling, unreachable/out-of-range holes,
wall/corner detours, a narrow channel, changed drainage with no adjacent water,
repeated blocking/opening, source recession, multiple sources, budget independence,
four-step/remaining-strength bounds, chunk/diagonal install/eviction, unloaded
neighbors, partially settled save/reload and zero idle work. Sixteen deterministic
obstacle fixtures also compare different queue budgets and complete recession.

Eight new physics tests cover source/all seven levels/falling water, upward swim,
one-block exit, shallow rendered-surface detection, two-block walls, ceilings,
required Jump/direction/range, one-shot impulse and exact restored gravity,
30/60/144 render-frame scheduling at60 fixed ticks, and unchanged dry jump/sprint.
The existing regional journal test now includes a downhill path across chunk
and region boundaries, three interrupted-save stages, reload, closing the hole
and source removal. Existing bucket, torch/drop/light, meshes, inventory, world
formats/recovery and client tests remain in the full suite.

Local results:

- PASS `cargo fmt --all -- --check`
- PASS `cargo clippy --locked --workspace --all-targets -- -D warnings`
- PASS `cargo test --locked --workspace`:192 client /248 core, four existing
  optional client measurements ignored, no failures
- PASS `cargo build --release --locked -p rustcraft`
- PASS original texture regeneration check and Python measurement-script compile
- PASS identical release benchmark harnesses, all scenes settle and idle at zero

[Void Fish acceptance and matched native performance procedure](ACCEPTANCE.md)
remains manual, including X11/Wayland, physical RX9060 XT and Windows10/11 GUI.
Neither successful CI nor this software renderer establishes that acceptance.
