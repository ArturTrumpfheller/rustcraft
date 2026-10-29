# Slice25 — Biome Generation v1 validation

Base: merged Slice24 `628537303e77f9f3e2f202f9e90dc11416990cf3`.
Rust1.99.1 / Bevy0.20.0; no dependency, lockfile, graphics-feature or world-height
change. Own code/art/captures are MIT OR Apache-2.0.

## Reproducible performance comparison

The matched harness in `biome_bench.rs` runs against the **actual extracted
Slice24 core source**, generator4, and the Slice25 core, generator5. The same
coordinates, seeds, loop counts, allocator and CPU clock instrumentation are used
on both. Twenty-one sites cover Plains, Forest, Desert, Snowy Plains, Rocky
Highlands and two ecotones for each of seeds0/42/2026. Labels describe v5's biome;
the baseline at the same location is its original unmodified v4 landscape.

For each site:120 warmed raw three-chunk column samples;30 cold9-source-column
lighting/mesh jobs;30 isolated light/mesh samples on prebuilt voxel inputs;32
successive exploration jobs with sliding radius-two cache retention; and a
return to the original column after evicting it. Initial generator construction
and three-chunk spawn preparation are measured separately, without GUI/disk.
A separate actual catalog/create/load/spawn measurement below includes I/O;
neither measurement includes complete menu-to-play latency.

Raw generation and complete worker jobs report P50/P95/P99 elapsed microseconds.
On Linux x86-64, CLOCK_THREAD_CPUTIME_ID also reports actual job CPU time at all
three percentiles. Requested heap high-water is instrumented equally on both
versions; per-process peak RSS and aggregate user/system CPU are captured with
wait4. Mesh bytes include56-byte logical vertices and4-byte indices. Cache
occupancy/payload, separate light/mesh P50 and eviction/regeneration time are in
the raw CSV. Memory excludes allocator metadata from requested heap, and no
measurement includes GPU timing. Empty snapshot worlds used for worker sources
are deliberate: the supplied generator and immutable overlay are authoritative.

Reproduce on Linux with the prepared/native Rust toolchain:

```sh
python3 tools/benchmark_biomes.py --output work/slice25-comparison --samples 3
cargo run --release --locked -p voxel_core --example biome_survey
```

The script refuses existing output directories, uses offline dependencies for
its isolated baseline and alternates version order across three repetitions.
See [core results](core/) and [four-seed distribution survey](biome-sites.csv).

All summaries below are **medians of per-site percentiles**, across three seeds
and three repetitions (nine values per biome;63 across all sites). They are not
pooled global percentiles or frame-time estimates. Units are microseconds.

| v5 scene at matched coordinates | Raw v4 P50/P95/P99 | Raw v5 P50/P95/P99 | Worker v4 P50/P95/P99 | Worker v5 P50/P95/P99 | Worker P50 change |
| --- | --- | --- | --- | --- | --- |
| Plains | 323 / 433 / 467 | 311 / 400 / 433 | 3821 / 4162 / 4233 | 3934 / 4534 / 4709 | +2.9% |
| Forest | 350 / 426 / 454 | 429 / 496 / 553 | 3979 / 4333 / 4370 | 4741 / 5109 / 5308 | +19.2% |
| Desert | 289 / 346 / 378 | 308 / 372 / 438 | 3419 / 3890 / 3995 | 3319 / 3648 / 3928 | -2.9% |
| Snowy Plains | 316 / 376 / 407 | 320 / 396 / 433 | 3601 / 3917 / 3934 | 3480 / 3831 / 4070 | -3.4% |
| Rocky Highlands | 358 / 454 / 467 | 401 / 473 / 519 | 4550 / 4857 / 4882 | 5195 / 5707 / 5708 | +14.2% |
| Plains/Forest | 312 / 392 / 476 | 322 / 398 / 418 | 3479 / 3725 / 3765 | 3541 / 3823 / 3889 | +1.8% |
| Plains/Desert | 320 / 403 / 444 | 349 / 431 / 488 | 3437 / 3750 / 3750 | 3507 / 3807 / 3830 | +2.0% |
| All sites | 320 / 397 / 449 | 326 / 403 / 458 | 3627 / 3981 / 4122 | 3705 / 4067 / 4363 | +2.1% |

