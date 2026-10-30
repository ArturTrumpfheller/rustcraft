# Vegetation and world details — Slice 29

New worlds select **generator9**. Generators1–8 retain their exact block output,
including historical Oak trees, caves and natural water. V9 changes vegetation
only: the v8 biome/climate fields, final basin beds, coasts, water initialization,
cave primitives and Y[-64,192) remain. Existing manifest5/region1, player and
session-journal envelopes already carry the necessary identity and IDs. There is
no migration, refill or vegetation insertion into historical worlds.

## Local generation

A v9 column discovers nearby tree anchors in a fixed neighborhood of eight-block
candidate cells. Each candidate has full-cell position jitter, independent salted
species/height/shape choices, and a continuous moisture/cold/highland density
multiplied by a48-block grove/clearing field. This is bounded coordinate sampling,
not a shared RNG or a dependency on loaded neighbors. Negative coordinates use
Euclidean cells. Candidate cells are an admission device, not a tree every eight
blocks: most are rejected and groves have actual gaps.

Oak retains the original Wood/Leaves and pixels; new Oak trunks vary4–6 blocks
with a rounded three-layer crown. Birch trunks vary5–7, with pale horizontally
marked bark, pale end grain and a compact four-layer crown. Spruce trunks vary7–10
with dark bark/needles, a tall central stem and alternating tapered foliage tiers.
Every individual crown is connected to its trunk. Point queries and chunk
rasterization share the same plans and trunk-before-leaf precedence.

Roots must stand on dry Grass/Snowy Ground above the existing sea datum. A5×5
footprint must have at most one block of height variation and no real surface
cave opening or wet ground. All crowns clear the final terrain; anchors also
respect finite edges, the world ceiling and the protected spawn clearing.
Candidate root checks share one bounded hydrology/cave plan and coordinate caches;
plans are never reconstructed for every output voxel. The actual v8 hydrology
algorithm is unchanged.

Plants are planned once for the18×18 column/halo, using the already computed
climate and final supported surface.18/28/48-block coherent fields define grass,
flower patches and flower-color regions, with salted per-cell thinning inside
those patches. They occupy only dry surface Air. They cannot appear in basins,
water sources, underground, on a carved opening, inside a trunk or beyond bounds.
Plant/Tree plans die with the sampler; the existing bounded terrain cache retains
pristine chunks, not a world-sized vegetation registry. Uniform Air/Stone/Water
slabs retain their compact descriptors; a v9 actual-detail ceiling skips empty sky.

| Region | Distribution |
| --- | --- |
| Plains | Open building space, grass patches, occasional red/yellow flower patches, scattered Oak/Birch; cold transitions may include Spruce |
| Forest | Moisture-weighted Oak/Birch groves, broad clearings, ferns and grass, occasional flowers; colder/elevated transitions admit Spruce |
| Desert | Sparse brown Dead Bushes on suitable exposed ground; no ordinary forests |
| Snowy Plains | Sparse Spruce groups; limited ferns, no dense green grass on snow |
| Rocky Highlands | Sparse Spruce on suitable gentle Grass/Snowy Ground; limited ferns, exposed Stone dominates |

## Permanent registry additions

Block0–21 and item1–11 retain their existing meanings, including legacy item7,
Empty Bucket8 and Water Bucket9. New IDs are append-only and independent of atlas
slots. Every new item stacks to64 and is in the two-row creative item palette.

| Block/item | Block ID | Item ID | Atlas tile(s), zero-based |
| --- | ---: | ---: | --- |
| Birch Wood |22|12|11 bark,12 end grain |
| Birch Leaves |23|13|13 |
| Spruce Wood |24|14|14 bark,15 end grain |
| Spruce Leaves |25|15|16 |
| Tall Grass |26|16|17 |
| Fern |27|17|18 |
| Red Flower |28|18|19 |
| Yellow Flower |29|19|20 |
| Dead Bush |30|20|21 |

Wood end grain follows existing ±Y face mapping. All leaves remain solid opaque
cubes. New materials can be placed in old worlds without changing the saved
generator. Older clients reject unknown IDs; use complete closed-directory copies
when testing old binaries. Unknown block31+/item21+ and generator10+ fail closed.
Legacy snapshot-v1's accepted material set remains frozen.

