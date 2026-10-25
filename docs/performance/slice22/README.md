# Slice 22 measurements

Measured on 2026-10-25 in the Codex Linux cloud environment, Intel Xeon Platinum
8573C, **Mesa lavapipe software Vulkan through Xvfb**, 960×640, VSync off,
radius2. Rust1.99.1 / Bevy0.20.0. No native Radeon or Windows GPU was available.
These observations do not predict the owner's RX9060 XT performance.

## Matched rendered steady scenes

The accepted Slice21 baseline is commit `91bce77`. `prepare_water_baseline.py`
exports it and adds the **same original fixture/camera/capture harness**, omitting
only Slice22 water code. Neither phase uses `--baseline` (which would disable the
accepted atmosphere). Debug binaries, same driver/resolution/terrain/camera/time,
sequential runs, 120 warm-up frames and >=5s warm-up, then120 frame intervals.
CPU is total process user+system time including startup/warm-up. RSS is per-child
maximum resident memory (not GPU VRAM). Raw observations: [frames.csv](frames.csv)
and the phase/scene `.txt` files. No Cargo builds or tests ran during these samples.

Slice21 has **no water**: pool/waterfall/multi-source/seam comparisons therefore
show the cost of adding visible water over identical solid fixtures. They do not
pretend Slice21 already simulated fluids. All water is settled here; surface
animation still runs. A separate active fixture is described below.

| Scene | Phase | P50 / P95 / P99 ms | CPU s | Max RSS MiB |
| --- | --- | --- | --- | --- |
| dry | slice21 | 75.98 / 91.02 / 92.41 | 38.24 | 344.3 |
| dry | slice22 | 79.19 / 90.75 / 93.75 | 37.48 | 347.0 |
| pool | slice21 | 83.15 / 99.30 / 117.53 | 43.38 | 345.1 |
| pool | slice22 | 103.43 / 134.84 / 145.92 | 50.48 | 338.5 |
| waterfall | slice21 | 82.99 / 104.25 / 117.53 | 42.23 | 345.5 |
| waterfall | slice22 | 95.30 / 118.28 / 132.78 | 48.68 | 350.8 |
| several | slice21 | 76.93 / 92.64 / 99.41 | 39.67 | 334.1 |
| several | slice22 | 99.21 / 123.88 / 190.18 | 50.99 | 349.7 |
| boundary | slice21 | 80.43 / 101.24 / 113.64 | 41.09 | 345.4 |
| boundary | slice22 | 91.27 / 102.87 / 109.15 | 44.45 | 340.5 |

The dry-scene P50 changed75.98→79.19ms (+4.2%); P95 changed91.02→90.75ms.
One software-rendered run cannot establish a statistically significant dry-world
regression or prove zero overhead. Core block storage/terrain vertex payload are
unchanged; dry simulation processes zero cells and the water shader is not drawn.
Visible alpha-blended water costs software rasterization time, notably the pool
and several-source scenes. Native frame-time acceptance remains required.

## Active fill and steady CPU behavior

Release `water_bench`, default256-cell budget, one deterministic run per fixture.
Each fixture ticks until pending=0; tick percentiles describe **active fill ticks**,
not repeated statistical trials. The stable loop then calls10,000 empty ticks.
No rendering, disk IO or native GPU is included. Mesh ms includes the complete
75-target terrain+water surface build after one bounded light solve; it is not
main-thread fluid time. [cpu.csv](cpu.csv) contains every reported field.

| Scene | Water cells | Fill ticks / processed updates | Tick P50 / P95 / max µs | Stable tick ns | Solid / water meshes | Mesh payload bytes | Full mesh ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dry | 0 | 1 / 0 | 0.396 / 0.396 / 0.396 | 9 | 50 / 0 | 217496 | 3.526 |
| pool | 9 | 3 / 52 | 5.379 / 10.114 / 10.114 | 4 | 50 / 4 | 235312 | 3.346 |
| waterfall | 175 | 17 / 890 | 4.995 / 28.035 / 28.035 | 4 | 50 / 4 | 292608 | 3.830 |
| several | 397 | 10 / 1987 | 50.545 / 66.924 / 66.924 | 4 | 50 / 4 | 340400 | 3.735 |
| boundary | 113 | 8 / 568 | 16.501 / 22.700 / 22.700 | 4 | 50 / 4 | 254864 | 3.227 |

Every steady fixture has pending=0 and processed=0 on further ticks. Water never
starts terrain generation. Maximum processing is256 cells/tick regardless of queue
size. Mesh payload is logical CPU/GPU vertex+index bytes, excluding allocator,
asset, driver and duplicate staging overhead. Terrain56B/vertex; water48B/vertex.
Blocks remain1B; per-chunk water counters and empty scheduler metadata are small
fixed additions. Actual resident memory is recorded above, separately from mesh
payload. Regional overlays and their COW/save memory retain existing bounds.

### Regional overlays

The same release fixture with `--regional` uses actual regional COW maps,
region-ready validation, dirty-region backpressure and mesh-revision invalidation.
All fixture columns are resident; no disk/streaming loads occur during timing.
These additional measurements ran after compilation completed, without concurrent
builds/tests. Raw data: [cpu-regional.csv](cpu-regional.csv); process CPU/RSS:
[cpu-regional-resource.txt](cpu-regional-resource.txt).

