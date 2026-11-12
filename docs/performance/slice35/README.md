# Slice 35 — Terrain Generation v3

Baseline: exact merged Slice34 `aab49019b5d4f2ceccef1b748c458df46375cc4b`.
New worlds select generator10. Existing generators1–9, original assets, stable
block/item IDs, save envelopes, X/Z extent and Y[-64,192) remain. No migration or
automatic upgrade occurs. Every historical golden fixture remains in the suite.

## Architecture and geographical goals

[WORLDGEN](../../WORLDGEN.md) specifies the integer fields and compatibility paths.
The geography combines continental elevation, separately modulated regional
uplift and relief suppression, rounded connected ridge bands, broad valley cuts,
foothills and quiet plains. This differs from amplitude scaling: a valley field
can cut through an uplift region, erosion-inspired weights can leave an elevated
plateau, and lowlands independently suppress small hills. Domain warping retains
sub-block precision so mountain amplitudes do not amplify integer warp steps.

All decisions use explicit coordinate hashes/fixed-point integer arithmetic.
Q12 multiplication uses bounded i64 intermediates; terrain coordinates use Q8.
The theoretical height upper envelope is below179, leaving ceiling headroom.
The model has no hard summit cap or mutable random stream. Runtime planning and
all arrays remain bounded by the existing local sampler. V10 adds a1296-byte
final-slope vector per water plan; historical versions allocate none of it.
Uniform sky/Stone chunks stay compact. Streaming workers/caches/publication,
cancellation, eviction and supported radii are unchanged.

V10 materials use final surface slope/elevation; steep/high ledges expose Stone,
snow follows continuous climate/altitude, and shores/deserts reuse Sand. The five
biome IDs and their climate geography remain frozen. V9 species and footprint
clearance use the actual final bed, reject wet/unsupported roots and respect the
ceiling. Explicit Air over decorations remains authoritative after save/reload.

The original v8 sea datum/basin/river/cave-pool mechanics remain. Padded terrain
samples reject high basin candidates and full river curves before installing
their endpoint basins. No per-chunk or per-column river acceptance is used.
The terrain field owns the continuous continental shelf rather than subtracting
the old ocean field from tall ranges. The existing four-block watertight cap and
closed-pool flood proof remain. V7 cave shapes are unchanged: mountain overburden
is solid, and some entrances terminate underground. No new cave topology, river
network, flowing-water rules, art, shader or gameplay feature is introduced.

V10 feature admission caches at most128 coordinate-keyed booleans in a FIFO beside
its existing bounded pool cache. Pure evaluation occurs outside the mutex;
parallel duplicates are safe and cache eviction/order cannot affect output.
Historical versions never populate this cache. Cache-capacity and uncached-parity
regressions cover all four seeds through repeated distant-region eviction.

Initial profiling found repeated whole-route admission in flat regions. The
bounded outcome cache removes that duplicated work; final material assignment
also reuses the pre-water climate grid. Fixed-scale field inlining eliminates
avoidable dynamic lattice divisions without changing integer outputs.

Compatibility bank adaptation widens v10 dry basin rims (squared-radius limit
2560→6144) and river margins7→14 blocks. Candidate/discovery bounds expand with
the banks; wet cores, source levels and route topology remain. Two observed steep
legacy-rim sites (seed0:-3776,-3648 and u64::MAX:960,-704) have dedicated regressions.
V10 also retains sub-block shoreline offsets and segment projection precision:
Q8 basin warps / Q16 river projection prevent a one-cell move from snapping the
nearest centerline point by a whole block. Historical versions retain their
original bank formula, projection and discovery bounds.

## Reproducible quality evaluation

