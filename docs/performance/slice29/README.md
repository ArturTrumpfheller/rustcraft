# Slice29 validation, measurements and acceptance

Implementation contract: [VEGETATION](../../VEGETATION.md). Baseline is merged
Slice28 **a3acd940f9fb0a4ca4bcabde555077097ac0ff6e**, not a feature branch.
Measurements below are release CPU measurements in this x86-64 cloud container
with a two-CPU cgroup quota, not native RX9060 XT GPU results.

## Executed checks

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo test --locked --workspace`: **310 core +196 client passed**,5 explicitly ignored measurement tests; all historical generator goldens retained.
- `cargo build --release --locked -p rustcraft`: passed.
- `python3 tools/generate_textures.py --check`: passed; all11 historical source tile grids/palettes were additionally compared with the baseline and remain exact.
- Final focused floating-origin regression: terrain/water/plants all stay aligned through far-coordinate rebases; F3 tests passed. Clippy/format/release checks were repeated after that fix.
- The release streaming measurement was explicitly run with `--ignored --test-threads=1` and passed.
- Linux/Windows GitHub Actions status is recorded on the PR. Native desktop acceptance remains below.

No dependencies, Bevy features, toolchain, save envelopes or historical golden
values were changed. Tests updating the default generator and next unknown ID
now expect9/31 respectively; historical known-output fixtures remain unchanged.

## Matched core measurements

`tools/benchmark_vegetation.py` exports the exact baseline and copies the current
core into disposable source trees. Identical timers wrap the real tree/plant
planner, light solve and mesher; the same requested-heap meter instruments both.
Production code has no timers.48 raw full-height columns and24 warm worker jobs
(after4 warmups) are sampled per scene; the cold job is also retained. Percentiles
use the sorted index `(N-1)*p/100`. These are small bounded workloads, with normal
cloud scheduling variation, not statistically certified native frame benchmarks.
The final run had no concurrent renderer/build/test work.

Scene names describe the **v9** target. Empty Plains has no small plants in the
16×16 target, though its required halo can contain detail. The shoreline target
(seed42, X/Z=-416/48) has15 small plants and476 water cells in surface slabs;
it is not an all-water target. `raw/sites.csv` is a read-only three-seed survey.

Times are milliseconds. Cells show **Slice28 → Slice29**; absolute coordinates,
cold jobs and every measurement are in [slice28.csv](raw/slice28.csv) and
[slice29.csv](raw/slice29.csv).

| Scene | Raw P50 | Raw P95 | Raw P99 | Warm worker P50 | Worker P95 | Worker P99 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Empty Plains | 0.315 → 0.304 | 0.420 → 0.484 | 0.483 → 0.519 | 1.508 → 1.929 | 1.679 → 2.154 | 1.760 → 2.276 |
| Flowering Plains | 1.022 → 0.401 | 1.250 → 0.558 | 1.368 → 0.596 | 1.345 → 1.496 | 1.995 → 1.679 | 3.050 → 1.773 |
| Sparse Forest | 3.077 → 0.419 | 3.339 → 0.556 | 3.845 → 0.587 | 1.292 → 1.407 | 1.453 → 1.539 | 1.502 → 1.613 |
| Dense Forest | 2.400 → 0.806 | 2.719 → 1.163 | 2.832 → 1.721 | 1.339 → 1.373 | 1.522 → 1.588 | 1.524 → 3.115 |
| Snow / Spruce | 1.180 → 0.670 | 1.359 → 0.803 | 1.646 → 1.582 | 1.528 → 1.510 | 1.676 → 1.692 | 1.812 → 1.847 |
| Desert shrubs | 0.384 → 0.368 | 0.575 → 0.617 | 0.758 → 0.790 | 1.506 → 1.488 | 1.620 → 1.797 | 1.709 → 1.810 |
| Rocky Highlands | 0.337 → 0.435 | 0.442 → 0.847 | 0.508 → 2.802 | 1.303 → 1.502 | 1.569 → 1.724 | 1.749 → 1.772 |
| Vegetated lake shore | 2.185 → 0.436 | 2.487 → 0.552 | 2.641 → 0.602 | 1.551 → 1.550 | 1.811 → 1.825 | 1.864 → 1.852 |

V9 stage times and resource sizes follow. Planning, mesh and light times are P50
microseconds. Vertices/indices include opaque, water and cutout buffers. Surfaces
are nonempty chunk meshes (an approximate draw-count proxy), not a GPU driver
capture. The light/mesh timers exclude generation of pristine source columns.

| Scene | Tree / plant µs | Mesh / light µs | Vertices / indices | Surfaces | Peak worker heap KiB | Terrain cache / vegetation plan bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Empty Plains | 1.5 / 12.9 | 372.8 / 1446.2 | 1120 / 1680 | 4 | 705.0 | 106496 / 64 |
| Flowering Plains | 1.3 / 17.4 | 403.5 / 956.2 | 3456 / 5184 | 5 | 691.3 | 98304 / 1024 |
| Sparse Forest | 1.0 / 15.1 | 376.9 / 904.9 | 2056 / 3084 | 6 | 643.2 | 131072 / 256 |
| Dense Forest | 454.0 / 20.0 | 340.4 / 881.8 | 1632 / 2448 | 4 | 510.3 | 73728 / 768 |
| Snow / Spruce | 158.6 / 14.9 | 344.7 / 1034.7 | 3244 / 4866 | 6 | 837.3 | 139264 / 256 |
| Desert shrubs | 0.9 / 12.9 | 359.1 / 996.9 | 2604 / 3906 | 4 | 690.5 | 118784 / 128 |
| Rocky Highlands | 71.4 / 14.2 | 345.0 / 1057.0 | 1716 / 2574 | 5 | 630.9 | 102400 / 384 |
| Vegetated lake shore | 23.4 / 6.1 | 554.4 / 884.7 | 5516 / 8274 | 7 | 933.5 | 135168 / 512 |

The heap meter reports peak **additional requested heap bytes** during a worker
job, excluding allocator metadata, stacks, GPU/driver storage and preexisting
world/cache allocations. Resident voxel bytes, water-pool cache bytes, exact mesh
payload bytes and core installation costs are also retained in both CSVs.
Vegetation plan bytes are sampler-owned transient metadata; no world-sized
vegetation registry is retained. Existing terrain-cache entry bounds remain.

Significant changes are reported explicitly:

- Dense Forest raw P50 improves **2.400 →0.806 ms** (about66%) because candidate roots share bounded hydrology/cave/ground plans instead of repeatedly rebuilding point geography. Shore and sparse-forest generation also improve substantially.
- Empty Plains warm-worker P50 rises **1.508 →1.929 ms** (about28%); light solve P50 rises1.048 →1.446 ms. V9 surrounding/taller vegetation changes the actual source-light workload, even where the target has no small plants. Its target vertex/index payload remains exactly69440 bytes in both versions.
- Rocky raw P50 rises **0.337 →0.435 ms** (about29%,98 µs), and its sampled raw P99 rises0.508 →2.802 ms. Its warm-worker P50 rises1.303 →1.502 ms. This remains bounded, but the tail is a recorded regression for later performance work.
- Dense Forest target mesh payload rises24304 →101184 bytes as the v9 site has more/different trees and ground detail. Warm-worker P50 rises1.339 →1.373 ms. Other payload changes and peak heap differences are visible in the CSVs; no correctness was weakened for these results.
- Each actual plant contributes only8 vertices/12 indices to one chunk cutout mesh. Empty targets allocate no decorative vertex/index buffers. Uniform Air/Stone/Water descriptors remain compact.

## Streaming, eviction, edits and save/reload

The actual release client ECS benchmark runs8 visits across forest chunk
boundaries at radius1, including returning to evicted columns. Seed42 starts
around chunks(48,-248) and moves across49/50/51. Both versions use the current
client; the **exact merged-source baseline comparison** is the separate core
harness above. Every visit asserts bounded chunks, images, meshes and correct
payload ownership. Main-install time measures CPU publication into retained Bevy
assets/commands, not a GPU-upload stall.

| Generator | Visit P50 / P95 / P99 ms | Max main install ms | Max queue-to-install ms | Retained mesh bytes | Plant mesh entities |
| --- | ---: | ---: | ---: | ---: | ---: |
| 8 | 14.404 / 55.311 / 55.311 | 0.545 | 71.653 | 1041600 | 0 |
| 9 | 10.280 / 33.671 / 33.671 | 0.549 | 41.812 | 1076568 | 7 |

[Complete release ECS output](raw/streaming.txt). The changing-radius/eviction,
stale-result, water-only publication, lighting, rebase and session-cleanup tests
cover all three mesh surfaces. Worker generation/remeshing remain asynchronous.

The separate24-sample v9 edit workload removes actual generated plants through
inventory transactions, saves/reloads manually placed new vegetation and inventory,
and evicts/regenerates the edited column. Every iteration verifies saved Air,
player items and the manually placed Yellow Flower. It uses disposable local
save directories and the real paired save protocol; filesystem caches affect the
reported microsecond timings. No generated baseline edit records are written.

| Operation | P50 / P95 / P99 µs |
| --- | ---: |
| plant-inventory-break | 0.90 / 0.95 / 0.96 |
| paired-save | 83.28 / 107.98 / 142.62 |
| paired-reload-and-column | 459.46 / 535.21 / 680.06 |
| eviction-regeneration | 422.06 / 515.62 / 569.19 |

[Edit raw data](raw/edits.csv). Plant-removal timings are per plant within each bounded sample.

## Genuine renderer captures

All eight images are actual RustCraft1280×720 production-material captures,
not concept art. `terrain_gallery` is read-only, with original assets, generator9,
FOV78°, time6000, and no planted screenshot fixtures. The gallery's explicit
`--survey-fog` uses96/160-block fog distances for context; other lighting/fog
code is the production code.60 frames and at least10 seconds precede capture.
Mesa llvmpipe/lavapipe Vulkan under Xvfb was selected explicitly in this cloud
container. These images verify appearance, not native AMD performance. Raw
camera commands and actual renderer output are in `raw/capture-*.txt`.

| Image | Seed | Center X/Z | Eye X/Y/Z | Direction X/Y/Z | Radius | Biome |
| --- | ---: | --- | --- | --- | ---: | --- |
| [Plains, grass and flowers](../../images/slice29/plains-flowers.png) |0| -2432,-4096 | -2424,9,-4082 | -6,-3,-10 |3| Plains |
| [Mixed Oak/Birch forest](../../images/slice29/mixed-forest.png) |2026| -896,-4096 | -878,13,-4068 | -12,-7,-22 |3| Forest |
| [Birch close-up](../../images/slice29/birch-close.png) |42| -1591,-4095 | -1584,7,-4088 | -7,0,-7 |2| Plains |
| [Spruce close-up](../../images/slice29/spruce-close.png) |42| -755,-4023 | -745,8,-4013 | -10,0,-10 |2| Snowy Plains |
| [Snow and Spruce](../../images/slice29/snow-spruce.png) |0| -4096,-4096 | -4077,10,-4078 | -13,-3,-16 |3| Snowy Plains |
| [Desert shrubs](../../images/slice29/desert-shrubs.png) |2026| 3456,-3968 | 3463,8,-3949 | 0,-3,-15 |3| Desert |
| [Vegetated lake shore](../../images/slice29/lake-vegetation.png) |42| -434,31 | -397,16,62 | -32,-12,-26 |4| Forest |
| [Groves and clearings by water](../../images/slice29/density-transition.png) |0| -2176,-4096 | -2190,17,-4050 | 20,-10,-32 |4| Forest |

The pale marked Birch trunk and compact four-layer crown differ from the original
Oak. Spruce has a taller trunk, wide lower tiers, inset lower foliage and a
narrowing tip; its silhouette differs from both broadleaf species. Snow has sparse
ferns, desert shrubs have no green foliage, and the lake retains its v8 geometry.

## Reproduce on Void Linux / Fish

Use Void glibc x86-64 with the native dependencies in [PLATFORMS](../../PLATFORMS.md),
Rust1.99.1 and supported Mesa/Radeon drivers. Run in a real X11/Wayland session:

```fish
git fetch origin
git switch feat/slice-29-vegetation-world-details
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --release --locked -p rustcraft
python3 tools/generate_textures.py --check

