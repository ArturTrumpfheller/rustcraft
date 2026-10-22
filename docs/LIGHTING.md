# Derived voxel skylight (Slice 10)

From accepted Slice-9 main `bc34e76`; MIT OR Apache-2.0 original implementation.
No saved generator, block/item ID, terrain art, world/player/journal format changes.

## Integer model and bounded domain

`voxel_core::lighting` computes deterministic integer skylight 0–15. Empty cells
with an unobstructed vertical column to Y32 receive 15 without downward loss.
All current solid blocks, including leaves, stop direct sun and propagation.
Indirect light travels through six-connected Air, losing one each step in every
direction. Only direct sky is a source; there are no emitters or sun/time changes.
The underside below Y-16 has level0; above Y32 is open sky. Finite horizontal
world edges are actual known Air. Pending streamed cells are opaque barriers.

A positive scattered contribution reaches at most 14 steps. A mesh samples the
Air immediately outside each face, so **15 blocks beyond the target X/Z bounds**
suffice. The full 48-block vertical span is always included: a roof can shadow
far below its own chunk. Full-height source columns cover the rectangle of target
columns plus one column on every horizontal side, including diagonals. Actual
light buffers use the tighter 15-block margin. This includes positive/negative
chunk and 64-block region seams and is independent of world extent/visit order.

Initialization scans direct sky, records each column's highest obstruction, then
seeds indirect propagation only across differing column roof heights. A bounded
FIFO floods the remaining dark Air. This gives the same result as seeding every
direct-sky cell, without processing the large open sky repeatedly. Unknown cells
are never assumed to be luminous Air. Streamed inputs are materialized from the
frozen generator plus **all regional edits in the required square footprint**;
I/O failure leaves old visuals/pending barriers and exposes the existing error.

## Worker, invalidation and publication

The existing single terrain worker owns streamed generation, light and meshing.
A process-local cache reuses pristine generated columns for missing neighbors;
each request applies its captured overlay to disposable copies. This cache never
contains player edits and can safely survive a stale result. It is pruned to the
current desired footprint plus one-column halo, at most 81 columns at radius3.
Resident authoritative blocks are copied rather than regenerated during editing.

Each successful edit records ordinary geometry targets separately from its
conservative full-height lighting targets (up to 3×3×3 chunks before clipping).
Local mesh dependency tokens advance for that same lighting footprint. Repeated
edits coalesce and connected targets publish as a unit; nearby dirty work precedes
new loads. The worker compares exact 18³ sample arrays against **last applied**
chunk lighting, remeshing only changed light or forced geometry. No probabilistic
hash can suppress a needed update. Discarded work never installs light arrays;
newer edits invalidate the whole result without clearing newer dirtiness. Old
complete meshes remain until replacements are ready; no clearing/recreating holes.

Resident light arrays are disposable (~5,832 bytes/chunk), pruned with eviction.
Maximum full-height light domain is 142×142×48 = 967,872 cells at radius3;
opaque/light arrays and the u32 flood queue are transient worker allocations.
Generated halo block cache is capped at 995,328 block bytes (81×3×4096). The
active chunk limit remains 147/default75. GPU mesh/entity removal is unchanged.
Regional retention includes current desired and running-job square footprints
plus at most eight pinned dirty maps; it never scales with travel history.

Legacy v1/v2 worlds remain fully resident/finite. Initial lighting is computed
once before their startup meshes; subsequent dirty batches use a bounded legacy
worker and a snapshot revision check, retaining dirty targets on stale completion.
All derived values are reconstructed after load; saves contain no lighting data.

## Presentation

Each face receives a flat neutral RGB multiplier from its adjacent Air light;
all four vertices agree. Fixed orientation factors are top1, bottom0.5, X0.75,
Z0.85. The light curve is `0.025 + 0.975 * 0.8^(15-level)` (minimum ~0.059 before
orientation), giving readable dark caves without making them uniformly bright.
There is no AO, smooth corner sampling or interpolation across unrelated faces.

One shared atlas StandardMaterial is unlit, so the integer-derived multiplier is
authoritative. Global ambient brightness is zero and the old directional light
is removed. Existing unlit inventory/hotbar/crosshair/save HUD remain independent
and readable. Atlas sampling/UVs, opacity, culling and floating origin are unchanged.
No shaders, textures, emitters, per-block materials or per-frame images added.

Bevy's standard COLOR attribute is f32×4: terrain stride increases 32→48 bytes,
with the same vertices/indices. Total quad vertex+index payload rises 152→216
bytes (**42.1%**), excluding driver alignment and retained CPU copies. This uses
the supported built-in material path; packed light attributes can be considered
later if measurements justify shader/pipeline complexity.

Tests, benchmarks, GUI/native coverage and limits are recorded in VALIDATION.
