# Slice22 completion measurements

The comparison baseline is PR39 head `a976d37b9bc949ba3e283df39e1236812ab74c69`,
which already has the complete original water simulation. The historical
[Slice21 comparison](../README.md) remains separate.

These finite resident CPU fixtures exclude regional overlay writes, save I/O and
light/mesh work; the software active fixture includes the mesh/light worker.
The CPU harness runs five trials each for dry, stationary pool, falling water,
twelve torches in the flow path, and chunk-boundary flow. It records active tick
latencies, total updates, washed torches and10,000 idle ticks. New-only metrics
isolate1,000 item creation/pickup cycles and10,000 full-inventory pickup attempts.
The baseline has no dropped-item API, so a pickup speedup is not claimed.

Renderer comparisons use sequential matching960×640, radius2, fixed-time6000
fixtures with the production shaders. No builds/tests may run during final
measurements. Software Vulkan/lavapipe results are **not native GPU performance**;
no inference about the owner's ~240-FPS RX9060 XT baseline is valid.

Reproduction (source `/workspace/setup/env.sh` in the prepared cloud):

```sh
python tools/prepare_water_completion_baseline.py work/water-completion-baseline
CARGO_TARGET_DIR="$PWD/target" cargo build --release --locked --manifest-path work/water-completion-baseline/Cargo.toml -p voxel_core --example water_completion_bench
cp target/release/examples/water_completion_bench work/water-completion-before
cargo build --release --locked -p voxel_core --example water_completion_bench
cp target/release/examples/water_completion_bench work/water-completion-after
python tools/measure_water.py --child work/water-completion-before > work/before.csv 2> work/before-resource.txt
python tools/measure_water.py --child work/water-completion-after > work/after.csv 2> work/after-resource.txt
# Build/copy water_gallery for each tree similarly, using debug binaries for both.
python tools/measure_water.py work/water-gallery-before work/water-gallery-after work/frames --frames 60 --before-label before --after-label after
python tools/measure_water.py work/water-gallery-before work/water-gallery-after work/active --frames 60 --before-label before --after-label after --active --scenes waterfall several boundary
cargo test --locked -p rustcraft item_presentation_cpu_measurements -- --ignored --nocapture
```

The minimal ECS item-presentation probe reports CPU update time and logical
retained meshes/vertices/entities, without a renderer or GPU. It does not measure
frame time, VRAM, driver allocations or terrain worker latency. Runtime item
creation shares one lazy mesh/material; world items are capped at4096 stacks and
save data at61,636 bytes. Session-switch tests check complete asset reclamation.

Limits: real Void X11/Wayland and Windows GUI/GPU acceptance remains manual.
Mesh-level translucent sorting can blend intersecting waterfalls incorrectly.

## Release CPU results (2026-10-26)

Rust1.99.1, Bevy0.20.0, Linux cloud, two CPU cores available. Each table value
is the median of five independent fixture fills, default256-cell tick budget.
`waterfall` washes two previously blocking torches; the torch fixture washes the
12 added floor torches plus one pre-existing fixture torch. The old simulation
cannot flow past the four torches immediately surrounding that source, so its
very low torch-fixture time represents stopped flow, not equivalent gameplay.

| Scene | Total active µs before → after | Tick P50 µs before → after | Tick P95 µs before → after | Washed torches before → after |
| --- | --- | --- | --- | --- |
| dry | 0.109 → 0.119 | 0.109 → 0.119 | 0.109 → 0.119 | 0 → 0 |
| pool | 13.158 → 13.089 | 4.177 → 4.118 | 5.067 → 5.152 | 0 → 0 |
| waterfall | 196.572 → 214.726 | 4.995 → 4.823 | 30.723 → 33.896 | 0 → 2 |
| torches | 0.827 → 147.184 | 0.827 → 20.557 | 0.827 → 29.159 | 0 → 13 |
| boundary | 124.285 → 134.215 | 17.578 → 18.822 | 25.932 → 27.602 | 0 → 0 |

Every fixture settles with pending=0. Stable calls cost a median4ns before and
after and process zero cells. The source/queue algorithm remains bounded. The
seam fixture adds about1.24µs to the median active tick in this small test;
the waterfall total rises18.15µs while now also removing two torches. These are
microbenchmarks, not a complete frame/stream/save latency estimate.

New-only1,000-cycle medians: torch→water plus item creation **0.299µs** (P95
0.318µs); indexed pickup **0.520µs** (P950.546µs).10,000 full-inventory attempts
average591ns and leave the item intact. Core process maximum RSS is9.5MiB in
both phases. `World`'s fixed layout grows264→296 bytes (+32B); no dense block
storage changes. The RSS equality does not prove zero allocator overhead for
individual item records or dense4096-item worlds. CSVs and resource observations
are committed alongside this report.

## Item presentation CPU/resources

