# Slice 39 — Cave Generation v3 (generator14)

Base: exact merged Slice38 `c6d36393c037e4295bf93c6a4efbc9fa12dd053c`.

## Investigation, before implementation

`caves.rs` owns the frozen v7 geometry inherited by v8–v13: 96-block
ownership cells, lobed ellipsoids, seven-leg swept branches, roots near -29
and a second chamber near -49. Branch elevations clamp to [-54,-8]. Only
entrance routes rise above this band. Their target is `biome::height` (v5),
so the newer v13 mountains can bury the entire route. Raising the chamber
radii or spawn probability cannot solve that geography or repeated room shape.

`local.rs` shares one 18² halo-column plan across all sixteen vertical chunks.
It builds v13 density profiles, hydrology, materials, caves and vegetation.
It carves after sparse surface occupancy, before underground pool fill and tree
application. Point queries also use this plan for v8+, so both integrations
must select the same versioned cavity predicate. Trees perform their own
terrain-root admission: that path must use v14 caves too.

`hydrology.rs` preserves a four-block sealed bed/bank cap. Pools are bounded
flood fills admitted against v7 geometry; applying those sources to a new
connected cavity would invalidate closure. V14 must skip their planning and
filling entirely. V1–v13 keep all pool admission/cache arithmetic unchanged.
The v13 geography dispatcher currently recognizes exactly version13; v14 must
explicitly reuse that mapping, including ecology/coast/water admission queries.

## Public reference study