# Read-only gallery: never opens a save directory.
cargo build --locked -p rustcraft --example terrain_gallery
python3 tools/capture_vegetation.py --output /tmp/rustcraft-slice29-gallery

# Exact Slice28/current release CPU comparison (Python3.12+).
python3 tools/benchmark_vegetation.py --output /tmp/rustcraft-slice29-bench
cargo run --release --locked -p voxel_core --example vegetation_edits_bench
cargo test --release --locked -p rustcraft --bin rustcraft vegetation_streaming_measurements -- --ignored --nocapture --test-threads=1

# Disposable gameplay data; the menu still creates/selects the world explicitly.
set -l slice29_data (mktemp -d /tmp/rustcraft-slice29.XXXXXX)
env RUSTCRAFT_DATA_DIR=$slice29_data RUSTCRAFT_WORLD_SEED=0 RUSTCRAFT_START_CHUNK=-152,-256 cargo run --release --locked -p rustcraft
```

The override seed prefills new-world creation and never changes an existing world.
Record the temporary path for restarting. Use the same seed and start chunk for
matching captures; F3 displays the world coordinates and generator. Examples:
seed2026/start`-56,-256` for mixed forest; seed42/start`-48,-252` for snowy Spruce;
seed2026/start`216,-248` for desert shrubs; seed42/start`-26,3` for a vegetated shore.
Use the gallery for the exact camera vectors above, since ordinary gameplay has
no flight/teleport camera.

- [ ] Create a new world; confirm generator9 and the five biome identities in F3.
- [ ] Inspect all8 gallery scenes and the three different species silhouettes.
- [ ] Walk/sprint/jump through every small plant; logs/leaves remain solid.
- [ ] Mine and place all9 additions from both rows of the E item palette. Confirm recognizable icons/held sprites, stack counts and top/side wood textures.
- [ ] Place Fern on snowy ground, shrubs on sand, flowers on Grass/Dirt; invalid support and water-filled placement must reject without consuming inventory.
- [ ] Mine supporting ground and verify one atomic ground+plant collection, including a vertical chunk seam. A full inventory must reject the entire removal.
- [ ] Pour water into plants: verify replacement and one collectible item per washed plant. Collect with a nearly full inventory; no duplication or silent loss.
- [ ] Remove generated grass/flowers and parts of each tree; place new plants/logs. F5, leave the area until eviction, return, close and reopen. Saved Air/placements/items must persist.
- [ ] Test an isolated complete copy of a generator1–8 save. Terrain/trees/water must retain its original generation; manually placed new materials must save/reload.
- [ ] Explore forest chunk seams and repeatedly cross radius boundaries. Check for geometry holes, cutout square backgrounds, water regressions and retained mesh/entity growth.
- [ ] Change time and render radius; plant sky/block brightness and fog must follow terrain. Existing torch light and water transparency must remain correct.
- [ ] Verify capture/unlock, focus, resize, inventory/pause and final save in native Void X11 and Wayland, then Windows10/11. Record GPU, driver, backend, radius and frame statistics.

## Remaining limits

Native Void/RX9060 XT, Wayland and Windows GUI/performance acceptance remains for
the owner. Cloud software Vulkan measurements are not native GPU evidence. The
CI matrix checks Linux and Windows builds/tests, not GUI input or physical GPU
performance. No generator/save migration, tree growth, leaf decay, saplings,
underwater plants, crafting, animation or terrain redesign is included.

Opaque leaves and nearest-LOD0 sprite shimmer are intentional current limits.
Water follows the existing bounded drop-capacity backpressure policy: a full4096
world-item list defers washing until capacity is available, preserving ownership
and retrying rather than losing items. Dense groves naturally add opaque tree
surfaces and cutout vertices; the following performance slices can optimize them
without changing this generator identity.