```sh
cargo run --release --locked -p voxel_core --example terrain_quality > work/v10-raw.csv
cargo run --release --locked -p voxel_core --example terrain_quality -- --final --step=64 > work/v10-final.csv
cargo run --release --locked -p voxel_core --example terrain_quality -- --final --step=64 --generator=9 > work/v9-final.csv
python3 tools/analyze_terrain_v3.py work/v10-raw.csv --output work/v10-raw.json
python3 tools/analyze_terrain_v3.py work/v10-final.csv --output work/v10-final.json
python3 tools/terrain_v3_reference.py --check
cargo test --release --locked -p voxel_core --test terrain_v3
cargo run --release --locked -p voxel_core --example worldgen_audit -- 10 42 -180 45
```

Each seed0/42/2026/u64::MAX samples an8192² central area and two2048² distant
areas at(-24000,20000) and(28000,-30000). The16-block macro grid gives294,912
observations per seed /1,179,648 total; final hydrology comparison uses64-block
spacing and actual cardinal one-block slopes at every sampled point. Heights,
16-block neighbor differences and one-block slope maxima are retained. Connected
four-neighbor components are sampling proxies, not proofs that every intervening
voxel is buildable. A32-block flat patch means nine16-spaced heights spanning at
most one block, with initial height4–25. It does not certify cave-free foundations.
A valley measurement compares the center with both opposite shoulders256 blocks
away; it grades512-block spans rather than claiming an automatically found route.

The Python reference independently evaluates the integer pre-water terrain and
commits52 exact height fixtures, including both coordinate limits and all four
seeds. It never reads Rust output to derive expectations. Full-height integration
separately checks materials, vegetation, cave foundations, water stability,
point/chunk/halo parity, reverse/parallel generation and eviction/regeneration.
Existing mine/build/journal interruption and save/reload suites include v10.
Native GUI checks remain distinct from these deterministic tests.

## Measured landscape quality

Same-area final v9 maxima are Y23 for all four seeds. V10 has long upland
components rather than isolated high cells. The raw16-block grid gives:

| Seed | P50 / P95 / P99 / max Y | Area at Y≥70 | Flat32 patch proxy | Largest upland component cells |
| --- | --- | ---: | ---: | ---: |
| 0 | 10 / 85 / 124 / 155 | 7.22% | 36.15% | 2808 |
| 42 | 10 / 97 / 129 / 151 | 9.46% | 32.57% | 4418 |
| 2026 | 8 / 67 / 110 / 151 | 4.64% | 28.82% | 3318 |
| 18446744073709551615 | 7 / 76 / 123 / 154 | 5.90% | 22.42% | 2534 |

Each raw component cell represents a16×16 sampling interval. Seed42's largest
upland component spans4418 cells (about1.13km² of sample footprint). Seed42 valley
(-2880,720) has Y24 between opposite256-block shoulders at Y133/Y135:109 blocks
of relief across a512-block span. This is an actual sampled cross-section, not a
claim that the entire valley floor has one width or elevation.

The final64-block survey after hydrology retains P95 Y67–97 and maxima Y151–153.
The dense raw grid additionally finds Y155. Final one-block slopes are0–1 in over
99% of sampled cells, with eight4-block steps among73,728 samples; the original
five-/six-block findings are removed at their regression sites. Those residual
steep wet-channel steps remain adverse observations for Hydrology v2, not a global
slope guarantee. Final gentle-lowland sample fractions are27.40–43.04%; these
exclude water and rough one-/16-block neighborhoods. They are distinct from the
raw flat32 proxy. Full per-seed distributions and component counts are in
[raw quality](raw/quality-v10-raw.json), [final quality](raw/quality-v10-final.json)
and [matched v9](raw/quality-v9-final.json).

## Captures and representative locations

Captures use the actual Bevy gallery, production meshes/materials/lighting,
original textures and software Vulkan. They are read-only rendered client
fixtures, not synthetic heightmaps or interactive streaming-gameplay recordings.
Normal scenes use supported radius4 and production fog. Panorama fixtures use
gallery-only radius10 and explicit wide survey fog160–320; this does not expand
the supported game radius. Generator9 comparisons use its frozen output in the
current gallery; the renderer itself is unchanged. Exact seeds, world camera
positions/directions, center coordinates and commands are retained in captures.json
and individual logs. FOV78,1280×720, world time6000.

