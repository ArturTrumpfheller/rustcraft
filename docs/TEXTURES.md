# Terrain texture contract (Slice 8)

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

One opaque, white, fully rough `StandardMaterial` references that Image. Existing
lighting/exposure remain, so faces retain directional shading. Leaves stay solid,
opaque and culled exactly as before; no mask/blend/depth sorting is introduced.
One entity/mesh per nonempty streamed chunk remains. Dirty connected publication,
local version validation and normal asset deletion are unchanged. The retained
terrain material holds the shared image even when all visible chunks are empty.

Terrain buffers contain position f32×3, normal f32×3 and UV f32×2 (**32 bytes per
vertex**, formerly 40 with RGBA colors); indices remain u32. For this quad mesh,
combined vertex+index bytes decrease 17.39%; unchanged padding/alignment and driver
overhead are separate. Atlas payload adds 32,768 GPU bytes and 32,768 retained CPU
bytes plus allocator/renderer overhead. No extra per-face draw calls are needed.

The crosshair, outline, original five colored selection swatches and save bitmap
HUD keep their own untextured materials/colors. Physics, DDA and placement safety
use world data, not texture pixels. Floating origin changes only entity transforms,
so UVs/mesh buffers remain identical at near and far world positions.

## Validation

Core tests cover all material faces, exact UV footprints/orientation, unchanged
CCW geometry/indices and mixed-material culling across signed X/Y/Z chunk seams.
Client tests inspect sRGB/sampler/mips/opaque pixels, every extruded gutter/corner,
distinct tiles, actual 32-byte Bevy layout, shared Image/material residency during
empty/refill/edit/eviction, far rebase UV stability and existing save/restart paths.
CI verifies JSON → PNG/RGBA reproduction on both desktop platforms. Measurements
and actual software/native coverage are recorded in [VALIDATION](VALIDATION.md).

## Slice 9 inventory icons

Inventory samples grass-side, dirt, stone, bark and leaves directly from the same
embedded atlas with nearest sampling. No terrain tiles, UV contracts, world IDs,
meshing or image assets change. Two retained unlit UI materials (flat and atlas)
serve four batches; no per-item materials or per-frame images are allocated.
Original UI geometry/glyph tables and counts are specified in INVENTORY.
