# Terrain texture and light modulation contract (Slices 8–10)

Slice21 extends this historical baseline with a saved day/night clock, separate
sky/block COLOR contributions, procedural celestial bodies/stars and matching
fog gated by geometric sky exposure. See [the current contract](DAY_NIGHT.md).

All art/source is original RustCraft, **MIT OR Apache-2.0**. Provenance, previews
and regeneration: [assets/terrain](../assets/terrain/README.md). This changes only
presentation and mesh payloads, never block IDs, generator versions or save bytes.

## Layout and sampling

Seven native **16×16** RGBA8 opaque tiles occupy slots 0–6 in a 4×2 atlas.
The image is **128×64**, each slot 32×32. Tile origin is slot origin + (8,8).
Every edge/corner is extruded by eight pixels; unused slot 7 is diagnostic magenta.
`voxel_core::texture` specifies dimensions/slots and face choices independently
of the engine. Slots are not persisted block IDs. Air emits no geometry/UVs.

| Block | +Y | -Y | ±X / ±Z |
| --- | --- | --- | --- |
| Grass | grass_top (0) | dirt (2) | grass_side (1) |
| Dirt | dirt (2) | dirt (2) | dirt (2) |
| Stone | stone (3) | stone (3) | stone (3) |
| Wood | end_grain (5) | end_grain (5) | bark (4) |
| Leaves | leaves (6) | leaves (6) | leaves (6) |

Image coordinates start at the top-left; V increases downward. Wall V decreases
with local Y, keeping grass tops and bark grain upright. Viewed from outside,
wall U is -Z on +X, +Z on -X, +X on +Z, -X on -Z. Horizontal U is +X; V is +Z
on top, -Z on the underside. Normals, CCW vertex/index order and chunk-relative
geometry are unchanged. UV corners cover the complete 16-texel footprint, **not**
a half-texel inset that stretches edge pixels. A compile-time 448-byte quad table
avoids per-vertex divisions or normal matching in hot meshing loops.

The client explicitly chooses nearest min/mag/mipmap, clamp-to-edge addressing,
LOD min/max zero, one mip and anisotropy 1. UVs stay within their tile footprint;
neighboring gutters repeat exactly its edge colors. No global atlas mip chain is
generated (that would blend tiles at coarse levels). These settings avoid bleed
and smoothing, but distant fine detail can shimmer/alias; a correctly padded
per-tile mip chain or texture array needs a separately measured implementation.

## Renderer and ownership

A single embedded `atlas.rgba` creates an sRGB Image once. JSON/PNGs are editable
source and inspectable generated artifacts; none are parsed or found by path at
runtime. No PNG feature, new crate, Python build/runtime dependency or asset-server
loading race is introduced. The binary works from an unrelated working directory.
CI uses the hosted runner's Python 3 standard library to verify generated files.

One opaque, white, unlit `StandardMaterial` references that Image. Derived voxel
skylight modulates neutral per-face RGBA vertex colors with fixed direction
factors; global ambient is zero and no directional entity washes out caves. Leaves stay solid,
opaque and culled exactly as before; no mask/blend/depth sorting is introduced.
One entity/mesh per nonempty streamed chunk remains. Dirty connected publication,
local version validation and normal asset deletion are unchanged. The retained
terrain material holds the shared image even when all visible chunks are empty.

Terrain buffers contain position f32×3, normal f32×3, UV f32×2 and light RGBA
f32×4 (**48 bytes/vertex**, 32 before Slice10); indices remain u32. Four-vertex
quad+six indices is 216 bytes, previously152: +42.1%, excluding driver alignment
and retained CPU copies. The built-in COLOR/material path avoids custom shaders
and keeps one chunk mesh/material. Packed light is a future measured option.
Atlas payload stays 32,768 GPU bytes and 32,768 CPU bytes plus driver overhead.

The crosshair, outline and save bitmap HUD retain separate materials/colors.
The nine-slot hotbar and 27-slot inventory stay independently unlit/readable. Physics, DDA and placement safety
use world data, not texture pixels. Floating origin changes only entity transforms,
so UVs/mesh buffers remain identical at near and far world positions.

## Validation

Core tests cover all material faces, exact UV footprints/orientation, unchanged
CCW geometry/indices and mixed-material culling across signed X/Y/Z chunk seams.
Client tests inspect sRGB/sampler/mips/opaque pixels, every extruded gutter/corner,
distinct tiles, actual 48-byte Bevy layout with light colors, shared Image/material residency during
empty/refill/edit/eviction, far rebase UV stability and existing save/restart paths.
CI verifies JSON → PNG/RGBA reproduction on both desktop platforms. Measurements
and actual software/native coverage are recorded in [VALIDATION](VALIDATION.md).

## Slice 9 inventory icons

Inventory samples grass-side, dirt, stone, bark and leaves directly from the same
embedded atlas with nearest sampling. No terrain tiles, UV contracts, world IDs,
meshing or image assets change. Two retained unlit UI materials (flat and atlas)
serve four batches; no per-item materials or per-frame images are allocated.
Original UI geometry/glyph tables and counts are specified in INVENTORY.

## Lighting samples and retro policy

The seven tiles/UV orientations and nearest LOD0 sampler are unchanged. All four
vertices of each face share one neutral multiplier sampled in adjacent Air, so
there is no smooth interpolation across shadow corners or AO. LIGHTING specifies
integer propagation and the small dark floor. No new game textures, per-block
materials/lights, extra terrain draw calls or per-frame atlas resources are added.

## Slice 16 terrain repetition

Terrain now uses block-repeat UV0 plus constant atlas-origin UV1 and an embedded
terrain-only WESL material. Inventory and viewmodel keep the original UV scheme.
See [greedy meshing](GREEDY_MESHING.md) for filtering, layout and validation.

## Slice 20 atlas slot7

The formerly diagnostic spare slot now contains an original torch tile: four
rows of gold/amber flame and twelve rows of wood grain. All previous slot pixels,
UVs, 128×64 dimensions, gutters, nearest sampler and opaque material stay exact.
The non-cube model samples the shaft/tip subrectangles; the inventory icon draws
two narrow rectangles from that tile rather than a full square. Flame vertices
remain at full brightness in darkness, with ordinary terrain fog still applied.
No alpha pass, PNG runtime dependency, per-torch material or light entity is added.
Provenance: assets/terrain/README.md. The standard-library screenshot helper
losslessly encodes PPM captures; it does not change the game image dependency set.