| Other measurement, all sites | Slice24 | Slice25 |
| --- | --- | --- |
| Worker thread CPU P50/P95/P99 (µs) | 3620 / 3934 / 4121 | 3704 / 4066 / 4328 |
| Exploration job P50/P95/P99 (µs) | 1615 / 2060 / 2161 | 1626 / 2239 / 2390 |
| Initial three-chunk spawn generation (µs) | 317 | 330 |
| Return after cache eviction (µs) | 3,807 | 3,942 |
| Prebuilt-source lighting P50 (µs) | 316 | 302 |
| Prebuilt-source meshing P50 (µs) | 125 | 117 |
| Worker mesh vertex/index payload (bytes) | 40,424 | 10,168 |
| Requested raw-generation peak heap (bytes) | 13,648 | 16,176 |
| Requested cold-job peak heap (bytes) | 447,256 | 445,336 |
| Requested exploration-job peak heap (bytes) | 447,256 | 445,336 |
| Maximum observed exploration cache columns | 15 | 15 |
| Observed cache voxel payload (bytes) | 184,320 | 184,320 |
| Retained generator-wide planning allocation (bytes) | 0 | 0 |

Process peak RSS was10,496KiB in every repetition for both versions; requested
heap figures provide the finer-grained allocation comparison. Total process
user+system CPU medians were5.163s (v4) and5.478s (v5) for the complete harness.
These RSS/CPU totals describe the benchmark process, not the Bevy client/GPU.
The persistent cache cap remains81 columns and the client radius caps remain
unchanged; this straight exploration trace actually retained at most15 columns.
The atlas adds16KiB of embedded pixels (32→48KiB), with the same shared material.

Forest raw generation rises22.7% and its cold worker P50 rises19.2%
(3.979→4.741ms); highland worker P50 rises14.2% (4.550→5.195ms). These are the
material regressions: v5 adds climate/dune/material sampling and more accepted
forest candidates/clearance checks. Repeated entrance/cave queries also consume
the v5 surface when those sites intersect the established ramps. Caching the
20² climate/height neighborhood and reusing18² materials bounds that cost. The
all-site worker P50 summary rises2.1%; exploration P95/P99 rise8.7/10.6%.

Lower mesh payloads are **scene geometry differences**, especially flatter,
sparsely wooded v5 plains and treeless sand. They are not a new mesh compression
algorithm or a universal74.8% memory-saving promise. Forest mesh payload can
increase at individual sites; inspect the CSV for seed/site variation. The
isolated lighting/meshing summaries depend on that same changed geometry.

The build, GUI captures and test processes completed before the alternating
timed benchmark repetitions. CPU measurements remain shared-cloud samples,
without physical GPU timings or a claim about the owner's native Radeon.


An additional matched pass measured **actual catalog creation + persisted player/world
initialization + opening + first three-chunk spawn preparation**, on the cloud
`/tmp` filesystem. `biome_creation_bench.rs` uses unique disposable directories,
checks the real created generator identity and never accesses user saves. Three
warmups precede30 creations per seed (0/42/2026), repeated three times per version.
The same script reproduces these separate `*.creation.csv` records.

| Catalog/create/load/spawn (µs), median per-seed percentiles | P50 | P95 | P99 |
| --- | --- | --- | --- |
| Slice24 | 514 | 617 | 630 |
| Slice25 | 527 | 640 | 707 |

This includes checksummed catalog/player/world file I/O, but excludes window,
GPU setup and entity publication. It is not a native SSD or full GUI load-time
claim. RSS/CPU figures above concern the separate generation/worker process.

## Actual RustCraft client captures

These are actual client-gallery renders of generator5 using the production
terrain shader, original embedded atlas, AO, skylight, fog and noon sky. They
are neither concept art nor synthetic biome maps. The fixed-camera gallery has
no hand/menu/debug overlay or saved edits. Five scenes use the ordinary1.62-block
eye; only the two boundary views use a12-block overview eye. No terrain, tree,
height, material or lighting override is applied.

Common settings:1280×720, vertical FOV78°, radius3, day0/time6000. Cloud renderer:
X11/Xvfb, llvmpipe (LLVM19.1.7), Mesa25.0.7, Vulkan. These screenshots and CPU
measurements **do not establish native RX9060 XT performance**, compositor/input
behavior or Windows GUI acceptance.

