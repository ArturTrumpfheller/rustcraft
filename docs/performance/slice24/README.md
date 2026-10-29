# Slice 24 — Terrain Generation v2 validation

All implementation, examples, measurements and captures are original RustCraft
work, MIT OR Apache-2.0. Baseline is merged Slice 23 `7752287` (PR #41); no dependency,
asset, renderer feature, block ID or physics change is included.

## Release CPU comparison

`tools/benchmark_terrain.py` reconstructs **the actual Slice 23 core source** in an
isolated scratch directory and compiles the identical `terrain_bench` harness for
both revisions. Rust 1.99.1, optimized builds, same cloud host and requested-heap
allocator instrumentation. Three alternating paired runs; each scene uses 120 raw
column samples and 30 cold generation/light/mesh samples after three warmups, plus
32 adjacent new columns with the production sliding cache. Seeds 0/42/2026 each
cover a v4 plain, hill and mountain location. The baseline uses exactly the same
coordinates, not hand-picked different old scenery. Raw CSVs include every seed,
coordinate, percentile, mesh payload and heap measurement.

This is worker CPU wall time, **not frame time, native i3/Radeon performance or an
FPS claim**. Shared-host scheduling produces percentile noise. P99 with 30 samples
has limited resolution. Allocator instrumentation is present in both executables.
No new runtime allocation meter or timer is added to the game.

Full cold column (three target chunks plus nine generated source columns,
lighting and meshes), median of each run's percentiles, milliseconds:

| Seed / v4 scene | Slice 23 P50/P95/P99 | Slice 24 P50/P95/P99 | P50 change |
| --- | ---: | ---: | ---: |
| 0 / plain | 3.656/4.235/4.681 | 4.030/4.617/4.724 | +10.2% |
| 0 / hills | 3.430/3.983/4.280 | 3.981/4.484/4.496 | +16.1% |
| 0 / mountain | 3.461/3.802/3.829 | 4.597/5.116/5.157 | +32.8% |
| 42 / plain | 3.451/3.849/4.000 | 3.351/3.873/3.910 | -2.9% |
| 42 / hills | 3.526/3.989/4.055 | 4.130/4.440/4.514 | +17.1% |
| 42 / mountain | 3.383/3.939/4.961 | 4.817/5.268/5.388 | +42.4% |
| 2026 / plain | 3.297/3.796/4.002 | 3.674/4.153/4.636 | +11.5% |
| 2026 / hills | 3.438/3.851/3.943 | 3.868/4.230/4.247 | +12.5% |
| 2026 / mountain | 3.647/4.664/6.262 | 4.510/5.017/5.038 | +23.7% |

Raw generation excludes lighting/meshing. Mesh payload uses the actual 56-byte
terrain vertex plus 4-byte indices. Light and mesh phases below reuse an already
generated 3×3 source, separating their costs from terrain generation:

| Seed / scene | Raw generation µs, old → new | Light µs, old → new | Mesh µs, old → new | Mesh bytes, old → new |
| --- | ---: | ---: | ---: | ---: |
| 0 / plain | 291.4 → 358.8 | 408.3 → 316.4 | 166.5 → 133.6 | 120528 → 71176 |
| 0 / hills | 274.9 → 304.7 | 370.2 → 358.3 | 148.0 → 132.6 | 80600 → 33976 |
| 0 / mountain | 293.2 → 401.2 | 367.9 → 346.3 | 162.3 → 190.9 | 117056 → 79112 |
| 42 / plain | 307.7 → 318.3 | 367.4 → 329.6 | 181.5 → 115.9 | 142848 → 40672 |
| 42 / hills | 277.9 → 350.6 | 367.9 → 357.1 | 166.7 → 138.8 | 100936 → 54312 |
| 42 / mountain | 269.1 → 387.2 | 370.0 → 333.4 | 163.6 → 178.6 | 103416 → 50840 |
| 2026 / plain | 241.6 → 314.8 | 371.3 → 328.9 | 139.9 → 108.4 | 78120 → 30752 |
| 2026 / hills | 249.7 → 259.7 | 367.9 → 345.8 | 164.2 → 119.4 | 118048 → 12648 |
| 2026 / mountain | 297.9 → 386.7 | 389.0 → 368.1 | 192.5 → 195.4 | 124248 → 62992 |

Regressions are deliberate and measured. Cold mountain jobs rise 23.7–42.4%; raw
mountain generation rises 29.8–43.9%. V4 computes ten noise fields and two smooth
weights instead of three simple layers, including bounded root slope checks.
A cold job generates nine source columns, so repeated raw costs dominate the
increase. Isolated lighting improves 3–23%. Mountain meshing still rises 1.5–17.6%
(despite smaller emitted buffers): taller terrain and face/AO masks change the
fixed voxel scan/merge workload. The increase is about 3–29µs per three-chunk mesh
phase; it does not explain the millisecond cold-job increase. No lighting or
meshing correctness is weakened to improve these results.

Sequential exploration with the cache has P50 changes +3.1–18.7%, generally about
1.6→1.8ms/job. The full P50/P95/P99 are in the CSVs. Queue/install latency and native
frame-time endurance still need the acceptance below. The one-worker design and
bounded cache remain suitable for this measured workload; no worker pool is added.

Construction remains area-independent (~9 ns median, zero persistent height/tree
planning bytes). Spawn preparation times exactly one generated three-chunk origin
column with `prepare_column`, excluding catalog/disk/player recovery and initial
meshing. Its median across nine calls per seed is listed below; these are short
single-call timings with visible scheduling noise, not title-to-play duration.

| Seed | Spawn prep µs, Slice 23 | Spawn prep µs, Slice 24 |
| --- | ---: | ---: |
| 0 | 283.7 | 369.9 |
| 42 | 301.4 | 337.0 |
| 2026 | 256.4 | 286.5 |

## Memory

The global allocator in the **benchmark only** measures peak additional requested
heap bytes over the state immediately before each operation. It excludes allocator
metadata, stack, driver/Bevy/GPU and previously retained buffers. It includes
output vectors alive during generation; these are not an RSS measurement or a
universal worst-case batch bound.

- Raw three-chunk generation including 12,288 output block bytes: old 13,712–13,840B,
  new 13,584–13,712B. Remaining peak is the bounded 18×18 plan/tree allocation.
- Cold full generation/light/mesh job: old 473–600 KB, new 447–473 KB across these sites.
  Mesh output shrinks 32–89% in all nine cases, largely from smoother terrain and
  fewer trees; this does not mean every possible scene has a smaller mesh.
- The 32-column sliding route retains at most 15 pristine source columns /
  **184,320 block bytes** in both revisions. No global visited-column cache is added.
- Existing worst configured pristine cache footprint stays 81 columns /995,328
  block bytes. Resident gameplay blocks stay 307,200 bytes at radius 2 and at most
  602,112 at radius 3. Scratch source/light domains and mesh buffers remain bounded
  by the existing job footprint, independently of world extent. Dense edited
  worlds can have different transient light/mesh costs; no new worst-case RSS
  or native VRAM claim is made.

Reproduce the complete comparison from the repository root (Linux shell):

```sh
python3 tools/benchmark_terrain.py --output work/slice24-comparison --samples 3
cargo run --release --locked -p voxel_core --example terrain_survey
```

The script refuses an existing result directory. Detailed site survey:
[terrain-sites.csv](terrain-sites.csv). Its 32×32 footprints are sampled every four
blocks; valley rows report center height and the lower of the opposing 32-block
flanks rather than a patch min/max. This offline survey never runs in the client.

## Actual RustCraft client gallery captures

These are real 1280×720 X11/Xvfb **llvmpipe/Mesa 25.0.7, LLVM 19.1.7 Vulkan** renders
using the production atlas, terrain shader, day/noon sky, fog, AO and skylight.
They are fixed-camera client gallery scenes, with no menu/hand overlay or saved
world edits. They are not concept art or native Radeon benchmarks. All use the
normal radius 3, vertical FOV 78°, generator v4 and day 0 / time 6000. Hills/transition
use a higher overview eye; other scenes use the ordinary 1.62-block eye above
safe actual terrain. The gallery calls the real edited-block spawn/clearance
routine to avoid placing the camera inside a trunk. Exact resulting feet:

| Scene | Seed | Requested X,Z | Actual feet X,Y,Z | Yaw | View |
| --- | ---: | --- | --- | ---: | --- |
| [Plain](../../images/slice24/plain.png) | 42 | -715,-1200 | -714.5,3,-1199.5 | 45° | ground |
| [Rolling hills](../../images/slice24/hills.png) | 0 | -1744,-2032 | -1742.5,11,-2030.5 | 0° | overview |
| [Mountain flank](../../images/slice24/mountain.png) | 42 | 976,-1440 | 976.5,16,-1439.5 | -120° | ground |
| [Valley](../../images/slice24/valley.png) | 0 | -1440,320 | -1439.5,12,320.5 | 0° | ground |
| [Transition](../../images/slice24/transition.png) | 2026 | 80,-624 | 80.5,19,-623.5 | 0° | overview |
| [Chunk seams](../../images/slice24/seams.png) | 42 | 56,56 | 54.5,7,54.5 | -45° | ground |

Yaw0 points north(-Z),90 east(+X). Mountain regions are modest uplands under the
current 48-block vertical limit. Radius/fog and tree canopies limit how much broad
geography is visible from one ground viewpoint. Traversing the regions is part
of native acceptance; the screenshots do not prove long-distance appearance or
native input/GPU behavior.

```sh
cargo run --locked -p rustcraft --example terrain_gallery -- 42 976 -1440 -120
cargo run --locked -p rustcraft --example terrain_gallery -- 0 -1744 -2032 0 --overview
# Requires a desktop/display and working graphics driver:
python3 tools/capture_terrain.py --output work/slice24-captures
```

The normal **release game client** was also exercised through its real title,
creation and catalog UI: new seed 42 world, safe grounded spawn at(0.5,11,10.5),
F3, autosave/final save and normal Quit. The stored root is 64 bytes with
format v5 / generator v4. See [actual gameplay](../../images/slice24/client-new-world.png)
and [catalog](../../images/slice24/catalog-v5.png). The same world then reopened with a different seed-form prefill, retained its
seed 42 / gen 4 identity, saved with F5 and quit normally; the manifest hash stayed
identical. These two captures use normal
running world time and the same software adapter; the F3 FPS is not native GPU
performance. Automated tests separately verify edited save/reload and journal faults.

`--scene plain|hills|mountain|valley|transition|seams` selects one gallery capture. Output
files must be absent. No gallery command accesses a real save directory.

## Automated and native acceptance

Local final checks passed: `cargo fmt --all -- --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace` (**192 client  + 256 core passed**, four existing
optional client measurements ignored). `cargo build --release --locked -p rustcraft` also passed; the final PR records
Ubuntu/Windows CI separately. Texture regeneration
and Python tooling syntax checks also pass. Historical generator1/2/3 goldens remain unchanged;
manifest v4's header CRC and region golden CRC remain pinned. Generator v4 has four
independent full-column CRC fixtures over six signed/distant sites.

New checks exercise parallel/reverse chunk order, point/chunk consistency including
raised neighboring crowns, extreme signed queries, eviction/regeneration, complete
edited mesh/light halos at region seams, spawn/headroom, tree support/clearance,
cave ramp access, foundation connectivity, v5 byte/identity validation, future
rejection without replacement, catalog identity and interrupted paired recovery.
Existing water/bucket/torch/inventory/player tests run with the new default too.
Old v4 and new v5 edited lighting both reconstruct after save/restart.

The quality grid covers[-2048,2048)² every16 blocks for four seeds. Observed
surface0..24, at most a two-block adjacent step (only one in 524,288 sampled edges;
all others 0/1),2,348–2,849 sampled 32² patches with height range ≤2 per seed,
and largest connected elevated components 1,157–4,795 coarse cells (each 16² blocks).
Connectivity is a coarse geographical indicator, not a proof that an entire
component has one flat elevation. A separate actual voxel flood verifies carved
terrain remains connected to its solid bottom in the sampled cave region.

Void Linux **glibc** / Fish, fresh isolated data:

```fish
git fetch origin
git switch feat/slice-24-terrain-generation-v2
git pull --ff-only
set slice24_data (mktemp -d /tmp/rustcraft-slice24.XXXXXX)
env RUSTCRAFT_DATA_DIR="$slice24_data" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=3 cargo run --release --locked -p rustcraft
# Create a new named world; preserve this shell/path for reopening.
# After a normal close, load near the mountain example in the same saved world:
env RUSTCRAFT_DATA_DIR="$slice24_data" RUSTCRAFT_START_CHUNK="61,-90" RUSTCRAFT_LOAD_RADIUS=3 cargo run --release --locked -p rustcraft
```

- [ ] New catalog entry says Regional v5 / Gen 4. Create seeds 0/42/2026 separately;
  walk plains, ridges, valleys and transitions. Judge useful building area, tree
  density and the deliberately modest peaks without height clipping.
- [ ] Walk/jump/sprint on hills, visit cave entrances near (-10,20) and their 128-block
  repeats; inspect roots/crowns, Y 0/16 boundaries, and safe initial/R/void spawns.
- [ ] Cross signed chunk/region seams 0/±16/±64 and far columns; no holes, bright
  strips, phantom faces or unsafe pending loads. Change radius 1–3 and revisit
  evicted terrain; F3/cache/resource counts settle.
- [ ] Mine to Air, build across a region seam, place/remove torches and water with
  buckets, save F5/autosave/Save and Quit, reopen and revisit after eviction.
  Verify blocks, inventory, world time, light and water interactions persist.
- [ ] Use complete closed **copies** of existing v1/v2/v3/v4 worlds. Confirm their
  original generator, bounds and exact edits, without conversion/new scenery.
  Preserve original files; test failed-save/recovery only in disposable copies.
- [ ] Record release 1080p, VSync off, load radius, i3-13100, RX9060 XT/driver/backend,
  worker/queue/install and F3 frame percentiles while exploring multiple new
  regions. Compare a closed Slice 23/gen 3 copy on matched hardware/settings.
- [ ] Repeat on Void X11 and Wayland, then Windows 10/11. Check mouse capture/focus,
  pause/options/inventory, normal close and restart. Cloud software images and CI
  do not complete these native GUI/performance checks.

Biomes, cave overhaul, natural water, new vegetation/materials and vertical-world
expansion remain separate slices. Back up whole closed world directories,
including regions, player identity, journals and backups; the root alone is not
a complete regional-world backup.