Consulted 2026-11-13: [official Java1.18 release notes](https://www.minecraft.net/en-us/article/caves---cliffs--part-ii-out-today-java),
[official Java1.18 support changelog](https://feedback.minecraft.net/hc/en-us/articles/4415128577293-Minecraft-Java-Edition-1-18),
[official generation preview](https://www.minecraft.net/en-us/article/new-world-generation-java-available-testing).
These describe roomy cheese caves, winding spaghetti, narrower noodles and
traditional carvers combined into networks. The inspiration is combining
scales, residual rock and connections, not matching undisclosed distributions.
The requested Wiki Cave/World_generation/Density_function/Java_Edition_1.18
pages were attempted; direct access was blocked by robots.txt. Indexed excerpts
confirm density composition/interpolation but are not a quantitative oracle.
No proprietary code, tables, assets or reference-game output is imported.

## V14 architecture and integration contract

`caves_v3.rs` owns v14. `Versioned` selects frozen `caves.rs` for v7–v13.
The new model combines an independently designed implicit 3D cavern field with
bounded anisotropic swept carvers on a feature graph:

- 128-block ownership cells contain jittered anchors. A smooth 512-block regional
  field modulates admission continuously. Empty ownership cells do not clip
  neighboring features or impose voxel-grid walls.
- Deep anchors vary through Y[-43,-24]. Separate upper anchors use the actual
  v13 hydrological terrain bed minus22–39 blocks, bounded to[-16,136]. Upper
  systems are present only when there is enough vertical separation. This is
  not a vertical stretch of the old model.
- Two signed Q10 3D fields, wavelengths32×20×28 and14×12×16, sculpt chamber
  walls, floors and ceilings, with positional warp, residual tapered pillars and
  partial wall shelves. A globally aligned4³ interpolation lattice is shared
  across all intersecting rooms in a column. Only room bounds allocate samples;
  tube-only and empty plans allocate no lattice. Density classification stays
  integer-only. Conservative envelopes include the maximum deformation.
- Cubic curves sampled into24 overlapping anisotropic chords produce long,
  smoothly changing passages. Widths vary along routes. Smaller secondary
  routes have greater than two-block minimum nominal diameter and extra height.
  Independent floor/ceiling voxelization can still introduce tight ledges.
- An owner alone emits +X/+Z connections to active neighbors. Major deep links,
  occasional upper links, returning branches and optional steep connectors make
  local loops and multi-level networks without a globally connected cave map.
  Major routes compose as Air after chamber residual rock, so a pillar cannot
  close a planned passage inside a chamber. Final terrain caps can truncate
  routes near low ground/water; measurements report the resulting components.
- Dry surface probes choose actual occupied v13 profile tops and final water
  beds, with bounded four-direction slope candidates. Mouths need bed>sea+6,
  dry ownership and distance from spawn. Only entrance routes bypass the normal
  eight-block roof guard. The ordinary spawn rectangle retains20 blocks of rock.
- Discovery uses two owner rings, independent of chunk traversal. Unit tests
  check every emitted feature against its owner envelope. Q4 world-coordinate
  curves use i64 cubic intermediates, preventing distant signed-coordinate
  overflow. No neighbors, resident-state checks, random stream or world map
  enter generation. Plans die with the existing column sampler.

The v13 terrain dispatcher explicitly accepts13 and14; all climate, density,
coast and water admission queries share this mapping. Base height/profile/biome
samples are exactly identical. Bounds, biome/block/item IDs, worker topology,
radii1–4, save identities/envelopes, journal behavior and authoritative edits
remain unchanged. Unknown generator15+ fails closed. New worlds alone use14.

### Water and future Slice40

Surface basins and source semantics remain v13. Their existing four-block
bed/bank seal is applied before any new cavity. Historical underground pools
are **not admitted, allocated or filled in v14**: their closure proof belongs to
the v7 cavity field. V1–v13 still evaluate their exact historical pool output.
There are no new procedural underground sources, aquifers or flooded systems.
Ordinary player-placed Water Sources and fluid edits continue to work.

For Slice40: cavity geometry is a dry predicate independent of water. A future
local water owner must prove final-cavity closure against the composed density,
carvers, foundation and protected surface caps; it must supply the same block
result to point, column, chunk and halo queries. Do not reuse `pool_center`
metadata as a v14 ownership proof. Any procedural change needs a new generator
version; preserve14's dry cavities and saved explicit Air.

### Determinism and regression evidence

`history-v1-v13.csv` was captured from exact merged Slice38 **before** modifying
production generation. It hashes full-height column blocks as little-endian u16
IDs plus biome identity, across four seeds and signed/distant/edge sites.
Existing independent v1–v13 fixtures remain untouched. The independent Python
`cave_v3_reference.py` provides signed3D noise and interpolated-lattice fixtures,
checked on both CI platforms. The new suite covers full-height sampler/chunk/
point/halo parity, foundations, water stability, ecology support, actual cave
physics/mining/placing/torch light, spawn, saved Air, journal recovery,
eviction, cancellation and repeated radius4 world switching.

## Reproducible quality analysis

```sh
cargo run --release --locked -p voxel_core --example cave_quality > quality.csv
cargo run --release --locked -p voxel_core --example cave_sites > sites.csv
python tools/cave_v3_reference.py --check
```

The fixed survey uses seeds0/42/2026/u64::MAX and central, negative, distant,
and mountainous128×128×256 windows. Mountains are found reproducibly by the
highest actual v13 profile on an8192² grid at spacing128; the output records the
exact window. Windows clip components: component extents are lower bounds,
not full-world network lengths. Absolute air bins start Y=-64, stride32, with
the last bin combiningY≥96. Surface-relative bins are depth0–15,16–31,…,80+.
Width/headroom bins are≤2,3–4,5–8,9–16,17–32,33+ blocks. Width is the smaller
of the two cardinal horizontal open runs at foot level, sampled on supported
floor cells, not a fitted tunnel radius or an unbiased chamber-size estimate.

Six-face components measure actual underground Air, not a heightmap.
A separate conservative floor graph requires solid support, two clear Air
voxels, level movement or a one-block step with swept headroom. A0.6×1.8 player
fits centered in those cells; diagonal shortcuts, climbing, long jumps and falls
are not counted. This remains a static navigation approximation; real controller
and transactional mining/placing are tested separately. A component's horizontal
span is reported, not an invented shortest-path length. Sparse surface-density
recesses remain included in both versions and explain some tiny high components.
No reliable quantitative Minecraft distribution was available; targets and
reference comparison are qualitative.

## Genuine rendered inspection and reproduction

```sh
cargo build --release --locked -p rustcraft --example terrain_gallery
python tools/capture_cave_v3.py --output /absolute/new/capture-directory
# Same production materials, only supported ordinary floor torches:
python tools/capture_cave_v3.py --output /absolute/new/torch-directory \
  --scene large-cavern --torch-only
```

All cameras/seeds/directions are in `tools/capture_cave_v3.py` and each capture's
JSON. Geometry and texture/shader assets are unmodified. `--cave-torches` installs
supported ordinary torches on a six-block development grid. The default capture
also uses **development-only diagnostic illumination**: mesh block-light is
floored to level10 while preserving face orientation/AO through the existing
production shader. It is not normal gameplay lighting and does not exist in the
playable client's rendering path. Torch-only captures omit that floor. Gallery
radius4 is within supported gameplay radii; the example loads a source halo and
never accesses saves. Linux cloud Mesa llvmpipe/Xvfb images establish actual
rendered geometry, not native Radeon FPS, VRAM or Windows GUI behavior.

## Performance reproduction

```sh
# Build first, then measure on an otherwise idle machine:
python tools/benchmark_cave_v3.py --output work/cave-cost --build-only
python tools/benchmark_cave_v3.py --output work/cave-cost --reuse --samples 128 --warmups 8
# Copy the first raw results before repeating in reverse order:
python tools/benchmark_cave_v3.py --output work/cave-cost --reuse --reverse --samples 128 --warmups 8
python tools/benchmark_streaming.py --baseline c6d36393c037e4295bf93c6a4efbc9fa12dd053c \
  --labels slice38 slice39 --caves-v3 --distance-expansion --rendering-install \
  --output work/cave-stream --build-only
python tools/benchmark_streaming.py --baseline c6d36393c037e4295bf93c6a4efbc9fa12dd053c \
  --labels slice38 slice39 --caves-v3 --distance-expansion --rendering-install \
  --output work/cave-stream --reuse --repeats 3 --frames 120
```

Both tools export the exact merged baseline and build instrumented disposable
copies, preserving production sources. CPU/wall, raw planning and vertical chunk
latency, cave planning/carving, light/mesh work, vertex/index payload, requested
heap allocations, compact chunks, caches, install and lifecycle costs retain raw
samples and P50/P95/P99/max. Clock instrumentation is included. Thread CPU uses
Linux CLOCK_THREAD_CPUTIME_ID; other platforms report unavailable instead of
inventing CPU time. Requested heap excludes allocator metadata, stacks and GPU.
Streaming is real headless worker/Bevy install behavior with scripted deadlines,
not presented frame timing. Existing ecosystem labels are retained in that
harness: forest=cavern, birch=intersection, taiga=narrow, meadow=deep,
grove=entrance; mountain is the upper-cave site. The raw logs retain exact states.

## Native acceptance: Gentoo/Void Linux and Windows

Use the pinned Rust1.99.1, existing native build dependencies and platform-selected
wgpu backend. Gentoo/Void X11 and Wayland need separate acceptance; Windows10/11
uses the native MSVC toolchain. No dependency, asset or graphics-feature changes
are required. The owner's approximately400FPS/330MB/radius4 report is context,
not an instrumented baseline from this cloud machine.

Fish on Gentoo/Void (after installing the packages in `docs/PLATFORMS.md`):

```fish
git fetch origin
git switch feat/slice-39-cave-generation-v3
git pull --ff-only
cargo build --release --locked --workspace
set test_dir (mktemp -d /tmp/rustcraft-slice39.XXXXXX)
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=4 cargo run --release --locked -p rustcraft
# After normal close, verify that the same world and edits reload:
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_LOAD_RADIUS=4 cargo run --release --locked -p rustcraft
# Interior gallery; development torches are explicit, no cave geometry edits:
cargo run --release --locked -p rustcraft --example terrain_gallery -- 42 42 70 0 --generator=14 --eye=42.5,-25.38,70.5 --direction=0.85,0.1,0.5 --cave-torches
```

PowerShell on Windows:

```powershell
cargo build --release --locked --workspace
$env:RUSTCRAFT_DATA_DIR = Join-Path $env:TEMP ("rustcraft-slice39-" + [guid]::NewGuid())
$env:RUSTCRAFT_WORLD_SEED = "42"
$env:RUSTCRAFT_LOAD_RADIUS = "4"
cargo run --release --locked -p rustcraft
# After normal close, run again with the same directory.
```

1. Create v14 worlds with all four survey seeds. Follow dry hill entrances into
   winding tunnels; place ordinary torches and explore lower and upper systems.
2. Check the recorded mountain and deep sites using the no-save gallery and
   compare with ordinary gameplay lighting. Record backend/driver, camera/seed,
   frame percentiles, CPU/RAM and queue/install metrics at radii2/3/4.
3. Walk, sprint, crouch and jump through narrow links/ledges; test real collision,
   drops, targeting, mining/placing, support removal and torch light at seams.
4. Place/collect water, open and seal a local basin, watch recession and reload.
   Existing natural surface basins must remain quiet and sealed.
5. Edit to explicit Air and build in caves; save, travel to evict, return, close
   and reload. Switch repeatedly between worlds and close with work in flight.
6. Open **complete copies** of older v1–v13 saves, inspect their original caves,
   pools and constructions. Verify identity and unchanged geometry.

Native physical-GPU/VRAM/FPS, Wayland input and Windows GUI remain owner checks.

## Known limits

This is an original Minecraft-inspired cave model, not distributional or visual
parity with Java1.18+. Some networks remain isolated; solid regions are deliberate.
Steep connections can require jumping, building or a drop. The standing graph
underestimates routes requiring longer jumps and excludes swimming. Density can
leave short voxel pinches and uneven room floors; no automatic path smoothing
changes the authoritative blocks. There are no aquifers, new ores, underground
biomes, structures or new art. Surface hydrology still uses its established fixed
datum and bounded sealing; its redesign belongs to Slice40. Large-window metrics
are analysis scratch only, never allocated during normal column generation.