| Scene | Seed | Requested X/Z | Actual feet XYZ | Yaw | Eye |
| --- | --- | --- | --- | --- | --- |
| [Plains](../../images/slice25/plains.png) | 42 | −1568/−4096 | −1567.5,4,−4095.5 | 45° | +1.62 |
| [Forest](../../images/slice25/forest.png) | 42 | −4096/−4096 | −4095.5,4,−4095.5 | 90° | +1.62 |
| [Desert](../../images/slice25/desert.png) | 0 | 2304/−4032 | 2304.5,5,−4031.5 | 0° | +1.62 |
| [Snowy Plains](../../images/slice25/snowy-plains.png) | 2026 | −3808/−4096 | −3807.5,7,−4095.5 | 90° | +1.62 |
| [Rocky Highlands](../../images/slice25/rocky-highlands.png) | 42 | 3824/−4032 | 3824.5,14,−4031.5 | 0° | +1.62 |
| [Plains/Forest](../../images/slice25/plains-forest.png) | 42 | −2936/−3000 | −2935.5,7,−2999.5 | 90° | +12 |
| [Plains/Desert](../../images/slice25/plains-desert.png) | 2026 | 1288/−3000 | 1288.5,8,−2999.5 | 90° | +12 |

Yaw0 faces north/−Z,90 east/+X. Pitch points0.08 below the horizontal direction
in normal scenes and0.3 in overview scenes. Spawn uses the actual normal
`spawn_position`/body-clearance function; it can select a nearby safe column.
All resulting feet above are recorded by the real client, not inferred from a
height map. Forest density, sand, cold pale ground and stepped exposed ridges are
recognizable without biome labels; boundary scenes show continuous terrain and
coherent material/vegetation change.

```sh
cargo build --locked -p rustcraft --example terrain_gallery
python3 tools/capture_biomes.py --output work/slice25-gallery
# One exact normal-eye scene, no save access:
cargo run --locked -p rustcraft --example terrain_gallery -- 42 3824 -4032 0 --generator=5
```

Use a working native display and normal graphics drivers. Cloud captures needed
a local virtual display and explicit path to the prepared Mesa software ICD;
those are test-environment settings, not product renderer requirements.

## Automated validation

- Format, strict locked workspace/all-target Clippy, locked workspace tests,
  release client build and original-texture reproducibility are required.
- Historical generator1–4 goldens are unchanged. New v5 CRC goldens cover four
  seeds and six signed/distant columns; threaded/reverse sampling agrees.
- Broad climate distribution, connected region interiors, non-chunk boundaries,
  smooth bounded heights, discrete surfaces/deep sand, warm rock and cold ground,
  forest/plains/highland density, supported trees, cross-chunk crowns, canopy
  clearance, Y31 headroom, safe spawns and the original cave ramp are tested.
- Full-neighbor and streamed edited meshes/light agree on signed region seams.
  Existing streaming tests retain eviction, stale-result rejection and bounded
  CPU/GPU asset behavior. Movement code/configuration is unchanged; its existing
  fixed-step, frame-rate, crouch, sprint and water regressions still run.
- New block/item IDs, mining/placement/pickup, water displacement, torch support,
  solid lighting/AO/greedy behavior, creative palette clicks and held meshes are
  covered. All old bucket/torch tests continue to run.
- New blocks/items and explicit Air survive journal interruptions in generator3,
  4 and5 worlds. Historical v4/gen3 and v5/gen4 manifest CRCs remain pinned.
  Existing corruption/unknown-generator rejection and player-binding checks run.
- All eight old tiles and extruded pixels were compared byte-for-byte to Slice24;
  generated artifacts pass the reproducibility check. No new game dependencies.

Executed on the final implementation:

```text
cargo fmt --all -- --check                                   PASS
cargo clippy --locked --workspace --all-targets -- -D warnings PASS
cargo test --locked --workspace                              PASS
  client: 192 passed, 4 existing profiling tests ignored
  voxel_core: 268 passed, 0 ignored; doc-tests: 0
cargo build --release --locked -p rustcraft                   PASS
python3 tools/generate_textures.py --check                    PASS
```

The seven client captures, four-seed survey and three matched benchmark pairs
also completed. CI status is reported on the dedicated PR; native Void/Wayland,
RX9060 XT performance and Windows GUI validation remain owner acceptance.