| Scene | Tick P50 / P95 / max µs | Stable tick ns | Processed updates |
| --- | --- | --- | --- |
| dry | 0.400 / 0.400 / 0.400 | 4 | 0 |
| pool | 10.096 / 11.899 / 11.899 | 4 | 52 |
| waterfall | 12.033 / 52.751 / 52.751 | 4 | 890 |
| several | 103.217 / 186.036 / 186.036 | 8 | 1987 |
| boundary | 38.229 / 52.321 / 52.321 | 4 | 568 |

The multi-source regional maximum was186µs (0.186ms), compared with67µs in the
finite fixture. This accounts for real overlay mutation; disk saves and dense
concurrent save-snapshot COW remain outside this microbenchmark. Stable regional
water also processes zero updates. For reproduction append `-- --regional` to
`cargo run --release --locked -p voxel_core --example water_bench`.

## Active rendered fixture

`water_gallery --active` deliberately pulses the same explicit sources every
second, exercising expansion/recession at5Hz with budget256. It uses production
core simulation/cached-light water meshing and **one** off-thread water mesh job,
retains GPU handles and never queues workers. The standalone gallery isolates
this work; it is not a full production session/streaming/endurance benchmark.
The raw `active-*.txt` observations record frame percentiles, actual processed
updates, maximum simulation µs, peak pending requests, worker count and RSS.
Source pulses are measurement tooling only, not gameplay mechanics.


| Active scene | P50 / P95 / P99 ms | Processed updates | Max tick µs (debug) | Peak pending | Mesh jobs | Max RSS MiB |
| --- | --- | --- | --- | --- | --- | --- |
| waterfall | 99.69 / 151.41 / 274.36 | 18339 | 5808.425 | 366 | 96 | 332.0 |
| several | 106.90 / 142.87 / 173.21 | 27896 | 3972.684 | 973 | 111 | 352.2 |
| boundary | 95.07 / 232.69 / 313.32 | 12845 | 4679.630 | 280 | 93 | 352.4 |

These active debug/software runs show occasional long frame tails (up to313ms
P99 in the seam fixture). The separate release CPU fill tests remain below67µs
per tick in these small scenes; that does **not** erase the active debug renderer
observations. Source pulsing, worker snapshot/install, alpha rasterization and
software scheduling share CPU resources. Full production-session native GPU
profiling is necessary before claiming the owner's frame-time target is met.
The active fixture stays at one mesh job in flight and peak pending below1,000;
all source pulses are explicit and cannot create new permanent sources.

## Reproduce

```sh
# From the Slice22 working tree, export the accepted Slice21 dry harness:
python tools/prepare_water_baseline.py work/water-baseline
CARGO_TARGET_DIR="$PWD/target" cargo build --locked --manifest-path work/water-baseline/Cargo.toml -p rustcraft --example water_gallery
cp target/debug/examples/water_gallery work/water-gallery-slice21
cargo build --locked -p rustcraft --example water_gallery
cp target/debug/examples/water_gallery work/water-gallery-slice22
python tools/measure_water.py work/water-gallery-slice21 work/water-gallery-slice22 work/water-metrics --frames 180
cargo run --release --locked -p voxel_core --example water_bench
# For an active renderer workload:
target/debug/examples/water_gallery several 2 --time=6000 --active --frames=120
python tools/capture_water.py --output work/water-images
```

For cloud reproduction source `/workspace/setup/env.sh`, start Xvfb and select
that environment's lavapipe ICD explicitly. Those overrides are **measurement
configuration only**, never game defaults. The baseline export changes only its
benchmark package version to0.1.21 so Cargo path-dependency artifacts cannot alias
Slice22 artifacts in a shared target directory. No runtime algorithm is changed.

Screenshots in `docs/images/slice22` use the actual production shaders/mesher,
original fixture art, same software renderer and fixed daylight6000/night18000.
No screenshot was generated by an image model. The channel and waterfall fixtures
are constructed by test code, rather than claiming manual native gameplay input.

## Acceptance boundaries

Automated assertions cover source stability/removal, bounded spreading/obstacles/
channels, deterministic multi-source budgets, unloaded frontier/load order/return,
old and water save formats, regional journal interruption/replay, source inventory
transactions, water physics/land restoration, shared mesh heights/culling/light,
retained blended assets and pure-water jobs with no new light work/terrain upload.
The unchanged CI matrix passed on both Ubuntu and Windows for code commit
`5c7a21d`: [workflow run134](https://github.com/ArturTrumpfheller/rustcraft/actions/runs/40227424085).
Local validation: fmt, Clippy with warnings denied,184 client +210 core tests
passed (three existing ignored tests), texture regeneration check and the
required release build all passed. See [validation.txt](validation.txt). Physical Void X11/Wayland,
Windows GUI, native RX9060 XT frame times, long travel with dense water, and
transparency from all camera configurations remain manual acceptance; see WATER.
Standard mesh-level alpha sorting can show errors in overlapping waterfalls.
