# Biome Generation v1 — Slice25 / generator v5

## Current new-world generator: v6 (Slice 26)

New worlds now select6, retaining manifest5. `WorldHeight` defines Y[-64,192),
chunk Y[-4,12), with minimum inclusive / maximum exclusive. Every v6 cell in
Y[-16,32) equals frozen v5 at the same seed/X/Z. New deeper cells are Stone;
Y[32,192) is Air. Climate, biome distribution, surface materials, caves and tree
crowns are unchanged. Uniform new slabs are returned without per-voxel work.
Existing generators1–5, bounds and golden fixtures remain unchanged. The biome
specification below documents v5 and the shared v6 surface stages. Earlier
fixed-three-slab audits are historical. See [v6 goldens, readiness, limits and
acceptance](VERTICAL_WORLD.md).

New worlds use generator **5** in the existing regional **manifest v5**. All
historical generator1–4 paths and goldens retain their exact output. Existing
manifest v4 always means generator3; existing v5/gen4 stays v5/gen4. No world is
converted, expanded, regenerated or assigned a new generator on load. Explicit
regional Air remains authoritative. Unsupported generator identities fail before
creation/recovery can replace data. See [SAVE_FORMAT](SAVE_FORMAT.md).

## Climate and geographical distribution

`voxel_core::worldgen::{Biome, Climate, sample_biome, sample_climate}` is an
engine-independent, bounded X/Z query. `Generator::biome` reports `None` for
historical generators; the client F3 overlay says LEGACY instead of inventing
biomes in old worlds. Queries require no chunks, neighbors, disk or stored map.
Unsupported horizontal coordinates return `None` before arithmetic.

All fields use the existing wrapping-u64 coordinate hash and signed integer Q10
smoothstep value noise. The original bounded36-block domain warp at384 blocks
feeds temperature (five parts768-block noise + one part384), moisture (four
parts640 + one part320), and Slice24's512-block geographical field. Independent
salts211/223 and227/229 separate climate from terrain and each other. Sampling
uses Euclidean division at negative coordinates and no floating-point RNG.

Classification, in priority order, on signed Q10 values (nominal[-1024,1024]):

| Condition | Biome | Character |
| --- | --- | --- |
| Geography >240 | Rocky Highlands | Elevated connected ridges, exposed rock, rare trees |
| Temperature <−260 | Snowy Plains | Pale full-block snowy ground, gentle rolling terrain, sparse trees |
| Temperature >180 and moisture <−100 | Desert | Low dunes/depressions, five-block sand cover, no roots on sand |
| Moisture >80 | Forest | Existing tree species, denser irregular stands and broad clearings |
| Otherwise | Plains | Grassy building areas, occasional rolling hills and sparse trees |

Dry temperate land can remain plains; warmth alone is insufficient for desert.
Highland temperature is independent, so warm uplands remain rocky. The committed
[survey](performance/slice25/biome-sites.csv) covers an8192² footprint at32-block
spacing for seeds0/42/2026/u64::MAX, including all five biomes on every seed.
Each type occupies approximately10–38% across those surveys. Tests also check
connected96² interiors, neighbor continuity and boundary offsets relative to16.
These are representative regression guards, not a minimum area guarantee for
every isolated threshold contour throughout the entire supported world.

## Terrain and material transitions

`worldgen::biome` extends Slice24's same plain/rolling/ridge model. Broad climate
weights reduce hill influence in arid/cold lowlands, blend in low80/28-block dune
noise, and retain the geographical ridge elevation. Smoothstep weights, rather
than the discrete biome label, control terrain. There is no height jump when a
classification threshold is crossed. The convex height bounds remain[-1,24];
no final height clamp creates truncated mountain tops.

A continuous24-block field offsets material thresholds by up to65 Q10 units.
This makes coherent irregular ecotones rather than one-block random stippling.
Surface temperature loses18 Q10 units per block above Y10. Snow can occur on
cold upper slopes without covering every highland. At geography>160, a two-block
neighbor slope or elevation>=13 exposes stone (snow when sufficiently cold).
Else cold ground becomes Snowy Ground, hot/dry lowland becomes Sand, and the
remaining ground uses the original Grass. Sand extends through depth4 below the
surface; other soils retain the three-block Dirt cover over Stone. Caves still
carve the established periodic tunnels and four-block entrance ramps, evaluated
against v5's own local surface. No new cave algorithm, natural water or structure.