```sh
cargo build --locked -p rustcraft --example terrain_gallery
# Set DISPLAY and use the platform-supported adapter normally.
python3 tools/capture_terrain_v3.py --output work/captures35
```

## Performance method

Use no concurrent compilation, tests or renderers during timing. Both source
exports build before any run. Source and executable hashes are retained.

```sh
python3 tools/benchmark_terrain_v3.py --output work/s35-core --samples 128 --warmups 8
python3 tools/benchmark_terrain_v3.py --adverse --output work/s35-adverse-core --samples 128 --warmups 8
python3 tools/benchmark_streaming.py --baseline aab49019b5d4f2ceccef1b748c458df46375cc4b --labels slice34 slice35 --terrain-v3 --distance-expansion --rendering-install --repeats 2 --frames 60 --output work/s35-stream
```

The core harness compares v9/v10 at nine matched full-height locations, including
high peaks, valleys, a distant region and u64::MAX. A separate current-v9 run
asserts identical historical block/light/geometry CRCs and resource counts.
Raw observations retain wall and Linux thread CPU time, full-column generation,
maximum individual chunk time, tree/plant planning, light/mesh stages, allocation
calls/requested bytes/peak requested heap, retained voxel/cache/light payloads,
surface/vertex/index counts and CPU core installation. Requested heap excludes
allocator metadata, stacks and driver/GPU allocations. `column_plan_us` isolates shared sampler planning; `chunk_total_us` and
`chunk_max_us` time the16 chunk calls. Clock overhead is present in both builds.

A separate24-trial lifecycle probe removes exactly eight natural plants from
seed0, column(-136,-256), adds the same supported flower and inventory stacks,
saves/reopens the paired session, evicts and regenerates the column, and asserts
that explicit Air and inventory survive. Raw per-trial break/save/reload/
regeneration times and P50/P95/P99/max are retained; break time is per plant.
The same harness runs against baseline9 and current10, with private temporary
directories only.

The existing streaming harness uses the actual bounded worker, frame-paced
requests, installation, real Bevy CPU mesh/bounds/visibility publication, travel,
return, cancellation, live radius changes, edits, world switching and shutdown.
V10 adds mountain/valley cases. Radii2/3/4 remain matched to the exact baseline;
P50/P95/P99/max and all raw adverse observations are retained. Headless synthetic
motion sets absolute positions; it is a scheduler/resource workload, not a claim
that a walking player can traverse that precise route without climbing.

No native GPU execution, FPS, Radeon throughput, driver allocation or Windows GUI
performance is claimed. Software-renderer capture frame intervals are not CPU
benchmark observations and are not used to justify performance conclusions.

## Measured CPU, memory and streaming results

Both orders were run without concurrent compilation, tests or rendering.
Core:128 retained samples +8 warmups for each of nine full-height locations/build;
6912 core observations including current-v9 parity across both orders. Lifecycle:
24 trials/build/order (96 total). Streaming:21 final-state workload keys × radii1–4
× two repeats/build/order (672 final states), with60 frame-paced motion updates per
non-session case. Sessions include12 A/B switches per case. The benchmark has no
RenderApp/GPU and does not measure gameplay FPS.

The exact baseline already supports radius4. Terrain mode therefore compares
matching radii1–4 and permits different geometry/resource signatures. An initial
aggregation assertion still assumed the earlier Slice33→34 radius expansion;
aggregation was corrected using the existing complete raw observations. No Rust
measurement, source fingerprint or adverse sample was discarded or rerun for that
correction. `method.json` retains the aggregation script hash separately.