## Cutout rendering

The shared original atlas is now **128×192 RGBA8 sRGB**,22 authored16×16 tiles,
32-pixel pitch,8-pixel extrusion, nearest filtering, LOD0 and no mip chain. Existing
11 tile pixels/slots are unchanged. The six new tree tiles are opaque. Only the
five plant tiles use binary transparent pixels. JSON accepts either RGB or RGBA
palette entries; `python3 tools/generate_textures.py --check` checks every artifact.
All art is independently authored RustCraft art under MIT OR Apache-2.0.

The existing opaque greedy mesh/material and separate blended water mesh remain.
Each small plant adds **two crossed quads**,8 vertices/12 indices, to an optional
**chunk-level** cutout mesh. It shares the terrain vertex contract, atlas and
terrain shader with a retained `AlphaMode::Mask(0.5)` material. The shader discards
transparent texels, the material disables back-face culling and enables double-sided
visibility. Plants write depth through the ordinary mask phase, without water-style
blending or per-plant sorting. Existing leaves do not become transparent.

Separate sky/block channels and geometric sky exposure use the existing daylight
and fog code; plants do not occlude sky or contribute cube AO. They sample light
at their actual cell. Full worker results publish all chunk surfaces coherently;
water-only results preserve vegetation. Water replacing a plant forces a full
geometry update even though opacity is unchanged. Removal, eviction and session
teardown reclaim the plant entity and mesh asset. Empty plant vectors allocate
no vertex/index storage. At most three mesh entities exist per rendered chunk.

Inventory icons sample the same transparent tile with a mask material. Held plants
are thin, double-sided sprites with a dedicated modest pose; they are never cubes.
Four new held tree blocks use normal textured cubes. No engine feature, runtime
image decoder, dependency, per-frame generation or plant entity is added.

## Interactions, support and persistence

`Block` provides explicit plant shape/height, collision, targetability, support,
fluid replacement and item-drop policies. All five plants are non-solid,
targetable with a smaller selection box, non-opaque and non-emitting. Grass and
flowers require Grass/Dirt; Fern also accepts Snowy Ground; Dead Bush accepts
Sand/Dirt/Stone. Placement rejects water-filled cells and missing/invalid support.

The existing bounded support transaction now handles plants as well as torches:
one supporting cell and at most five attachments. Mining collects the complete
item set or refuses the entire action when inventory/region capacity is full.
A direct single-cell edit cannot silently leave a plant floating. Pending generated
plants across a vertical chunk boundary are resolved from the versioned baseline;
unknown dependencies defer editing. Existing torch errors/rules remain intact.

Water replaces plants through the existing bounded fluid-decoration drop path.
Each washed plant creates one collectible persistent item, with no duplicate on a
no-op/retry. Full drop capacity defers displacement until capacity is available,
as for torches; items are not silently lost. Plant item meshes are shared per type,
and dropped-item entities remain within the existing4096-stack bound. No new
plant lifecycle/growth simulation exists.

Generated vegetation has no edit records. Saved Air, builds and placed plants
always override generation, including after eviction/regeneration. Removal,
inventory and washed drops travel through the existing coordinated save snapshot
and redo journal. Interruption tests cover session-intent/world/player stages in
worlds with generators3–9; removed generated plants remain removed. No journal,
checksum, record budget or save envelope changes.

## Validation and limits

[Measurements, genuine captures, exact cameras and Void/Fish acceptance](performance/slice29/README.md).
Focused tests cover species topology/height variation, signed seams, reverse and
parallel generation, point/halo/chunk agreement, v8 geology preservation, final
plant support, collision/targeting/collection, water displacement, registry/save
round trips and interrupted recovery. Historical generator goldens are retained.

Plants are static one-cell decoration. Leaves are deliberately opaque; no decay,
saplings, growth, crafting or wood conversion is implemented. Tree/plant ecology,
more advanced terrain integration and distant sprite minification belong to later
work. Nearest LOD0 sprites may shimmer at distance. Native Void X11/Wayland and
Windows/Radeon play-testing remains an owner acceptance step.