## Vegetation and bounded work

The existing four/five-block trunk and original leaves/wood species remain.
V5 candidates occupy eight-block cells with independent offsets1–6 on both
axes, using salt257. Climate smoothly varies acceptance from95 to780 per mille
before cold/arid/highland suppression and a64-block clearing field. Root surfaces
must be Grass or Snowy Ground. Sand/Stone roots are rejected. Every candidate
requires real non-carved support, <=2-block local relief, a clear5² crown footprint
and a crown below Y31. The origin spawn reservation remains open. Trunks take
precedence over overlapping crowns; accepted tree output is sampled in world
coordinates, including neighboring candidates at chunk/region boundaries.

One temporary20² height/climate grid supplies an18² surface/halo plan. Its cached
materials and heights serve all three vertical chunks, so climate/material
selection is not repeated for every voxel. Candidate checks remain bounded local
queries. No area-wide allocation, startup survey, persistent biome map or extra
streaming cache is introduced. Worker, edit, lighting, eviction and render-origin
contracts remain the same. [Measurements and reproducible scenes](performance/slice25/README.md).

## Blocks, art and current limits

Sand is block20/item10; Snowy Ground is block21/item11. Existing block0–19 and
item1–9 retain their meanings, including torch mounts, water states and legacy
item7. Both new materials are ordinary full, opaque, colliding, mineable cubes;
sand gravity and snow accumulation/layers/weather are deliberately absent.

Three original16² tiles (sand, snow_top, snow_side) append slots8–10 to the atlas,
now128×96 RGBA with the same32-pixel pitch and8-pixel extrusion. Slots0–7 and
all their art/gutter bytes are unchanged. Snowy Ground has a snowy top, cold
snow/dirt side and original dirt underside. All game, greedy shader, icon, held
and dropped-torch UV scales use the extended layout. Assets/source/tooling are
original RustCraft MIT OR Apache-2.0 work; no downloaded texture/reference pack.

The supported Y range is still **−16 through31**, with ground<=24 and crowns<=30.
Major mountains and deeper caves require a separate versioned vertical-expansion
slice covering world bounds, save limits, lighting halos, streaming, physics and
memory budgets first. Small render radii constrain the visible geographical
breadth. Forests reuse the present opaque cube crowns; new species/details,
weather, water geography, ores and structures remain deferred.

---

# World generation — frozen v1–v3 and geographical v4

## Slice 24: generator v4, regional manifest v5