Full distributions, sample counts, allocations, stage costs and every maximum:
[core forward](raw/core-forward/percentiles.csv),
[core reverse](raw/core-reverse/percentiles.csv),
[stream forward](raw/stream-forward/percentiles.csv),
[stream reverse](raw/stream-reverse/percentiles.csv).
Compressed raw observations and exact source/executable hashes are beside them.
All numeric console samples are retained in `samples.csv.gz`; the console logs
retain states/test results without duplicating those same sample lines.
[Cross-order final-signature equality](raw/cross-order-parity.json) covers all168
workload keys per build.
[Final-source equality](raw/measured-source-scope.json) checks every measured production Rust
source against the delivered working tree. Aggregation-only Python changes do not
affect the timed Rust binaries.

Core forward, **milliseconds**. Median raw generation CPU and warm complete-worker
CPU compare exact v9→v10 locations. V10 warm-worker quartets retain P50/P95/P99/max.

| Scene | Raw generation CPU P50 v9→v10 | Warm worker CPU P50 v9→v10 | V10 warm worker P50 / P95 / P99 / max |
| --- | ---: | ---: | --- |
| plain | 0.276→0.449 | 0.733→0.757 | 0.757 / 1.308 / 2.207 / 2.556 |
| forest | 0.771→0.518 | 0.536→0.662 | 0.662 / 0.880 / 0.950 / 1.283 |
| peak | 0.316→0.293 | 0.338→0.650 | 0.650 / 0.854 / 1.031 / 1.245 |
| valley | 0.246→0.222 | 0.237→0.256 | 0.256 / 0.370 / 0.420 / 0.482 |
| transition | 0.279→0.220 | 0.451→0.325 | 0.325 / 0.432 / 0.533 / 0.533 |
| coast | 0.249→0.246 | 0.211→0.462 | 0.462 / 0.564 / 0.717 / 0.798 |
| distant | 0.340→0.321 | 0.579→0.627 | 0.627 / 0.819 / 1.148 / 1.648 |
| max-seed | 0.419→0.362 | 0.622→1.013 | 1.013 / 1.195 / 1.378 / 1.411 |
| cavern | 0.444→0.395 | 0.951→0.608 | 0.608 / 1.322 / 3.842 / 5.977 |

Raw generation CPU medians are lower at eight of these nine locations in both
orders; the plain case rises about61–62% (roughly0.17ms/column). This is a real
remaining cost of the new field/material model, not a blanket speedup. Whole-route
admission reuse and fixed-scale specialization were investigated before timing.
Warm worker medians rise at the peak/coast/max-seed sites because the generated
light/mesh workload changes. Peak raw generation is lower while its warm worker
CPU is higher; the stage/resource tables distinguish those costs.

Lighting/meshing and compact storage, same forward locations. Bytes are logical
CPU payloads, not GPU allocation. Compact count includes Air/Stone/other uniform
slabs out of16; the full CSV retains each subtype and allocation high-water.

| Scene | Light P50 ms v9→v10 | Meshing P50 ms v9→v10 | Mesh bytes v9→v10 | V10 compact slabs | V10 allocation calls / peak requested heap bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain | 0.337→0.430 | 0.076→0.199 | 55,792→111,352 | 14 | 244 / 632,264 |
| forest | 0.286→0.350 | 0.148→0.201 | 101,184→113,336 | 14 | 242 / 533,832 |
| peak | 0.208→0.463 | 0.060→0.081 | 3,968→12,648 | 15 | 107 / 1,155,784 |
| valley | 0.111→0.112 | 0.073→0.087 | 23,560→27,776 | 15 | 131 / 199,496 |
| transition | 0.252→0.176 | 0.105→0.081 | 20,088→1,488 | 15 | 86 / 478,536 |
| coast | 0.106→0.188 | 0.055→0.102 | 496→71,416 | 14 | 157 / 304,072 |
| distant | 0.283→0.270 | 0.201→0.245 | 142,352→161,200 | 13 | 281 / 586,196 |
| max-seed | 0.338→0.633 | 0.192→0.252 | 116,808→122,760 | 12 | 285 / 1,184,200 |
| cavern | 0.402→0.312 | 0.277→0.212 | 246,008→140,368 | 13 | 304 / 505,740 |

