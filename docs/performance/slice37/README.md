# Slice 37 — Biome Generation v2 (generator12)

Baseline: exact merged Slice36 **`3d0f13c7e603ed060d53a722cf73c51619ade308`**.
V12 is the new-world default. Existing worlds keep their seed/version/nonce,
procedural blocks, biome classifications, vegetation, water and authoritative
player edits, including explicit Air. Bounds X/Z[-32768,32768), Y[-64,192),
manifest5, region1, journal/player/inventory envelopes and block/item meanings
remain. There is no migration or automatic PR merge.

## Research and visual references

Public references consulted on2026-11-12:

- [World generation](https://minecraft.wiki/w/World_generation),
  [Biome](https://minecraft.wiki/w/Biome),
  [Noise router](https://minecraft.wiki/w/Noise_router),
  [Density function](https://minecraft.wiki/w/Density_function),
  [Terrain features](https://minecraft.wiki/w/Terrain_features).
- [Multi-noise parameter interval API description](https://maven.fabricmc.net/docs/yarn-1.20.5-pre3%2Bbuild.2/net/minecraft/world/biome/source/util/MultiNoiseUtil.NoiseHypercube.html).
- [Mojang's 1.18 experimental generation notes](https://www.minecraft.net/en-us/article/new-world-generation-java-available-testing).
- [Mojang's modern Meadow article and photographs](https://www.minecraft.net/en-us/article/around-block--meadow).
- [Mojang's Frozen Peaks article and photographs](https://www.minecraft.net/en-us/article/around-block--frozen-peaks).
- [1.18 Java release and Grove photograph](https://www.minecraft.net/en-us/article/caves---cliffs--part-ii-out-today-java)
  ([external photograph](https://images.ctfassets.net/8y6ykjruobr4/5XemSITAY3RjNFkaLEeEwC/2f82bd8826ab26fe52d8e9a0fadee29a/1-18-grove.jpg)).

Direct Wiki requests were restricted; indexed public descriptions supplied its
multi-noise relationships. The architectural lesson is shared continuous
geography: T/H select climate families, C/E/W organize landforms, depth separates
surface from underground environments, and interval distance selects suitable
biomes. Mountain ecosystems depend on relief and regional climate; moisture
supports woodland and cold high terrain loses trees. Ocean/shore/river geography
must match actual terrain/water. Minecraft's precise tables, constants, code and
assets are not used. Depth is zero at RustCraft's classified surface; underground
biome selection remains deferred. The external images guide flower-rich open
meadows, dark conifer groves and treeless snow/rock above wooded slopes. They are
qualitative references without matched cameras, seeds or pixel equivalence.

## Original architecture and terrain guarantee

`worldgen/biome_v2.rs` owns original integer Q10 ecology. The v11 profile builder
exposes its already-computed C/E/W; the v12 planner reuses them, runs the unchanged
v11 water/bed evaluator, and classifies/resurfaces the final18² halo. V11's
independent reference fixtures remain unchanged. No biome selects terrain height,
modifies a density bit, carves a cave or places water. Full-height v11/v12 tests
compare every solid/air/water cell below the surface; only surface materials and
decoration may differ. Deep geology remains byte-identical.

Temperature uses1536/640-block value-noise wavelengths weighted5:1; humidity
uses1024/448 weighted4:1. A shared640-block64-block-amplitude warp and3:4 rotation
reduce axial grain. Euclidean noise cells and truncating signed integer arithmetic
are pinned independently in Python and Rust. No mutable RNG, biome cells or
world-sized map participates.

Original lowland rectangles minimize `2*temperature_distance² + humidity_distance²`.
Intervals are not Minecraft data. Desert requires T≥200; Snowy Plains requires
T≤-420, forcing intermediate climates. Suitable wet temperate intervals select
Forest/Birch Forest; cooler wet intervals select Taiga; drier intermediate land
selects Plains. C/E/W and height choose mountain families without altering geometry.

A regional snowline is `76 + T*90/1024 + maritime_moderation + E/160 - W/160`;
maritime moderation is0–5 blocks. The treeline is18 blocks lower. Mountain
shoulders begin near ecologicalY38; increasingly cold/moist elevation selects
Meadow/Grove, high treeless slopes and snow. Warm high summits retain Stony Peaks.
Dry highlands are rock rather than sandy peaks. These are original compressed
RustCraft altitude bands, not Minecraft's world-height coefficients.

The ecological elevation proxy bilinearly interpolates v11 offsets on an absolute
32-block lattice, then combines three parts proxy with one part actual final
height and caps it eight blocks above a lowered dry bed. This removes per-ledge
ecosystems from sparse density displacement. A per-plan vector holds at most
nine lattice samples; it is discarded with the sampler. Actual block geometry,
root/crown support and cliff slope still use the unmodified final surface.

## Stable biome roster and materials

Biomes are derived diagnostics, not region records. The enum has explicit u8
representation; existing0–4 retain meanings and new identities append5–14.

| ID | Biome | Surface/ecosystem |
| ---: | --- | --- |
|0|Plains|Open grassland, moisture-dependent scattered Oak/Birch, grass/flowers|
|1|Forest|Wet temperate woodland, mostly Oak with Birch, varied clearings/undergrowth|
|2|Desert|Warm dry lowland Sand; sparse Dead Bushes|
|3|Snowy Plains|Regionally cold lowland Snowy Ground, rare Spruce/ferns|
|4|Rocky Highlands|Dry/exposed upper shoulders, Stone or cold Snowy Ground|
|5|Meadow|Elevated grass, strong flower patches, sparse Birch/Oak|
|6|Birch Forest|Temperate moist woodland,92% Birch candidate species|
|7|Taiga|Cool moisture-supported Spruce woodland, fern-rich ground|
|8|Grove|Cold wooded mountain slopes, Spruce and coherent upper snow|
|9|Snowy Slopes|Treeless snowy upper mountains, exposed steep Stone|
|10|Stony Peaks|Warm/dry exposed summits, no trees|
|11|Beach|Low gentle coastal Sand, independently proven nearby marine water|
|12|Ocean|Actual existing marine basin; sandy/rocky bed|
|13|River|Actual existing river water/bed; no unrelated strip noise|
|14|Lake|Existing lake/pond water and bed|

Slope≥3 exposes Stone; suitably steep upper slopes also expose rock at slope2.
Gentle cold ground uses the existing Snowy Ground block, with Dirt below. Warm
alpine peaks remain Stone; Sand never follows Desert up mountains. Water labels
require positive water depth and the existing actual `WaterBody`. Beach candidates
are dry coastalY3–5, slope≤1, with real marine water found on cardinal8-block or
diagonal6-block probes. The finite probe stencil is conservative: it can miss
small enclosed inlets between probes. It cannot invent an inland sea or sand a
mountain. Historical coastal material overrides are disabled only for v12.

## Vegetation, support and lifetime

All original assets/crown shapes/heights remain. Full-cell-jittered8-block tree
candidates use independent density/species decisions. Compatible lowland forest
families share a continuous humidity curve; an80-block clearing field leaves
natural openings. Meadow stays sparse; Grove uses Spruce; the8-block treeline
fade removes upper trees. Grass/ferns, red/yellow flower patches and sparse dry
bushes reuse existing assets and independent24/40/64-block patch decisions.

Roots require actual dry Grass/Snowy Ground, no cave opening, a clear5×5
footprint, ceiling/edge margin and the existing spawn reserve. V12 alone permits
higher footprint ground at most two blocks above the root: every species' lowest crown is at least three
blocks above root ground. V9–v11 keep the old one-block rule. Tests reject buried
canopies, unsupported roots/plants, water conflicts and out-of-bounds generation.
Player-planted vegetation, support edits, displacement/drop behavior and saved
Air retain the established transactional paths. Root plans borrow known final
water/ecology data and use existing bounded local caches for the remaining halo.
There is still one async worker and no retained global ecology cache.

F3 uses `Generator::biome` for v12 and the frozen classifier for historical
worlds. It caches a single saved-identity/absolute-position sample; no climate is
resampled while that position is unchanged. F3 regression tests cover switching
identities at the same coordinates.

## Reproduction

```sh
cargo run --release --locked -p voxel_core --example biome_quality -- --step=16 > work/quality12.csv
python tools/analyze_biome_v2.py work/quality12.csv --output work/biome-maps
cargo run --release --locked -p voxel_core --example biome_ecosystems > work/ecosystems.csv
python tools/biome_v2_reference.py --check
python tools/check_biome_v2.py work/quality12.csv work/ecosystems.csv work/ecology-consistency.json
cargo build --release --locked -p rustcraft --example terrain_gallery
python tools/capture_biome_v2.py --output work/biome-captures --matched
python tools/benchmark_biome_v2.py --output work/biome-core --samples 128 --warmups 8
python tools/benchmark_streaming.py --output work/biome-stream --baseline 3d0f13c7e603ed060d53a722cf73c51619ade308 --labels slice36 slice37 --biome-v2 --distance-expansion --rendering-install --repeats 1 --frames 64
```

Use a working desktop for captures; cloud evidence uses Xvfb/Mesa llvmpipe Vulkan.
Gallery-only radius10/wide fog panoramas are explicitly labeled; game radius1–4
and ordinary fog remain unchanged. The game requires none of Python/Matplotlib,
NumPy/Pillow/Xvfb. Captures never read user saves. Performance runners export and
instrument disposable copies under `work/`, build both executables first, and
retain raw observations/fingerprints. Repeat with reversed order and no concurrent
builds, tests or renderers. Cloud CPU/renderer costs are not native Radeon FPS.

## Survey, visual and performance evidence

- [Actual Bevy gallery, matched v11/v12 panoramas and real client/F3 captures](comparison.md).
- [Four-seed maps, region sizes, fragmentation, altitude and ecosystem counts](quality.md).
- [Exact baseline CPU, streaming, allocation, payload and RSS evidence](performance.md).
- [Validation commands, deterministic fixtures and platform status](validation.md).

The first coast/Grove performance pass exposed redundant work. V12 now proves
marine proximity with the exact v11 unmodified-height predicate: hydrology marks
Ocean before basin/channel lowering and preserves that identity. It therefore
avoids rebuilding lake/river geography for each probe; the protected starting
shore retains its exact final-depth fallback. A classification-local cache reuses
the existing20² raw density tops and memoizes at most756 exterior probe positions;
it is discarded after the18² tile and creates no persistent biome map. Tree footprint checks need
dry support and crown clearance, so they skip ecological classification of
neighbor materials. Historical admissions/materials remain untouched. Regression
and before/after column CRC checks verify these optimizations preserve v12 output.

The coastal cold-column P50 remains1.421ms versus v11's0.693ms after optimization
(from3.281ms initially). Radius4 Forest has more foliage: resident mesh payload
11.56→16.88MB and request-ready P95 258→475ms on this cloud worker. These costs,
all tails and per-scene memory are retained in performance.md. Native FPS remains
unmeasured; no universal performance-equivalence claim is made.

## Known limits and next slices

- Cold surfaces reuse full Snowy Ground blocks; there are no powder snow, ice,
  snow layers, frozen rivers, seasonal changes or new assets in this slice.
- The biome roster targets distinctive ecosystems, not every Minecraft biome.
  Depth/cave ecology remains a later generator; cave v3 is Slice38.
- Exact v11 water geometry is deliberately retained. Its water admission still
  depends on old climate, rivers are rare finite lowland connections, lakes can
  have rounded outlines and every surface source sharesY2. Watershed routing,
  river levels, waterfalls, broader rivers and climate-aware hydrology are Slice39.
- Fine geometric steps and narrow altitude bands can still create small edge
  components. Survey component statistics are center-sample estimates at16 blocks,
  not exhaustive block polygons or a claim that each sampled chunk is uniform.
- Native Gentoo/Void X11/Wayland, Windows GUI and the owner's Radeon hardware
  require their own acceptance. The user-reported v11 ~400FPS/330MB/27%CPU at
  radius4 is context, not a measured v12 threshold or a cloud benchmark baseline.

## Native Gentoo/Void Linux and Windows acceptance

Use closed copies of existing v1–v11 worlds and keep originals untouched. Build
this PR and exact Slice36 in separate directories with the same Rust1.99.1,
Bevy0.20, display settings, FOV78, VSync off, resolution and driver. Confirm old
world generator/biomes/blocks, saved Air, inventory, water/buckets and support
edits survive load, eviction, regeneration and restart. New worlds must show
GENERATOR12 in F3; unsupported future manifests must reject loading.

On Gentoo or Void glibc, install the platform packages from PLATFORMS.md and
`cargo run --release --locked -p rustcraft`. Fish uses `env RUSTCRAFT_WORLD_SEED=0
RUSTCRAFT_START_CHUNK=-120,-252 RUSTCRAFT_LOAD_RADIUS=4 cargo run --release --locked
-p rustcraft`; create a new world or load a closed test copy with that seed. Test
both X11 and Wayland, cursor capture/unlock, focus loss, inventory, pause/options,
settings rollback and switching A→B→A repeatedly. Inspect the gallery sites in
`tools/capture_biome_v2.py`, including the complete cold mountain and warm peak
transitions. Walk across signed chunk/region boundaries and edit water/vegetation.

On Windows10/11 x64, use the MSVC prerequisites in PLATFORMS.md. PowerShell sets
`$env:RUSTCRAFT_WORLD_SEED='0'; $env:RUSTCRAFT_START_CHUNK='-120,-252';
$env:RUSTCRAFT_LOAD_RADIUS='4'`, then runs the same locked release command. Verify
D3D12/Vulkan as available, alt-tab/capture, DPI/window modes, saves and independent
sessions. CI deterministic fixtures must match Linux before visual acceptance.

For each platform compare exact baseline/current at radii2,3,4 in mixed/Birch
forest, Taiga, Meadow/Grove transition, snow, coast and desert. After convergence,
record frame/update and streaming P50/P95/P99/max, process memory/retained caches,
resident mesh payloads and adverse observations using the same capture duration.
Inspect complete transitions from several directions; do not choose acceptance
from a single attractive seed. Do not merge until the owner has reviewed results.