**New worlds alone select generator4. Generators1/2/3 and all historical golden
fixtures remain frozen.** Generator versions describe generated blocks; save
versions describe serialization. Existing manifest4 continues to mean generator3.
Manifest5 uses the same 64-byte layout with an explicitly supported generator
(currently4). Unsupported future generators fail with a generator error; no
migration, conversion, fallback, block-ID change or rewriting on load occurs.
See [SAVE_FORMAT](SAVE_FORMAT.md#slice-24-versioned-regional-manifest-v5).

### Geographical shape

`worldgen::terrain` contains the original, integer-only v4 height function:

- Two 384-block noise fields warp X/Z by at most36 blocks, removing obvious
  alignment with chunks or the 64-block edit regions.
- A 512-block geographical field smoothly controls plain/hill/mountain weights.
  These are continuous shape weights, not biome types or abrupt region choices.
- Plains combine a base elevation with 384-block lowland variation and subtle
  64-block contours. Rolling regions combine96/40-block slopes. Their amplitudes
  blend with cubic smoothstep; substantial quiet areas remain common.
- Mountain regions use a skewed96-block ridge field, squared for connected
  crests/valleys, plus128-block elevation variation. Smooth geographical exposure
  connects multiple crests into broad elevated regions. Mountain weights blend
  heights rather than selecting independent per-region algorithms.
- A12-block detail field contributes only0.125 blocks on plains, rising to0.375
  in mountains. Fractional Q10 heights are retained until final floor division.
  No final height clamp is used.

Every sample has bounded work and depends solely on seed/version/world X/Z.
Noise/hash use explicit wrapping u64, Q10 integer arithmetic and Euclidean cells.
Signed skew division is Rust's explicit truncation toward zero on both platforms;
final surface rounding uses `div_euclid`. There is no floating point, mutable RNG,
hash iteration dependency, world-sized plan or dependency on resident neighbors.
Point and chunk sampling share the same height/cave/tree rules. Column samplers
reuse the existing18×18 height halo and bounded nearby eight-block tree grid.
The asynchronous worker, light halos, stale-result checks and cache eviction are
unchanged. Full-world materialization remains forbidden for local generators.

### Materials, caves, trees and spawn

Shape lives separately from `local::terrain` (grass, three dirt layers, stone),
`local::cave`, and tree placement. This is the integration point for the later
biome slice; no climate fields, biome registry or unused framework is added.
Future output changes must select a new generator and extend the explicit
supported-version list; manifest5 and the existing region/journal layouts already
carry that identity.

Caves retain the existing protected spawn, bottom two foundation layers, periodic
seeded tunnel and four-block-high entrance ramp. V4's own surface determines its
entrance height. This is a compatibility adaptation, not a new cave system. The
old128-block cave repetition remains a limitation for Slice26. V4 tree candidates
retain the existing species/crown/grid, with fewer roots and a five-by-five slope
check (maximum2 blocks relative to the root), grass support, full canopy clearance
and top <=30. Cross-column trunks take priority over leaves as before. The
protected area around(0,10) stays open. Actual edited blocks still determine
startup/R/void spawn through the existing bounded sky-exposed clearance search;
no physics change or generated spawn platform is needed.

### Vertical audit and future expansion

The current bounds remain X/Z[-32768,32768), Y[-16,32): three16³ vertical chunks.
The convex height blends analytically bound surface Y to[-1,24]. The measured
four-seed survey reaches0..24; taller crowns finish at30, leaving Y31 clear.
Players have overhead above even the highest ground. The bottom two layers stay
Stone, and caves do not remove whole mountain foundations. Mountains are modest
voxel uplands, not modern Minecraft's much taller peaks. Normal render radius1–3
also limits the breadth visible from one location.

A separate vertical-expansion slice should first version world bounds and region
record limits; replace fixed three-chunk assumptions in streaming, source/light
halos, collision/spawn bounds, save/journal validation and mesh lifecycle; then
measure memory, lighting flood costs and recovery before selecting a taller new
generator. Existing worlds must retain their original bounds. Slice24 does none
of that expansion and never fakes height through clipped summits.

### Evidence and remaining work

[Performance, actual client gallery captures, coordinates and Void acceptance](performance/slice24/README.md).
New independent goldens cover seeds0/42/2026/u64::MAX at six signed/distant columns.
Tests cover parallel/reverse requests, point sampling, regeneration, strict v5
identity, journal interruptions, region/lighting/mesh seams, spawn, grounded trees,
canopy clearance, cave ramps, foundation connectivity and multi-seed quality.
Quality indicators sample a4096²-block footprint per seed: height/slope histogram,
32² buildable footprints, and connected lowland/upland components. These are
regression guards, not a claim that every region is attractive or walkable.

Slice25 can influence versioned shape/material selection; Slice26 can replace
periodic caves; Slice27 adds natural water; Slice28 adds vegetation/detail rules.
None is implemented here. Native Void X11/Wayland, Radeon performance and Windows
GUI acceptance remain separate from cloud tests/software captures.


## Historical generator v3 (v4 regional worlds)

Generator v3 locally addresses X/Z [-32768,32768), Y [-16,32). Construction stores
only seed/version/extent; no heights/tree arrays proportional to world area.
Each column sampler builds an 18×18 height plan (16² center plus one-cell halo)
and nearby deterministic eight-block-grid tree anchors. Three vertical chunks
and their thin face borders reuse that plan. Point queries use the same rules.

Heights use original integer Q10 smoothstep noise/hash composition. Caves and
entrance ramps repeat by 128-block world-coordinate neighborhoods with seeded
centerline variation; spawn ground is protected. Candidate roots are checked for
supported grass and canopy clearance. Trunks win before leaves; crowns cross
chunk/region boundaries without writing into neighbors. Output depends only on
seed/version/coordinates, not visitation or resident neighbors. No external noise
or asset library is added.

The old small-world output is not silently retuned: generator v1/v2 branches and
goldens remain frozen for their saved identities. V3 has separate far-column
CRC goldens for seeds 0, 42 and u64::MAX, sampler/point and full-neighbor/halo
comparisons, distant canopy/cave tests and extreme query checks. Never change
these outputs in place. Larger supported bounds or changed structures require a
new generator/version policy. `generate()` deliberately refuses v3 full-world
materialization; only local requests are supported.

Regional edits apply after generation, including explicit Air, before chunks
become ready. Neighbor faces use the same saved identity/overlay even when their
GPU chunks are absent. Corrupt regional data and missing primaries with backups block loading
rather than regenerating. An absent region without a backup means unedited terrain. See SAVE_FORMAT and ARCHITECTURE for authority/cache lifetime.

Rendering uses a floating origin; physics and targeting keep f64 world positions.
The supported 65,536-block extent is a deliberate rollout, not literal infinity.
The 48-block vertical range and modest radius remain unchanged. There are no new
textures, biomes, water, ore systems, growing trees or generator-owned lighting fields. Derived Slice-10 skylight is reconstructed separately (LIGHTING).

## Historical frozen generators

The sections below document retained older modes; new worlds use v4 above.


The implementation is original RustCraft code, MIT OR Apache-2.0, in `voxel_core::worldgen`. No Bevy, shared RNG, noise library or copied game assets are used. This is a finite first generator, not an infinite-world/biome system.

## Frozen generator-v1 contract

`GenerationConfig { seed: u64, version: 1 }` selects the entire current algorithm. Default seed: **2026**. Unsupported versions return an error. Changes to the generated output require a new generator version and updated fixtures; tuning is not silently changed under version 1.

Coordinate hashing uses explicit wrapping u64 operations and fixed mixing constants. Heights use three scales (32, 16, 8 blocks) of integer Q10 smoothstep value noise. Euclidean cell division handles negative coordinates continuously. There are no floating-point noise functions, random-stream advancement, platform-dependent hash maps or dependence on traversal/thread order. Current golden full-snapshot CRCs pin seeds 0, 1, 42, 2026 and u64::MAX; CI checks the same fixtures on Linux and Windows.

`Generator::new` builds a finite 64×64 i32 height cache (~16 KiB) and a small list of deterministic tree anchors. `block(BlockPos)` and `chunk(ChunkPos)` are immutable queries. Chunk requests can be evaluated in any order; crowns and cave cells are sampled in world coordinates without writing into a neighbor. `generate` stores only nonempty chunks. No unsupported empty chunks are inserted.

## Terrain, caves and trees

- The editable/serialized volume remains X/Z [-32,32), Y [-16,32): at most 48 resident 16³ chunks. Outside is air. The finite edge is intentionally visible; there is no surrounding ocean or streaming.
- Layered coherent noise creates broad hills/valleys and smaller contours. Surface block Y is bounded to [-8,18], with grass on top, three dirt layers and stone deeper. The bottom two layers remain solid, keeping the land connected and avoiding unsupported spawn regions.
- A continuous east/west cave near z=-10 varies its centerline with seeded coherent noise. Wider nodes form small caverns. A four-block-high, three-block-wide stair-stepped access ramp near x=-10 descends from the surface around z=20 toward the tunnel. Its height changes by at most one block per horizontal step, so jumping traverses its steps. The central spawn area is protected from caves. This deliberately simple network is not a biome/3D cave noise implementation; some tunnel openings appear at the finite boundary.
- Tree candidates occupy an eight-block grid with deterministic jitter/omissions. Trunks are four or five blocks tall; crowns use a compact trimmed block shape. The anchor is supported grass, the canopy clears terrain, candidates stay inside bounds, nearby anchors are separated by at least six blocks on one horizontal axis, and the spawn/entrance areas remain open. Crowns can cross chunk boundaries. Trees do not grow/decay. Wood and leaves are opaque solid blocks, with ordinary collision/culling and original colors.

## Spawn and saved-world authority

The client prefers a column near (0,10), but computes feet height from **actual authoritative blocks** using `physics::spawn_position`. It searches nearby sky-exposed ground, skips tree tops, checks player clearance and uses the same routine at startup, R and void recovery. It does not reset generated worlds to the old demo Y. Edits may relocate the safe column; no blocks are created/removed for spawn. A valid all-air snapshot remains all-air, with a clear-air fallback because supported ground is impossible.

The seed environment variable is consulted only by the lazy new-world factory after confirming neither primary nor backup exists. Loaded v1/v2 full snapshots never invoke a generator or parse a seed override. They retain finite missing-cell-is-Air semantics. Slice-6 v3 worlds select generator v2 and persist its identity in v3, rather than storing a dense generated world. See [the save/upgrade contract](SAVE_FORMAT.md).

## Generator v2 and bounded streaming

Legacy v3 worlds select `GenerationConfig { seed, version: 2 }`. This extends the sampling volume to X/Z [-64,64), Y [-16,32), with a 128×128 height cache (~64 KiB) and additional deterministic eight-block tree-grid anchors. It deliberately preserves the original height/noise/cave arithmetic while selecting a new version contract for the expanded bounds/tree plan. Generator v1 remains unchanged and its five golden full-snapshot fixtures still pass. Generator v2 has independent golden column fixtures for seeds 0, 42 and u64::MAX, including expanded negative/positive coordinates. Do not retune either version in place.

Chunk generation is immutable and independent of request order or loaded neighbors. A worker materializes only a center column (three chunks) plus thin face borders for meshing, applies the captured edit overlay, and emits three engine-neutral meshes. Border generation samples just the cells `mesh_chunk` actually reads; it avoids generating eight full neighboring columns. Full-neighbor reference mesh tests verify the thin halo is equivalent, including edits at seams and trees/caves. Rendering a column does not depend on GPU neighbor residency, so loading/unloading neighbors does not change its correct faces.

A (0,0) spawn column is prepared before the App, then follows ordinary radius eviction. R/void requests an evicted spawn and waits before selecting clear edited ground. Remaining desired columns load on one bounded worker; main-thread install requires desired membership and matching local mesh dependencies, not the global save revision. Cache eviction frees block vectors and GPU entities/assets, but never removes edit-map entries. In-volume pending data is a temporary solid collision/ray barrier and is not editable; outside the finite bounds is known Air. New chunks apply edits before becoming ready. See [architecture](ARCHITECTURE.md) for lifecycle/ownership and [v3 persistence](SAVE_FORMAT.md) for crash/recovery limits.

## Performance and next boundary

Legacy worlds retain bounded full startup loading. Streamed worlds prepare only one small spawn column before App/window creation; new-column generation and cached dirty mesh batches run off-thread. The main thread polls without waiting, installs at most one result per frame, discards obsolete work and frees evicted assets. It never performs full-world generation or disk I/O in the frame loop. Default radius 2 retains at most 75 vertical chunks (307,200 block payload bytes); the configured ceiling is 147 chunks / 602,112 bytes, without an extra spawn pin. The new-column thin worker halo holds at most 15 chunk vectors / 61,440 block payload bytes plus mesh buffers. Editing remeshes cached target and face-neighbor data, generating only missing face cells. A large connected mesh batch can materialize up to the finite 192-chunk volume; normal clicks copy only a small neighborhood. Connected dirty targets publish together, preventing mixed old/new faces at edited seams. These counts exclude edit-map allocations, renderer/driver buffers and Bevy overhead. Meshing, renderer buffers and save encoding have separate transient allocations. v3 save snapshots share the immutable overlay without copying the resident cache; no generation or disk I/O runs in the render loop.

Measured debug/cloud timings and functional coverage are recorded in [VALIDATION.md](VALIDATION.md); they are not desktop GPU benchmarks. Larger worlds will need region/halo planning (especially trees), worker scheduling for generation/meshing, memory budgets and streaming. Keep global coordinate sampling deterministic when introducing those systems. Do not extend this finite cache or synchronous mesh startup into unbounded work.