The meshing column above is the opaque stage; water/cutout stage distributions,
per-chunk maximum/total time, shared column planning, surface counts and exact
light/cache bytes remain in the full tables. Taller terrain increases retained
work at some sites, while changed natural content reduces it at others.

Additional adverse natural slope: seed42, chunk(-62,-218), the photographed
flank at(-992,-3488). This deliberately avoids assessing only rounded summits.
It uses a separate verified build and128 samples +8 warmups/build/order, adding
768 core observations including v9 parity. Full [forward](raw/adverse-core-forward/percentiles.csv)
and [reverse](raw/adverse-core-reverse/percentiles.csv) distributions remain separate
from the original nine-scene measurements.

| Steep flank | v9 | v10 |
| --- | ---: | ---: |
| Generation CPU P50, forward / reverse ms | 0.275 / 0.276 | 0.352 / 0.348 |
| Warm worker CPU P50, forward / reverse ms | 0.432 / 0.495 | 0.954 / 0.944 |
| Warm worker CPU P99, forward / reverse ms | 0.571 / 0.798 | 1.338 / 2.353 |
| Warm worker CPU max, forward / reverse ms | 0.719 / 1.077 | 1.382 / 3.365 |
| Logical column mesh bytes | 14,384 | 148,800 |
| Compact slabs /16 | 15 | 13 |
| Peak requested worker heap bytes | 503,112 | 1,042,632 |

This flank has about10.3× the small baseline column mesh, with roughly doubled
light/mesh CPU and requested heap. It remains a bounded three-materialized-slab
column rather than a dense256-layer allocation. This is a per-column test, not
a radius4 GPU or whole-view memory measurement. The extra cost is visible in the
evidence; no average across lowlands hides it.

Streaming table: **largest per-scene P99**, milliseconds, v9→v10 at each matching
radius. These are maxima over case distributions, not pooled averages or a
claim of identical geometry. All cases retain their own P50/P95/P99/max in CSV.

| Order / radius | Worker CPU | Main installation | Streaming Update | Request ready |
| --- | ---: | ---: | ---: | ---: |
| forward / 2 | 12.094→6.451 | 1.499→0.483 | 2.203→0.542 | 219.481→201.177 |
| forward / 3 | 8.110→5.193 | 1.492→0.415 | 7.018→0.624 | 269.793→204.203 |
| forward / 4 | 7.320→5.391 | 1.883→0.450 | 1.849→0.931 | 327.297→288.070 |
| reverse / 2 | 7.345→6.862 | 1.181→0.546 | 1.335→1.385 | 190.457→202.614 |
| reverse / 3 | 8.953→8.878 | 1.167→1.768 | 1.892→3.167 | 216.629→213.130 |
| reverse / 4 | 14.561→6.404 | 1.644→1.289 | 1.844→2.027 | 304.292→289.343 |

Radius4 final resources are identical across measurement orders for geometry and
logical payload. Surface count sums retained opaque/water/cutout mesh slots.

| Scene | Surfaces v9→v10 | Mesh bytes v9→v10 | Resident voxel bytes v9→v10 | Exact light bytes v9→v10 |
| --- | ---: | ---: | ---: | ---: |
| stationary | 353→365 | 8,985,160→8,778,040 | 831,488→872,448 | 2,373,624→2,431,944 |
| forest | 348→384 | 8,758,312→10,098,760 | 720,896→864,256 | 2,216,160→2,426,112 |
| coast | 325→358 | 12,950,232→9,119,184 | 999,424→819,200 | 2,315,304→2,233,656 |
| mountain | 286→322 | 5,590,512→9,077,296 | 708,608→835,584 | 2,111,184→2,472,768 |
| valley | 231→276 | 3,510,192→5,884,544 | 557,056→675,840 | 1,837,080→2,052,864 |
| cavern | 359→365 | 16,192,408→10,438,248 | 1,126,400→892,928 | 2,356,128→2,309,472 |
| dense-construction | 357→369 | 9,380,720→9,172,608 | 847,872→888,832 | 2,396,952→2,455,272 |

