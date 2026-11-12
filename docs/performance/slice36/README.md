# Slice 36 — Minecraft-style terrain v4 (generator 11)

Baseline: **exact Slice35 `f527b75c21e8c40db4320a40b7630975fe1accb1`**, PR #53.
This slice is stacked on that complete implementation. Old worlds keep their
recorded generator, seed, bounds, nonce, inventory and authoritative edits.
Only newly created worlds select generator11. There is no migration or merging.

**[Actual v10/v11 comparison gallery and public Minecraft visual reference](comparison.md)**.

## Investigation and public references

V10 uses smoothstep-interpolated **value** noise: random scalar lattice values
produce broad lobes with similar rounded shoulders. Taking absolute values of
its ridge/valley fields folds both signs onto the same zero contours. Multiplying
rounded uplift masks by those folded fields repeatedly surrounds low regions
with raised bands. The valley width, ridge shape and regional relief have little
independent control over local slope character. More octaves or warp amplitude
would leave this construction intact. V10's height range alone was not the issue.

Research consulted on 2026-11-12, before choosing the replacement architecture:

- [World generation](https://minecraft.wiki/w/World_generation),
  [density functions](https://minecraft.wiki/w/Density_function),
  [noise router](https://minecraft.wiki/w/Noise_router),
  [terrain features](https://minecraft.wiki/w/Terrain_features).
- [Custom world generation: terrain density relationships](https://minecraft.wiki/w/Tutorial:Custom_world_generation).
- [Mojang's 1.18 Java release description](https://www.minecraft.net/en-us/article/caves---cliffs--part-ii-out-today-java).
- [Mojang developer Q&A and terrain photographs](https://www.minecraft.net/en-us/article/caves---cliffs-update--part-ii-dev-q-a).
- [Henrik Kniberg's terrain-generation presentation](https://www.youtube.com/watch?v=CSa5O6knuwI).

Some direct Wiki requests were robots/restricted; indexed public descriptions
provided the density/spline relationships. The presentation's public page was
reachable but its video/transcript was not available for frame measurements.
No Minecraft executable, matched seed captures or reference measurements were
available in this environment. Public illustrations are qualitative references;
**no pixel-level equivalence, numerical Minecraft comparison or exact parity is
claimed**. No proprietary source, noise tables, datapack constants, libraries,
textures or reference screenshots are copied into this repository.

The relevant architectural lesson is shared geography, not particular constants:
continentalness shapes ocean/inland offsets; erosion changes relief and slope
character; weirdness/peaks-and-valleys organizes highs and lows. Splines combine
these continuously into offset, stretching and jaggedness. A 3D density function
then gives solid/air geometry; surfaces and biome geography use related inputs.
Discrete biome IDs need not select unrelated height generators. Minecraft also
uses folded weirdness; the defect is not that *all* absolute-value functions are
invalid, but v10's particular folded value fields and nearly uniform masks.

## Independent v11 model

`worldgen/terrain_v4.rs` is separate from every frozen historical terrain module.
All generation decisions are integer Q12; signed cell division is Euclidean.

1. Coordinate-hashed gradient vectors, quintic lattice interpolation and seeded
   subcell phases replace v10's scalar value-noise fields. Three octaves each
   form continentalness, erosion and signed weirdness. Rotated coordinates and
   unequal octave wavelengths reduce aligned cell artifacts. No domain warp,
   radial mountain masks or absolute-value ridge/valley contours are used.
2. Shape-preserving cubic Hermite splines use shared harmonic tangents. Continental
   offset, erosion-controlled relief and a signed valley-to-peak profile combine
   before material assignment. Knots have nonzero shared tangents rather than
   flattening every segment boundary into a shelf. Coasts suppress inland relief.
3. Elongated weirdness wavelengths112×448 /48×144 /24×56 and a second,
   perpendicular112×448 family organize narrow linked ranges and saddles. An
   independent896-block structural-grain field blends the two continuously,
   avoiding a world-wide preferred ridge direction. Erosion cuts and varies them; the signed fields produce
   asymmetric flanks instead of a symmetric distance-to-zero ridge. Quiet plains
   reduce72-block rolling detail to one-third of a block, while hills retain more
   relief. Jagged detail is admitted only in appropriate high/low-erosion terrain.
4. Surface density is `offset - y*4096 + amplitude*noise/4096`. Amplitude smoothly
   reaches at most12 blocks in rugged regions and vanishes in quiet lowlands.
   Two independently hashed 3D gradient fields use32×28×32 and12×8×12 scales.
   They interpolate on an absolute4³ lattice shared by adjacent chunk plans.
   Noise is bounded to[-4096,4096] before applying the amplitude.
5. Each horizontal cell keeps a compact occupancy mask, a guaranteed-solid floor
   and its highest solid block. At most26 density cells are evaluated per column
   position, never the full256-block height. A bounded dense memo shares lattice
   vertices across the20² planning grid; it replaces an initial allocating map.
   The final18² profiles use5184 bytes (16 bytes each); temporary density memo is
   under12KiB at the theoretical altitude span. The400 raw rim heights (1600 bytes)
   feed water planning directly, and vegetation borrows the final18² surface grid.
   This avoids recomputing hundreds of 3D profiles for water halos and root tests.
   No persistent density cache grows.
6. Smooth asymptotic summit compression aboveY130 avoids a hard clipping plane.
   Offset stays below162; adding the12-block density bound leaves at least18
   blocks below the exclusive192 ceiling. The lower bound preserves foundation
   layers. Existing tree admission also enforces its full crown ceiling margin.

An early rendered candidate still had a broad dome and a circumferential cliff.
It was rejected. Narrower multiscale ranges, a steeper non-saturating peak spline
and smaller/finer 3D displacement replaced it before the final comparison.
A second broad survey exposed a preferred diagonal grain; continuously blended
perpendicular range fields removed that global directional pattern. The final
terrain has sharper alternating ridges, saddles and broken slopes;
it is not simply a taller v10 or a heightmap with cosmetic stone patches.

## Integration and compatibility

The same profile top serves point, column, water, vegetation and height queries.
`Generator::height` retains its established meaning: surface before cave openings
and decoration, after surface hydrology. Profile occupancy additionally decides
solid/air below that top. Stone-slab shortcuts use the guaranteed density floor,
not only `height-4`, so they cannot accidentally fill surface recesses.

Existing v8 ponds/rivers and closed cave pools remain, with v10's candidate-wide
admission/bank precision. V11 screens complete lowland candidates against its
conservative offset+amplitude envelope. Sea level is still source-cellY2.
Any hydrology projection away from the original density top establishes a solid
new bed, including dry banks. A fixed independently checked recess at
seed0:(-1440,56,-4000) tests this contract; a projected height must never name Air.
Wet beds remain solid before cave composition; the existing four-block wet-bank
cap also overrides density recesses, avoiding new shoreline leaks. This is
compatibility integration, not watershed hydrology. Small inherited pond outlines
remain elliptical; they do not construct v11's mountain or valley geography.

V7 cave primitives, foundations and pool discovery are unchanged. Elevated rock
can bury old cave entrances. Ground materials use final height/slope and existing
climate: steep rock, altitude-cooled snow, grass/dirt and coastal/desert sand.
The five biome IDs/climate map stay fixed. They do **not** yet fully track the new
continental/erosion geography; a biome overhaul remains deferred. Supported v9
Oak/Birch/Spruce and plants use actual final roots and crown footprints. All three
mesh surfaces, lighting/AO, single-worker streaming, cancellation, cache budgets,
render radii1–4, floating origin and save envelopes remain intact.

Y[-64,192) gives190 blocks above the sea datum, enough for this slice's ordinary
ranges with building headroom. It gives much less ocean-to-foundation depth than
modern Minecraft. A future sea-level/height revision must be a **new generator
and explicit world identity/bounds contract**, with save-budget, fluid, lighting
and streaming measurements. Never reinterpret existing manifests or lift the
current sea level under old worlds.

## Reproducing geography and visual comparisons

```sh
cargo run --release --locked -p voxel_core --example terrain_quality -- --generator=11 --step=16 > work/quality11.csv
cargo run --release --locked -p voxel_core --example terrain_quality -- --generator=10 --step=16 > work/quality10.csv
cargo run --release --locked -p voxel_core --example terrain_quality -- --generator=11 --step=64 --final > work/quality11-final.csv
python3 tools/analyze_terrain_v4.py work/quality11.csv --version 11 --output work/diagnostics
python3 tools/analyze_terrain_v4.py work/quality10.csv --version 10 --output work/diagnostics
python3 tools/terrain_v4_reference.py --check
cargo build --locked -p rustcraft --example terrain_gallery
python3 tools/capture_terrain_v4.py --output work/captures36
```

Use a working native desktop/adapter for captures. Cloud images use Xvfb and Mesa
llvmpipe Vulkan; these are real Bevy renders of production meshes/materials/light,
not generated illustrations. Supported-radius gallery views reproduce ground-eye
poses and normal fog; they are read-only fixtures, not interactive play recordings.
Development panoramas explicitly use gallery-only radius10 and fog160–320. No game
render-distance setting or limit changes. FOV78°,1280×720, day6000. Exact commands,
seed/coordinates, absolute eye/direction and image SHA256 are in the capture manifest.

The same coordinates are rendered for each version. Ground camera yaw is
selected reproducibly toward an open view using the independent height reference,
so close stone walls do not obscure the landscape; yaw can differ by version. Their geography,
height, water and resulting ground-eyeY can differ substantially: a v11 plain
can be submerged in v10. Do not misread such a pair as a matched biome or camera
height experiment. Panoramas use the same explicit absolute pose for both.
Compare the terrain categories as well as these coordinate-matched examples.
Public Minecraft reference images remain external and are not redistributed.

## Survey definitions and limits

Each version samples seeds0/42/2026/u64::MAX over8192² centered on zero and2048²
windows at(-24000,20000) and(28000,-30000):1,179,648 raw16-grid observations.
Final hydrology surveys use64 spacing and73,728 observations. Every point also
samples actual cardinal one-block slopes and16-block neighbor heights.

Elevation/slope quantiles, height differences at16/64/256/1024 blocks and X-axis
height autocorrelation are descriptive measurements. Flat32 means nine samples
on a32² square span at most one block, conditional on land atY≥4. It is stricter
than normal buildability and is not a cave-foundation proof. Mountain masks use
Y≥70; four-connected components provide sampled widths/depths/isolated components.
Zhang-Suen mask skeletons provide ridge-length, branch-pixel and orientation
proxies, not surveyed geological crest lines. Their resolution is16 blocks.
A valley compares a point with opposite shoulders256 blocks away, reporting
relief across512 blocks, not claiming a watershed or a constant valley width.

Two radial tests count candidate extrema with similar heights around several
rings. Quantized plateaus and narrow ridge lines bias these detectors; zero is
not proof of no rings, and more candidates can mean more local peaks. The broad
64/128/256-ring detector and relief images supplement direct visual review.
Neither metric is a trained perceptual circular-feature classifier. The before/
after renders, including the rejected early candidate, inform the visual verdict.

## Performance reproduction

```sh
python3 tools/benchmark_terrain_v4.py --build-only --output work/s36-core
python3 tools/benchmark_terrain_v4.py --reuse --output work/s36-core --samples 128 --warmups 8
python3 tools/benchmark_terrain_v4.py --reuse --reverse --output work/s36-core-reverse --samples 128 --warmups 8
python3 tools/benchmark_streaming.py --baseline f527b75c21e8c40db4320a40b7630975fe1accb1 --labels slice35 slice36 --terrain-v4 --distance-expansion --rendering-install --build-only --output work/s36-stream
python3 tools/benchmark_streaming.py --baseline f527b75c21e8c40db4320a40b7630975fe1accb1 --labels slice35 slice36 --terrain-v4 --distance-expansion --rendering-install --reuse --repeats 2 --frames 60 --output work/s36-stream
```

Build both exports first; run timings with no concurrent builds/tests/renderers.
The reverse run reuses the same binaries/manifests (copy the build output directory
before changing order). Disposable instrumentation stays in work/. Source and
executable hashes, raw observations and P50/P95/P99/**max** are retained.
`density_plan_us` times the complete v11 initial terrain/profile/material stage;
it is not a pure isolated noise microbenchmark. Column planning includes all
remaining admission/root work. Raw generation has separate allocation-call,
requested-byte and peak-live-heap counters, including density planning. WorkerCPU/wall, raw column generation, lighting,
meshing, allocations, requested heap, compact slabs, caches and install costs are
separate. CPU Bevy publication/update/visibility is not GPU submission or FPS.
The current build also runs v10 and checks block/light/full-geometry signatures
against the exact baseline at every matched benchmark location.

No Core i3-13100, RX9060 XT, physical GPU, 1080p/240Hz or native FPS claim is made.

## Native acceptance: Gentoo / Void / Windows

Use an isolated **new** directory, keep its printed absolute path for reload, and
keep closed-directory copies of old saves for compatibility checks. Existing
worlds will deliberately keep their recorded generator. For Void/Fish:

```fish
git fetch origin
git switch feat/slice-36-minecraft-style-terrain
git pull --ff-only
set test_dir (mktemp -d /tmp/rustcraft-slice36.XXXXXX)
echo $test_dir
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=4 cargo run --release --locked -p rustcraft
# Same path after normal close: saved world, Air edits and inventory must reload.
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_LOAD_RADIUS=4 cargo run --release --locked -p rustcraft
```

Gentoo Bash/Zsh: use `test_dir=$(mktemp -d /tmp/rustcraft-slice36.XXXXXX)` and the
same `env` invocations. Use the platform's normal X11/Wayland and Mesa/driver setup.
Windows10/11 PowerShell (native MSVC tools and supported graphics driver):

```powershell
$env:RUSTCRAFT_DATA_DIR = Join-Path $env:TEMP ("rustcraft-slice36-" + [guid]::NewGuid())
$env:RUSTCRAFT_WORLD_SEED = "42"
$env:RUSTCRAFT_LOAD_RADIUS = "4"
cargo run --release --locked -p rustcraft
# Restart with the same environment/path after normal close.
```

Create several named worlds with the four survey seeds; inspect plains, forest,
foothills, ranges, coast and valley sites from the capture manifest. Compare v10
in a copied historical world or read-only gallery, not by changing a manifest.
Walk/sprint/jump along cliffs and chunk seams, enter caves and water, mine/build
across signed region boundaries, remove vegetation, place torches/buckets and
explicit Air, save/restart, evict/return, switch A→B→A and close during streaming.
Test radii2/3/4, rapid exploration, fog/F3, X11/Wayland capture/focus and Windows
resize/close. Record GPU/backend, RAM, frame tails and the intended1080p mode.
Native GUI, physical Radeon and owner visual acceptance remain manual gates;
Linux/Windows CI is compilation/headless validation, not those measurements.

## Measured geography

Raw16-block survey, identical windows and elevation scale. Quantiles are Y values.

| Seed | v10 P50/P95/P99/max | v11 P50/P95/P99/max | v11 area Y≥70 | v11 land32 span≤1 /≤2 /≤4 |
| --- | --- | --- | ---: | --- |
| 0 | 10/85/124/155 | 21/83/131/160 | 7.75% | 5.50% / 12.13% / 21.73% |
| 42 | 10/97/129/151 | 21/85/131/159 | 8.05% | 5.10% / 11.94% / 21.93% |
| 2026 | 8/67/110/151 | 21/81/127/158 | 7.22% | 6.06% / 13.03% / 22.88% |
| 18446744073709551615 | 7/76/123/154 | 20/78/125/158 | 6.55% | 5.11% / 11.73% / 21.68% |

V11's largest Y≥70 sampled components span416–1008 blocks along their longest
bounding dimension, with280–548 samples, compared with2534–4418 broad v10 samples.
The narrower ranges have less contiguous high plateau area, with connected
foothills and intervening saddles. This threshold is not a claim that every crest
remains above70. Sampled512-block valleys reach120–129 blocks of shoulder relief.

The ridge skeleton orientation counts (E/SE/S/SW) are2520/1871/2593/2127 for
seed0; the rejected one-direction model was2216/804/3129/3466. Other final seeds
have similarly distributed counts. Skeleton length grows from23–43km to146–181km
over these large windows, reflecting narrow branching features rather than a
claim of real drainage/ridge surveying. Full branches, isolated components,
multi-scale differences, slopes and correlations are in the JSON reports.

The broad radial detector reports31–45 v11 candidates among758–838 broad extrema,
versus0–3 among312–399 in v10. **This proxy does not numerically establish ring
elimination.** V10's flat centers and large contour walls often escape a64–256
block local-maximum test; v11 has more distinct rounded peaks as well as narrow
ridges. The actual matched panorama is the stronger evidence of the removed
large ring pattern. Naturally rounded hills remain intentional. No zero-circle
or worldwide visual guarantee is claimed.

Final hydrology64-grid maxima are154–156 for v11; raw16-grid maxima are158–160.
Final one-block slope P95 is2–3, P99 is5–6, and max is13–16 versus v10 P95/P99=1
and max4. These genuine steep steps include 3D cliff edges; v11 intentionally
does not smooth every slope into a uniform ramp. Their collision/water/point/mesh
composition is covered by the integration regressions.

[Raw measured datasets and hashes](raw/survey-files.json),
[v10 analysis](diagnostics/quality-v10.json), [v11 analysis](diagnostics/quality-v11.json).
All compressed CSVs use deterministic gzip metadata and can be independently
recreated with the commands above.

Diagnostic maps use the same fixed altitude and relief color scale:

| v10, seed0, central8192² | v11, same region/seed |
| --- | --- |
| ![Diagnostic v10 relief, not a render](diagnostics/v10-seed0-region0-relief.png) | ![Diagnostic v11 relief, not a render](diagnostics/v11-seed0-region0-relief.png) |

![Measured cross-sectional height profiles; diagnostic, not a render](diagnostics/terrain-profiles.png)

The profile plot uses optional development-only Matplotlib:
`python3 tools/plot_terrain_v4.py work/quality10.csv work/quality11.csv --output work/terrain-profiles.png`.

## Validation

Rust1.99.1 / Bevy0.20.0 / edition2024; lockfile, dependencies and original texture
artifacts are unchanged. Required formatting, locked workspace/all-target Clippy,
asset verification and normal release builds pass. **563 workspace tests pass**
(220 client,329 core unit,14 integration; seven existing optional tests ignored).
The optimized core suite also passes; optimized client streaming46 and session12
tests pass separately. The complete debug run and the CI workspace job use `--test-threads=1` to avoid
wall-clock lifecycle deadlines competing with each other in the two-vCPU cloud
quota. An earlier concurrent run timed out in the radius4 lifecycle test while
compilers and software renderers were also active; its isolated rerun and the
complete serial run pass. No production timeout, worker limit or test deadline
was enlarged to hide this result.

A hint-only inline experiment did not improve the measured planning cost and
was discarded. Profiling instead identified duplicate rim/root height evaluation;
sharing the already evaluated raw/final grids removes that work. All36 column
CRCs and the independent field/density fixtures remain exact after this change.

The68 independent Python fixtures pin continentalness, erosion, weirdness, offset,
amplitude, surface top, guaranteed floor and every density occupancy bit. The36
full-column CRCs freeze final Rust v11 block output (they are not independently
derived Python block fixtures). V1–v10 historical goldens remain intact. Full
density-band point/raster checks prevent the old height-minus-four slab shortcut
from masking recessed geometry. The audit exercises foundations, materials,
plants/trees, water quietness, caves, signed/distant/world edges, reverse order,
halos and parallel generation. Existing journal interruption, explicit Air,
builds/inventory, cancellation/eviction, session A→B→A and shutdown checks include
v11 as appropriate.

## Measured column cost and memory

Environment: AMD EPYC9V74 virtual machine, **two-vCPU quota**, 8GiB
RAM limit, Linux; headless release CPU tests. The actual environment is retained
in [environment metadata](raw/environment.json). No build/test/renderer ran during
the timed final runs. Both execution orders use the same verified binaries;128
measured samples plus8 warmups per column, two trials per streaming workload.

Source35 is exact f527b75c21e8c40db4320a40b7630975fe1accb1, version10; source36
is the current version11 export. The current export additionally runs10 and its
block/light/full-mesh signatures and resource counts match the exact baseline in
both orders. The source/executable hashes are retained with each dataset. The
core export predates only a test-assertion wording edit (`fixed independent` →
`independently checked`); its runtime source is otherwise identical. Scene
names are fixed workload labels inherited from the v10 comparison, not proof
that both generators have the same terrain class at those coordinates.

Forward-order column CPU microseconds. Raw is a newly planned full-height column
with the existing candidate/pool memo warm; warm worker reuses its pristine column
cache and measures lighting/meshing/publication preparation. Cold streaming is
measured separately. Median values are shown here; all P95/P99/max are retained.

| Workload | v10 rawCPU P50 | v11 rawCPU P50 /P95 /P99 /max | v10→v11 warm-workerCPU P50 | v11 terrain/density-stage P50 |
| --- | ---: | --- | --- | ---: |
| plain | 467.6 | 987.4 / 1584.1 / 2403.7 / 3690.2 | 858.4 → 1217.0 | 656.4 |
| forest | 558.6 | 1760.6 / 2398.6 / 3115.5 / 4008.0 | 755.9 → 782.9 | 623.9 |
| peak | 322.3 | 1247.8 / 1590.9 / 1875.6 / 4349.4 | 705.2 → 1032.0 | 756.3 |
| valley | 248.1 | 582.6 / 809.2 / 1268.4 / 1355.0 | 317.4 → 465.5 | 396.9 |
| transition | 230.2 | 350.7 / 466.0 / 523.9 / 893.0 | 378.3 → 347.9 | 250.2 |
| coast | 273.9 | 365.1 / 515.1 / 898.8 / 1867.8 | 554.6 → 273.5 | 265.2 |
| distant | 342.2 | 464.1 / 569.3 / 627.0 / 702.4 | 772.8 → 674.4 | 281.2 |
| max-seed | 399.5 | 455.6 / 701.8 / 1593.0 / 4724.9 | 1182.5 → 809.5 | 266.6 |
| cavern | 423.4 | 567.0 / 864.3 / 1062.4 / 8761.9 | 707.9 → 730.8 | 304.6 |

The new geometry is not free: raw planning is slower than v10, especially in
forests and rugged areas. Sharing the raw water rim and final root-surface grid
reduced initial v11 raw medians by34–41% at the plain/forest/peak sites; the
[before/after planning observations](raw/planning-optimization.csv) keep the
rejected duplicated-evaluation cost visible. Noise itself, not the full256-block
volume, is evaluated only in the compact surface band.

| Workload | v10→v11 column mesh bytes | v10→v11 raw peak live requested heap bytes | v10→v11 raw allocation calls |
| --- | --- | --- | --- |
| plain | 111352 → 147312 | 25412 → 22932 | 69 → 26 |
| forest | 113336 → 133424 | 28164 → 25716 | 95 → 51 |
| peak | 12648 → 167648 | 16212 → 21268 | 20 → 19 |
| valley | 27776 → 102920 | 16148 → 21332 | 9 → 15 |
| coast | 71416 → 29760 | 16116 → 21300 | 10 → 13 |
| max-seed | 122760 → 145072 | 24436 → 29060 | 33 → 27 |

The photographed range's narrow rock faces can produce far more mesh than a
flat v10 summit: the peak column grows from12648 to167648 bytes (about13.3×),
while its warm-worker median grows from0.705 to1.032ms in this run. This is a real
geometry/resource tradeoff, not a renderer speedup. Other sites shrink, such as
the coast column. Raw heap excludes allocator metadata, stacks, GPU memory and
other process resources; allocation calls are separate from retained capacity.
Streaming RSS and global worker-window allocation peaks are reported separately.

[Core forward percentiles](raw/core-forward/percentiles.csv.gz),
[reverse percentiles](raw/core-reverse/percentiles.csv.gz), and the per-observation
compressed CSVs in those directories retain every raw value, stage, allocation,
cache, compact Air/Stone slab count, signature and paired save/reload measurement.
Percentiles use floor((n-1)*p/100), without deleting outliers.


## Measured streaming, publication and resident resources

Forward order, milliseconds; fixed scripted240Hz deadlines,60 movement frames,
two trials per workload. Request-ready includes the one-worker queue and ends
when blocks/light are installed, before ECS mesh publication. It is not a frame
time. Workload labels retain their original coordinates; the new geography can
have a different slope/biome class there. Radius1 remains in raw data as a control.

| Workload | Radius | v10 workerCPU P50 | v11 workerCPU P50/P95/P99/max | v10 request-ready P50 | v11 request-ready P50/P95/P99/max |
| --- | ---: | ---: | --- | ---: | --- |
| stationary | 2 | 1.37 | 1.52 / 3.60 / 5.34 / 5.36 | 22.43 | 32.67 / 77.70 / 83.26 / 83.28 |
| stationary | 3 | 1.25 | 1.76 / 4.56 / 4.78 / 9.64 | 47.94 | 75.35 / 209.25 / 223.65 / 231.10 |
| stationary | 4 | 1.27 | 1.51 / 3.34 / 5.00 / 8.14 | 85.45 | 129.36 / 264.73 / 279.81 / 286.31 |
| sprinting | 2 | 1.12 | 1.53 / 3.45 / 4.87 / 5.27 | 20.84 | 27.40 / 71.53 / 83.18 / 85.47 |
| sprinting | 3 | 1.13 | 1.34 / 3.04 / 4.45 / 4.91 | 40.51 | 58.02 / 135.91 / 143.38 / 150.38 |
| sprinting | 4 | 1.09 | 1.59 / 3.57 / 5.46 / 8.65 | 67.96 | 126.18 / 277.78 / 293.79 / 302.09 |
| mountain | 2 | 1.28 | 2.72 / 5.35 / 12.76 / 13.34 | 24.35 | 44.02 / 98.72 / 104.79 / 105.04 |
| mountain | 3 | 1.21 | 3.16 / 5.94 / 12.38 / 21.97 | 48.26 | 108.18 / 227.13 / 247.60 / 256.46 |
| mountain | 4 | 1.37 | 2.90 / 7.60 / 13.57 / 27.30 | 92.22 | 176.99 / 358.27 / 378.85 / 391.55 |
| valley | 2 | 1.06 | 1.41 / 3.68 / 5.98 / 6.54 | 20.94 | 30.31 / 63.26 / 70.24 / 72.56 |
| valley | 3 | 0.97 | 1.52 / 4.72 / 5.92 / 9.12 | 42.96 | 61.85 / 120.43 / 125.16 / 127.76 |
| valley | 4 | 1.02 | 1.51 / 4.21 / 7.42 / 9.90 | 71.52 | 99.67 / 194.47 / 208.13 / 212.16 |

At radius4 the mountain worker median grows from1.37 to2.90ms (wall1.37→2.92ms);
request-ready median92→177ms, P99325→379ms. V10 also has a60.9ms worker maximum
in this run; it is retained rather than removed. The v11 mountain maximum is
27.3ms. Sprinting worker median1.09→1.59ms and request-ready68→126ms illustrate
that the cost also affects traversal. These are material CPU/queue regressions,
not a claim of unchanged performance. Shared planning reduced avoidable work;
remaining geography and mesh costs need native owner assessment before merging.
The one-worker, cancellation, fairness, cache and radius bounds are unchanged.

Radius4 stage CPU-wall medians in milliseconds (timers inside the worker):

| Workload | Generation v10→v11 | Lighting v10→v11 | Opaque mesh v10→v11 | Worker wall v10→v11 |
| --- | --- | --- | --- | --- |
| stationary | 0.360 → 0.452 | 0.330 → 0.358 | 0.260 → 0.421 | 1.270 → 1.511 |
| sprinting | 0.376 → 0.510 | 0.325 → 0.377 | 0.237 → 0.434 | 1.095 → 1.611 |
| forest | 0.428 → 1.027 | 0.330 → 0.487 | 0.245 → 0.357 | 1.193 → 2.125 |
| mountain | 0.374 → 1.198 | 0.423 → 0.750 | 0.235 → 0.447 | 1.373 → 2.918 |
| valley | 0.330 → 0.803 | 0.262 → 0.307 | 0.181 → 0.200 | 1.020 → 1.511 |
| coast | 0.380 → 0.671 | 0.298 → 0.439 | 0.251 → 0.309 | 1.134 → 1.657 |

Water/cutout timings are separate in the raw data. Stage medians do not sum to
total medians because they can belong to different jobs and source/halo work.

| Radius4 workload | v10 main-install P50/P99/max (ms) | v11 main-install P50/P99/max (ms) | Stream-update P99 v10→v11 (ms) |
| --- | --- | --- | --- |
| stationary | 0.039 / 0.218 / 0.279 | 0.713 / 2.403 / 2.977 | 0.314 → 1.550 |
| sprinting | 0.039 / 0.148 / 0.179 | 0.785 / 1.815 / 1.940 | 0.342 → 1.951 |
| forest | 0.044 / 0.462 / 0.538 | 0.044 / 0.115 / 0.219 | 0.573 → 0.325 |
| mountain | 0.038 / 0.745 / 1.279 | 0.046 / 0.109 / 0.275 | 1.242 → 0.339 |
| valley | 0.037 / 0.106 / 0.132 | 0.038 / 0.144 / 0.221 | 0.314 → 0.358 |

Main install includes chunk/light integration and CPU mesh-asset updates; whole
stream updates also include queue/eviction/dispatch work. The origin/plains
workloads show higher install cost, with P99 up to2.40ms at the stationary site;
this result is not hidden by the cheaper mountain-site publication. Timers include
the identical disposable instrumentation and allocator meter in both builds.
The separate identical retained-install fixture at radius4 publishes81 chunks
with243 opaque/water/cutout assets: CPU publication median1.437→1.458ms,
P992.452→2.831ms, max7.364→4.433ms; visibility-update median1.137→1.151ms.
This fixture isolates existing renderer behavior and does not substitute for
publishing the actual changed mountain meshes or measure GPU submission.

All ordinary radius4 final views have81 columns /1296 compact chunk descriptors.
Resident resource medians below use decimal MB; the CPU resident mesh payload is
not VRAM. Cache columns stay bounded by the existing view/halo policy.

| Radius4 workload | Mesh payload MB v10→v11 | Pristine cache MB v10→v11 | Light cache MB v10→v11 | RSS KiB v10→v11 | Worker-window requested-heap maximum MB v10→v11 |
| --- | --- | --- | --- | --- | --- |
| stationary | 8.778 → 10.035 | 1.368 → 1.389 | 2.432 → 2.362 | 46196 → 47916 | 0.898 → 0.925 |
| sprinting | 8.713 → 10.578 | 1.384 → 1.417 | 2.438 → 2.362 | 46696 → 47324 | 0.898 → 0.924 |
| forest | 10.099 → 17.761 | 1.368 → 1.909 | 2.426 → 3.126 | 49392 → 61928 | 1.052 → 1.261 |
| mountain | 9.077 → 21.328 | 1.303 → 2.314 | 2.473 → 3.750 | 47708 → 66356 | 1.258 → 1.554 |
| valley | 5.885 → 6.866 | 1.073 → 1.257 | 2.053 → 2.187 | 47708 → 61768 | 1.003 → 1.071 |
| coast | 9.119 → 13.812 | 1.241 → 1.659 | 2.234 → 2.811 | 49392 → 58508 | 1.008 → 1.222 |

RSS/high-water marks are process-wide, cumulative and order-dependent, including
the test harness and previous scenes; they are not isolated per-terrain allocation
claims. Worker-window heap is global incremental requested heap and can include
concurrent main-thread allocations. Stacks, allocator metadata and GPU allocations
are excluded. Independent allocation-call/live-heap column results are above.

Reverse-order results are retained separately, not pooled to erase tails. Its
radius4 workerCPU medians (v10→v11 ms) are: stationary 1.13→1.40, sprinting 1.07→1.40, mountain 1.27→3.51, valley 1.03→1.96. All168 final state
signatures per build are identical across execution orders, including resources,
editing, travel, radius changes and sessions. Geometry differs between10 and11
by design. Failed-worker counts remain zero; gated cancellation and shutdown
complete. No raw tail samples were deleted.

[Forward streaming percentiles](raw/stream-forward/percentiles.csv.gz),
[reverse streaming percentiles](raw/stream-reverse/percentiles.csv.gz), raw
`samples.csv.gz`, exact source/binary SHA256 manifests, method and final-state
signatures are retained in those directories. The method corrects an inherited
v9/v10 descriptive label; actual executed generator arguments were10/11 and the
as-executed harness hash is preserved. Native1080p/Radeon frame and GPU acceptance
remains a separate manual check; no native FPS improvement is claimed.