## Native Void Linux glibc / Fish acceptance

Run the owner's tests before merging. Use normal native drivers and an isolated
absolute data root. Do not point fault-injection or older clients at the only copy
of a real save; historical tests use complete closed-directory copies.

```fish
git fetch origin
git switch feat/slice-25-biome-generation-v1
git pull --ff-only
cargo build --release --locked -p rustcraft
set biome_test_dir (mktemp -d /tmp/rustcraft-slice25.XXXXXX)
echo $biome_test_dir
# Create a new world with seed42 in the menu; the variable prefills the form.
env RUSTCRAFT_DATA_DIR="$biome_test_dir" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=3 RUSTCRAFT_START_CHUNK="-98,-256" cargo run --release --locked -p rustcraft
# Close normally; reopen the same seed42 world at the forest test column.
env RUSTCRAFT_DATA_DIR="$biome_test_dir" RUSTCRAFT_LOAD_RADIUS=3 RUSTCRAFT_START_CHUNK="-256,-256" cargo run --release --locked -p rustcraft
```

Seed42 rocky flank starts at chunk239,−252; Plains/Forest at−184,−188. Create
additional worlds in the same catalog with seed0 (desert chunk144,−252) and
seed2026 (snow chunk−238,−256; Plains/Desert chunk80,−188). The seed variable only
prefills creation; it never changes a loaded world's identity. Optional start
chunks select an area, not the exact camera/yaw in the gallery table. F3 shows
actual world coordinates and biome. R intentionally returns to origin; position
is not saved.

1. On **both X11 and Wayland**, verify new catalog worlds identify generator5.
   Compare all five regions in normal walking, not only F3. Walk each boundary:
   no chunk-shaped biome edges, height discontinuities or one-block stippling.
2. Explore forest clearings and seams, climb highland flanks and inspect cold
   ground/dunes. Trees must have roots, clear crowns, no obvious regular rows,
   and no ceiling clipping. R/void respawn must find safe clear supported ground.
   Check the existing origin cave ramp near X−10/Z20 and the tunnel near Z−10.
3. E → click Sand/Snowy Ground (Shift-click grants a stack). Transfer them to
   hotbar, select, inspect all held faces, mine, collect and place across signed
   chunk/region borders. Check snowy top/side/dirt underside and proper sand.
   New cubes must collide and target normally. Displace placed water with each;
   place standing/wall torches against them and mine supports. Verify buckets,
   torch drops, inventories and movement retain existing behavior.
4. F5, wait SAVED, exit normally and reopen the same world. Check exact blocks,
   saved Air, stacks/selection and time. Travel away to evict the edited chunks,
   return and compare; repeat rapid seam edits while the terrain worker runs.
5. Open **copies** of regional v4/gen3, regional v5/gen4 and older finite worlds.
   They must retain their generator/terrain/removals and inventory without new
   biome generation. Grant/use both new items from E, save/reopen and verify.
   Preserve the complete directory when backing up player/world/journal/regions.
6. Check F3 toggle, resize, focus, capture/release, pause/inventory, walking,
   jumping, crouch, sprint and respawn on native input. Record GPU/driver/backend,
   resolution, radius, build mode, frame P50/P95/P99, worker latency and memory
   during sustained exploration and eviction. Compare against Slice24 on the
   same machine/settings. Cloud timings are not an RX9060 XT estimate.
7. Repeat GUI and save/reload checks on **Windows10/11 MSVC** using its native
   supported renderer. Ubuntu/Windows CI cannot substitute for desktop tests.

## Remaining limitations

Y remains−16..31; the maximum surface is24, maximum crown30. Highlands are modest
uplands, not large mountains. A dedicated versioned vertical expansion must
precede major mountain/deep-cave work. Trees use the existing solid opaque crown
art/species. Sand has no falling simulation; snow is a full visual terrain cube,
without accumulation/layers, freezing, weather or ice physics. No new caves,
water geography, plants, species, structures, ores, entities or multiplayer.

The broad scalar threshold system can create small marginal contours; it is not
a Voronoi map or a formal minimum-region-size guarantee. The bounded draw radius
limits visible geographical breadth. Full GUI startup and native
physical-GPU endurance need owner acceptance in addition to the measured core
harness. The PR remains unmerged for that review.