The radius4 mountain mesh grows about62%, and the valley about68%; these content
costs are retained rather than hidden by smaller coast/cavern payloads. The
single worker,1296 resident descriptor bound,121-column pristine halo cache and
128-decision admission cache remain bounded. Maximum v10 process HWM across
both runs is53,552KiB (about52.3MiB); baseline maximum57,368KiB. HWM includes prior
cases within the process and is not a per-scene heap/VRAM measurement. No failed
terrain job is reported, and drained queue/first-dispatch assertions pass.

Adverse observations remain: v10 worker wall33.245ms (gated teleport/radius4,
reverse), worker CPU28.077ms (teleport/radius4, reverse), installation10.278ms
(teleport/radius4, reverse), and streaming Update18.595ms (same case). Forward
editing/radius3 records27.818ms worker CPU. Baseline also has unfavorable tails:
36.977ms worker wall,16.588ms installation and27.632ms Update in different cases.
Unpaired maxima do not isolate a cause. Core reverse coast worker CPU reaches
7.820ms; forward cavern worker P99 is3.842ms versus0.824ms in reverse. Those
variations are preserved, not averaged away. These are measured locations, not
a worst-world latency bound or a guarantee of a native frame deadline.

Paired saving/reloading, **milliseconds**, v10 P50/P95/P99/max (24 trials/order):

| Order | Save | Reload + column | Eviction regeneration |
| --- | --- | --- | --- |
| forward | 0.086 / 0.234 / 0.295 / 0.432 | 1.400 / 1.607 / 1.823 / 2.206 | 1.377 / 1.576 / 1.645 / 3.494 |
| reverse | 0.080 / 0.122 / 0.124 / 0.129 | 1.357 / 1.457 / 1.486 / 1.496 | 1.315 / 1.612 / 1.667 / 2.253 |

Both versions preserve all eight removed plants as authoritative Air and retain
the placed flower/inventory through paired reload and regeneration. Baseline
quartets and all individual observations are retained in each lifecycle CSV.
No native GPU/FPS improvement is inferred from these CPU/resource results.

## Validation and delivery status

Formatting, locked workspace/all-target Clippy, all workspace tests, optimized
workspace regressions, normal locked release build and asset verification pass.
Debug and release each run554 tests (220 client,324 core,7 terrain-v10 integration,
2 historical integration and1 persistence lifecycle); seven optional probes are
ignored by the ordinary suite. The benchmark probes above are run explicitly.
Independent Python fixtures and every historical v1–v9 golden remain intact.
[Command logs](raw/validation/).

The small cloud filesystem required removing obsolete build artifacts and debug
symbols from gallery executables. Assertions, Rust optimization settings, tests,
source or assets were not weakened. Intermediate no-space/aggregation failures
were corrected before the successful checks/evidence. Source exports use disposable
instrumentation; the ordinary game was rebuilt from clean core release artifacts
after measurement. Native Gentoo/Void/Windows GUI/Radeon acceptance remains open.
Linux/Windows GitHub CI status is recorded separately when remote access permits.

## Known limitations and follow-ups

- Biome distribution remains the old climate geography; a Rocky Highlands ID can
  occur in low terrain and climate Forest can extend into foothills. A dedicated
  biome slice should align ecological regions with the new terrain fields.
- Heightfield terrain has no surface overhangs. Ridges are deliberately broad;
  four-block cave roofs and Stone foundations retain their existing guarantees.
- Caves retain their old vertical distribution. Some mountain entrance routes
  become buried; altitude-aware access is deferred to Cave Generation v3.
- Water uses one sea datum. There are no elevated lakes, watershed drainage or
  continuous erosion simulation. Whole-feature sampling is a bounded heuristic;
  further hydrology should replace the lowland admission heuristic with routing.