The isolated debug ECS probe has no GPU/render app:0→12 visible items changes
median update276.08→327.94µs (+51.86µs), P95403.22→463.41µs. It retains1→3 meshes
(the additional water mesh and one shared item mesh),24→224 unique asset vertices,
and1→14 mesh entities. It allocates no per-item mesh/material or texture. This
measures preparation/system overhead only; it cannot establish native draw cost,
VRAM or frame rate. Session lifecycle tests reclaim these resources completely.

## Software renderer results

All runs use the original production gallery at960×640, radius2, VSync off,
fixed daylight6000, Xvfb and Mesa lavapipe. Both versions keep the day/night
atmosphere. Each process warms for at least120 frames and5 seconds; steady runs
then sample30 intervals, active/repeated-dry runs60. Frame percentiles describe
one short run per phase, not statistical confidence intervals. Process RSS is
maximum resident CPU memory, not GPU VRAM. No builds/tests/gameplay window ran
concurrently. The gallery isolates terrain/water and omits gameplay UI/item draws;
item preparation is measured separately above.

| Scene | Before P50 / P95 / P99 ms | After P50 / P95 / P99 ms | RSS MiB before → after |
| --- | --- | --- | --- |
| Steady dry | 75.57 / 229.48 / 242.56 | 63.49 / 76.40 / 78.42 | 346.7 → 347.5 |
| Steady pool | 66.61 / 83.52 / 83.73 | 70.38 / 82.35 / 83.99 | 350.0 → 350.9 |
| Steady waterfall | 80.09 / 92.79 / 92.97 | 74.91 / 86.08 / 90.04 | 338.9 → 352.2 |
| Steady several | 86.24 / 91.51 / 93.26 | 75.02 / 85.17 / 100.08 | 349.7 → 349.4 |
| Steady boundary | 75.96 / 102.34 / 124.97 | 75.41 / 86.19 / 87.62 | 350.0 → 352.1 |


The first dry sample showed long tails, so dry was repeated after the other runs
with60 intervals. This unresolved warm-up/host variance is why a dry-world speedup
is not claimed. The repeated dry P50 changes67.37→69.69ms (+3.4%); P95 changes
128.79→127.04ms and P99214.04→144.78ms. These short software observations do not
establish statistical significance or predict native RX9060 XT frame times.

Active fixtures pulse explicit sources once per second at the same5-Hz fluid
cadence. `WATER_LIVE` records actual updates, maximum simulation time, pending
requests and mesh jobs. Emission-changing jobs now recompute light and terrain;
ordinary water jobs reuse the cached light. At most one gallery job is in flight.

| Active scene | Before P50 / P95 / P99 ms | After P50 / P95 / P99 ms | RSS MiB before → after |
| --- | --- | --- | --- |
| waterfall | 79.70 / 90.61 / 100.24 | 103.76 / 140.40 / 160.54 | 351.2 → 346.6 |
| several | 84.75 / 94.22 / 101.33 | 77.01 / 89.23 / 97.90 | 351.8 → 355.1 |
| boundary | 83.28 / 174.92 / 489.42 | 73.04 / 85.51 / 85.78 | 341.4 → 353.8 |

The active waterfall regresses79.70→103.76ms median (+30.2%), P9590.61→140.40ms.
That cost is reported explicitly: the new run also processes more fluid updates
(11,909 vs9,655) and handles washed emitters, while the baseline keeps torches
as barriers. Its maximum debug tick is2.61ms vs2.11ms. The several-source and
boundary medians improve in this sample; the old boundary run has a489ms P99
outlier. Host scheduling, software alpha rasterization and different evolving
water bodies prevent attributing all of these differences to one code path.
Native active-water/torch gameplay profiling is still required. There is no
claim that the owner's240-FPS baseline has been maintained or exceeded.

The complete measurements, including unfavorable tails, are retained in
`steady/`, `active/` and `dry-repeat/`. Production-shader screenshot fixtures
were refreshed after the measurement runs. The pool screenshot uses the optional
`--focus=pool` close-up; measured default cameras are unchanged. Gameplay UI was captured separately;
no image model, proprietary artwork or edited reference screenshot was used.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace`:188 client +224 core tests passed.
- The separate item-presentation probe and17 viewmodel tests passed; four CPU
  probes are excluded by default, and the new item probe was also run explicitly.
- `cargo build --release --locked -p rustcraft`: passed (first optimized Bevy
  build25m47s in this two-core environment, unrelated to game frame time).
- `python tools/generate_textures.py --check`: passed; no dependency/lockfile change.
- Code commit `5c5a414` passed both Ubuntu and Windows in
  [PR workflow138](https://github.com/ArturTrumpfheller/rustcraft/actions/runs/40390083801).
- Native Void X11/Wayland and Windows GUI/GPU acceptance remains pending; see
  [manual checklist](../../../WATER.md#void-linux-manual-acceptance-fish).