- Long mountain chains extend beyond radius4 visibility/fog. They are explored
  through traversal. The panorama fixture is explicitly a development capture.
- Native Gentoo/Void/Windows and physical Radeon acceptance remain manual until
  measured on those systems. CI build success cannot establish native GUI behavior.

## Native Gentoo / Void Linux and Windows acceptance

Use the pinned Rust1.99.1/Bevy0.20.0 toolchain and native dependencies/drivers in
[PLATFORMS](../../PLATFORMS.md). Gentoo needs equivalent C/C++/pkg-config, X11,
Wayland/xkbcommon and Vulkan libraries with appropriate Mesa/Radeon USE options.
Build release with `cargo build --release --locked --workspace`. Never benchmark
through compilation or use the only copy of a real world.

Fish on Gentoo/Void (repeat separately on X11 and Wayland):

```fish
git fetch origin
set terrain_root (mktemp -d /tmp/rustcraft-s35.XXXXXX)
git worktree add --detach "$terrain_root/base" aab49019b5d4f2ceccef1b748c458df46375cc4b
cargo build --release --locked --workspace
# A fresh directory creates v10; keep/record this path for restart.
env RUSTCRAFT_DATA_DIR="$terrain_root/new" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=4 cargo run --release --locked -p rustcraft
# With all instances closed, restart the same world near a representative range.
env RUSTCRAFT_DATA_DIR="$terrain_root/new" RUSTCRAFT_START_CHUNK="-62,-218" RUSTCRAFT_LOAD_RADIUS=4 cargo run --release --locked -p rustcraft
```

Create separate fresh directories for0/2026/u64::MAX. For historical acceptance,
create a v9 template with the exact baseline executable, close normally, copy the
**entire** directory twice, and run baseline/current on the copies. Confirm the
saved generator remains9 and saved Air, constructions, inventory and all regional
geometry match. Do not ask an older binary to open the new v10 world.

PowerShell on Windows10/11 with current supported drivers and MSVC Build Tools:

```powershell
cargo build --release --locked --workspace
$terrainRoot = Join-Path $env:TEMP ("rustcraft-s35-" + [guid]::NewGuid())
$env:RUSTCRAFT_DATA_DIR = Join-Path $terrainRoot "new"
$env:RUSTCRAFT_WORLD_SEED = "42"
$env:RUSTCRAFT_LOAD_RADIUS = "4"
cargo run --release --locked -p rustcraft
# After normal close, keep the same data path:
$env:RUSTCRAFT_START_CHUNK = "-62,-218"
cargo run --release --locked -p rustcraft
```

On each native system:

1. Visit plains/forest, valley, range, transition and shore coordinates from the
   capture manifest. Walk/jump/sprint slopes and ridges; check supported roots,
   snow/rock/soil transitions, banks and caves. Test spawn for all four seeds.
2. Exercise radii2/3/4, rapid signed-seam reversals, distant restarts, peak views,
   live4→1→4 during work, and pending-ground collision safety. Check coherent
   opaque/water/cutout/light publication and resource plateaus over ten minutes.
3. Mine a procedural plant/root/terrain cell to Air, build across a region seam,
   place/remove water with buckets, add a roof/torch, save/reload, evict/return and
   repeat A→B→A world switching during work. Verify inventory, edits and locks.
4. Match seed/location, copied world bytes, camera,1080p/FOV78, time, driver,
   backend, present mode and warmup between builds. Alternate order at least three
   times; keep raw P50/P95/P99/max frame intervals, request/worker/install/update
   times, RSS/private bytes, mesh payload and cache/asset counts. Record adverse
   peaks. PresentMon/OCAT or MangoHud frame intervals are not GPU execution times.
   Use supported native profiling for Radeon GPU/VRAM claims; otherwise leave
   those unmeasured. Check focus/cursor, fullscreen, resize and DPI independently.
