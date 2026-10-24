# RustCraft architecture

Status: **Slice14 world management**, based on main `50b2ae6`. The independent
`voxel_core::persistence::catalog` owns safe directory IDs, metadata, complete-copy
legacy import, validation/staging and archive. A catalog OS mutex serializes
mutations and managed opens; each bundle retains its own SaveStore lock.
`client::menu::worlds` owns paged original bitmap forms and one disk worker.
`session` loads only the selected existing catalog ID and reuses Slice13's final
save/worker/entity/asset barriers. See [WORLDS](WORLDS.md) for filesystem/security
semantics and [MENUS](MENUS.md) for lifecycle. Frozen game formats and generators,
lighting, movement and inventory rules remain unchanged.

## Slice 15 user preferences

`client::settings` owns bounded versioned user configuration and a short-lived
IO worker, outside SaveStore. `menu::settings` reuses the retained canvas with
drafts, key capture and timed display rollback. Apply installs validated keys,
camera base and radius through existing adapters; session setup retains the
user resource across worlds. See [SETTINGS](SETTINGS.md).

## Slice 7 design note (before implementation)

- New worlds use generator v3 and a v4 regional identity. The first supported
  extent is X/Z [-32768,32768), Y [-16,32): local sampling, no area-sized startup
  allocation. This is a tested large rollout; floating-origin rendering and a
  further format/extent decision are required before larger coordinate promises.
  Existing v1/v2 snapshots and v3/gen2 worlds never expand or migrate implicitly.
- Heights/tree candidates are computed from bounded local plans (coordinate hash,
  integer noise, nearby anchors). Old generator branches and goldens stay frozen.
- Region overlays cover 64×64×48 blocks, with computed/sharded disk paths and no
  startup global index scan. A constant checksummed world manifest pins seed,
  generator, extent and a world nonce. Region files validate that identity,
  coordinates, size, sorted records and CRC, including explicit Air.
- Resident physics/mesh caches stay radius-bounded. Region overlays are cached
  only for desired halos / running work; unsaved regions remain pinned in RAM,
  with an eight-dirty-region edit limit and save pressure rather than data loss.
  Clean distant overlays and local mesh tokens can be discarded. Copy-on-write
  is regional, not proportional to all edits ever made.
- One bounded terrain worker handles prioritized/coalesced meshes and new loads;
  region reads and generation are off-thread. Existing local output tokens and
  atomic connected dirty publication remain. Pending or failed loads are solid
  barriers, with visible errors and controlled retries. Respawn reloads normally.
- F5/autosave/final close snapshot only dirty regions and mark only successfully
  captured revisions clean. Region replacement reuses synced temp/backup/rename
  and the session lock. Files are individually atomic, not a cross-region
  transaction: a failed multi-region save keeps all dirty RAM state for retry.
  Manifest identity is committed before first region writes; missing primaries
  with backups/corruption never silently regenerate. No per-click journal.
- Measure accepted Slice-6 baseline versus local generation/meshing, distant
  travel/cache bounds, dirty latency, frame percentiles, CPU/RSS and corruption/
  partial-save recovery. Keep one worker unless measurements justify a pool;
  texture assets and dependency upgrades stay out of scope.

## Core and presentation

`voxel_core` owns integer coordinates, 16³ block vectors, deterministic generators,
meshing, f64 player physics, DDA targeting, validated mutation and persistence.
It has no Bevy types; its only external dependency remains crc32fast. `client`
adapts Bevy input/schedules, owns entities/GPU assets, save feedback and worker
scheduling. No new dependencies, renderer features or crates; the original embedded atlas is
shared by one terrain material. See TEXTURES for the core face/UV contract.

Y is up. Local cells are validated and coordinates split by Euclidean division,
including negatives. Material IDs remain Air=0, Grass=1, Dirt=2, Stone=3, Wood=4,
Leaves=5, independent of enum layout. Horizontal world bounds are mode-specific;
Y remains [-16,32). Legacy missing data is Air. In-volume streamed pending data
is a solid collision/ray barrier and cannot be edited. Outside bounds is known Air.

## World modes and local generation

- v1/v2 files: exact finite 64×64 full snapshots, fully resident, no generator.
- v3 files: frozen generator v2, 128×128 volume, canonical complete edit overlay.
- v4 manifests: generator v3, X/Z [-32768,32768), regional edit storage. This is
  a 65,536×65,536×48 supported rollout, not literally infinite coordinates.

Generator v1/v2 output is unchanged and pinned by historical goldens. Generator
v3 has no area-sized height/tree cache. A column sampler plans 18×18 heights and
nearby deterministic eight-block tree-grid candidates. Center chunks and face
halos use that same bounded plan. Integer noise/hash, periodic cave/entrance
sampling and canopy tests use world coordinates; neighbors and visit order do
not affect output. Point queries use the same rules. New goldens pin distant
positive/negative columns and seed extremes. Full-world generation is explicitly
disallowed for generator v3. See WORLDGEN for the frozen version contracts.

## Authority, residency and scheduling

`World` separates resident chunks from authoritative edits. v3 retains its
historical immutable global overlay. v4 caches complete per-region overlays as
independent Arcs. Dirty region snapshots share those Arcs; subsequent mutation
copies only that region. A change updates the resident block, global save revision
and local output tokens (geometry plus full-height bounded light dependencies). Equal assignments are no-ops.

The radius remains 1–3 (default 2): at most 147 chunks, default 75, regardless of
world extent. Desired/queued sets are bounded. One terrain job is in flight.
Connected dirty components coalesce and publish together only if all local output
tokens are current. Near dirty geometry precedes new loads. New loads prefer the
nearest rings and the direction of motion inside a ring. Unrelated edits do not
invalidate useful work. Obsolete work cannot recreate evicted data/assets; valid
installs satisfy duplicate requests discovered during movement.

Mesh jobs copy cached full-height source columns within the target rectangle plus
one-column square halo. Missing sources come from immutable generated-column
cache and captured edits, including diagonal regions. They compute bounded light,
compare exact applied samples, and mesh only changed light or forced geometry.
The cache is pristine generator output; it never substitutes for edited authority.
No cached resident column is regenerated on edits, and missing halos are reused.
All affected targets validate local light dependency tokens before any publication.

Regional reads happen on the terrain worker. Each request captures only its
required region footprint, preventing historical overlays from propagating from
one worker to the next. Successful payloads install absent regional cache entries
only; they never replace newer live edits. Clean distant overlays and mesh tokens
are pruned. Pending dirty regions remain authoritative even when their voxel and
GPU chunks unload. Eight dirty regions are the hard edit-backpressure limit;
four trigger save pressure. Save failures retain all dirty data, and further
edits in a ninth clean region are rejected instead of growing memory indefinitely.

Failed region reads keep safe barriers, expose `LOAD ERROR - F5 RETRY`, and use
bounded per-column retry deadlines. F5 retries promptly. Other eligible work can
continue; no silent backup fallback or procedural replacement of invalid data.

## Memory and lifetime bounds

Default resident block payload remains 307,200 bytes; radius3 caps it at 602,112.
New light inputs require full-height square halos instead of Slice8's thin faces.
The maximal worker source/cache footprint is 81 columns / 243 chunks / 995,328
block bytes each (cache and scratch are separate). Main-thread snapshots copy
only available source chunks. Applied exact light samples use 5,832 bytes per
resident chunk, at most 857,304 payload bytes. Light/flood scratch is bounded to
142×142×48 cells, not world extent; mesh/driver buffers remain separate.

Regional retention covers current desired and running-job square footprints plus
eight pinned dirty maps: conservative union cap40 maps (16+16+8), independent of
travel. Each map still caps at 196,608 records. There is no global disk index or
visited-region list. Dense regional COW, large light batches and mesh buffer
allocation remain measured optimization boundaries; no automatic radius expansion.

Bevy removes obsolete entities and Mesh assets; empty streamed chunks need no
GPU mesh. Legacy empty-handle behavior remains unchanged. Idle frames do no disk
I/O, generation or remeshing.

## Player, view and respawn

Physics remains a fixed 60-Hz f64 kinematic AABB controller, resolved Y/X/Z with
swept contact, wall sliding, gravity, grounded jumps and no auto-step. Clearance
uses the same 1e-8 open contact tolerance as collision, retaining the diagonal
corner fix. Actual block data is authoritative; meshes never drive physics.

Camera interpolation operates on previous/current world positions and publishes
world-space `ViewPose` for editing. Presentation subtracts a chunk-aligned f64
`RenderOrigin` before f32 conversion. Origin changes rebase all terrain entities
and the camera together before view/edit feedback, without changing simulation,
saved coordinates or ray origins. Highlights subtract the same origin; the HUD
stays camera-relative. This keeps precision at distant coordinates.

Spawn is not pinned. R/void requests the origin column, waits above the world
with physics/editing disabled while pending, then selects clear supported edited
ground. Both interpolation positions reset together. Spawn search is capped at
64 blocks rather than scanning the large world. No terrain is created for spawn.
An optional v4-only `RUSTCRAFT_START_CHUNK=x,z` prepares a distant column/region
during the Loading state. It does not persist player position; R still returns to origin.

## Regional persistence and failure behavior

The immutable v4 world manifest identifies seed, generator, supported extent and
world nonce. Computed, sharded 64×64×48 region paths contain full last-write overlays,
including explicit Air. There is no implicit migration from v1/v2/v3. File reads
validate lengths/counts, reserved fields, CRC, sorted unique positions, material
IDs, region coordinates and matching world identity. Unknown/corrupt files stop
loading that data without overwrite. See SAVE_FORMAT for exact bytes and recovery.

One save worker snapshots only dirty regions. It commits the manifest before first
region writes, validates old files, syncs temporary files, replaces backups and
atomically renames each regional primary. Unix also syncs directory entries.
Regions are individually atomic, not one cross-region transaction. Partial failure
may commit some regions but retains every dirty RAM revision and cancels close;
idempotent retries preserve older backups and repeat directory sync. Success
marks only captured revisions clean, leaving newer edits dirty. F5, 30-second
real-time autosave, pressure saves and final-close saving share this protocol.
Forced termination can still lose changes since successful saves; no per-click
journal or absolute power-loss guarantee is claimed. The session OS lock covers
both terrain reads and save workers.

## Diagnostics and next boundaries

Idle-transition logs report counts, queue peak, buffers, worker/install/update and
dirty/queue-to-install latency maxima, plus P50/P95/P99 frame intervals and mean
FPS over 128 samples. These include renderer/driver scheduling, not GPU timestamps.
VALIDATION records headless CPU/RSS and graphical comparisons. The measured local
job cost does not currently justify a pool; one worker avoids unneeded concurrency
and memory complexity. Larger radii, dense-region latency, asynchronous allocator
cleanup, multi-job budgets, further coordinate ranges and opt-in migrations need
separate measured work. The original Slice-8 atlas is documented in TEXTURES; additional lighting, mipmap
strategies and richer material systems remain separate work.

## Historical Slice 8 texture pipeline (lighting/stride superseded below)

`voxel_core::texture` maps solid blocks and axis normals to seven atlas slots,
without Bevy or persistence IDs. `ChunkMesh` carries local positions, normals,
UVs and indices. Original winding/face order/culling stay unchanged. A constant
quad table supplies UVs per visible face; there is no per-vertex atlas division,
image generation per chunk or new work in the idle render loop. Terrain colors
are replaced by UVs; UI/swatch colors continue using the original block palette.
The logical vertex stride is 32 bytes (was 40); index buffers are unchanged.

`client::terrain_texture` creates one 128×64 RGBA8 sRGB Image from embedded original
bytes and pins nearest min/mag/mipmap, LOD0, clamp addressing and no anisotropy.
`spawn_scene` creates one opaque rough StandardMaterial referencing that Image.
It remains retained across remesh, empty chunks, travel and far-origin rebasing;
chunk teardown removes only the chunk mesh/entity. GPU and CPU art pixels are
32 KiB each, excluding driver alignment/overhead. No PNG decoder or async asset
loading path is needed. Lighting/exposure and all bitmap UI remain unchanged.
Authoring source, padding/orientation and the mip aliasing tradeoff are in TEXTURES.

## Slice 9 inventory and player sessions

`voxel_core::inventory` owns fixed 36-slot storage, validated stacks, cursor,
selection, merge/split/swap/quick-transfer and staged world/inventory edits.
It imports no Bevy types. Failed/full/pending/body-overlap actions commit neither
side. There are no item entities, tools, crafting or extensible registry yet.
Five explicit item IDs map to the existing five block types.

`client::inventory_ui` handles modal input before view/physics and renders original
pixel geometry/glyphs in four retained meshes, two unlit materials and the existing
atlas image. Pointer movement updates cursor transforms only. No per-frame asset
allocation or terrain material changes. Hidden empty batches retain valid small
buffers rather than uploading zero-vertex meshes. Camera parenting preserves
floating-origin precision. Existing crosshair and target outline remain.

Production startup loads/replays player state before spawn preparation. The one
save worker captures world and inventory revisions together. A checksummed,
bounded journal records the intended root, dirty regions and player before any
world commit; replay completes it before gameplay. World-only SaveStore saving is
refused once player state is initialized. Successful old jobs clear only captured
revisions. Inventory-only changes trigger F5/autosave/final close too.

This extends the historical per-file world write protocol above; it does not
change any world byte format or generator, and does not make filesystem writes
simultaneous. Full closed-directory backups are necessary; older clients must
use isolated copies. See INVENTORY for initialization failure and recovery limits.

## Slice 10 lighting integration

`voxel_core::lighting` implements integer direct sky plus six-neighbor attenuation,
including full vertical occlusion, exact disposable chunk samples and a bounded
15-block lateral margin. Opaque leaves use ordinary solid rules. No saved derived
fields or new IDs. The stream worker completes required region inputs before
calculating light; failures retain old meshes or pending barriers and use existing
visible retry feedback. Initial streamed meshes wait for that authoritative halo.

Geometry and lighting dirtiness are separate. A successful batch updates exact
applied light arrays and changed meshes together; stale results never install
light arrays. Generated pristine halo cache can safely be reused even after a
stale job, then prunes to current desired + halo. Legacy finite edits also use an
asynchronous light/mesh worker with snapshot revision validation; initial legacy
lighting is computed once before startup meshes. Full legacy bounds stay fixed.

One shared unlit StandardMaterial modulates the original atlas using f32 RGBA
face colors (48-byte vertex, 42.1% additional quad payload). Global ambient is
zero and the old directional light is removed. Fixed top/bottom/X/Z factors and
a small cave floor replace global PBR washout. UI materials remain separately
unlit/readable; UVs, culling, origins and resource ownership remain intact.
LIGHTING describes attenuation, no-AO policy, limits and reconstruction.

## Slice 11 controller integration

`Player` owns actual crouched posture; `PhysicsConfig` owns both body/eye heights
and all speed/acceleration constants. Support and standing clearance query actual
world blocks. Grounded crouch caps momentum, clips horizontal travel to supported
positions, and leaves jump/fall/gravity and swept collision order intact.

`client::controls::MovementBindings` maps keys to core commands. The fixed adapter
records previous posture alongside previous feet; camera interpolation uses both.
`ViewPose` retains displayed feet plus conservative transition posture so edits
protect actual and displayed bodies independently of eye-height changes. Existing
modal/focus/save pauses remain in the same schedules. `MovementGestures` records
forward press/release timing using real elapsed time and gates held jump across
transitions. The fixed adapter ORs held requests with one consumed pending tap;
core support alone decides when a launch is legal. Sprint launches add a bounded
run-up bonus; sprint air control preserves it without increasing the cap.
`SprintFov` tracks the perspective base, eases a9% relative gain and restores it
without changing aspect/frustum settings or creating UI/GPU resources.

## Slice 12 viewmodel

`client::viewmodel` owns six immutable meshes, two materials, one original skin
Image and one overlay view. Only a single cuboid arm OR a held block is visible.
A fitted hand lens is compensated in camera X/Y; uniform model depth scaling
keeps all swing vertices behind UI without changing their projected silhouettes.
The main world camera remains authoritative for view/rays. An identity overlay
child copies perspective projection, loads color and clears depth; layer1 draws
the original arm/block behind existing near-camera UI. World/target stay layer0.
Distance-driven animation reads displayed feet and actual vertical/ground/posture
state. Interaction's existing accepted input gate increments a bounded-action
serial; the viewmodel reads selection after edits, without parallel inventory
or save state. Hidden/pause/capture/reset consumes animation history safely.
No per-frame mesh/material generation; resource costs/limits are in VIEWMODEL.

## Slice 16 terrain geometry

`voxel_core::mesh` greedily merges visible coplanar faces sharing tile/skylight.
The client adapts its engine-neutral repeated UV and tile-origin buffers to a
terrain-only embedded material; UI/viewmodel retain StandardMaterial. The worker
and persistence contracts remain unchanged. Session asset cleanup includes the
terrain material type. See [algorithm and measurements](GREEDY_MESHING.md).

## Slice 17 ambient occlusion

`voxel_core::mesh::ao` samples a fixed one-cell occupancy halo and modulates
terrain COLOR without changing skylight values. Greedy keys include four AO
counts packed into one byte; constant and exactly matching affine fields merge.
Non-affine unit quads choose their diagonal from corner brightness. The same
halo serves visibility; a fixed color table avoids repeated modulation math. `BlockChange::affected_chunks` now includes
edge/corner neighbors, forcing AO rebuilds even when exact light samples match.
Existing source-column/region halos, local tokens, stale rejection and worker
publication rules already cover those dependencies. No new material, pass,
vertex attribute or persistent state. See [algorithm and evidence](AMBIENT_OCCLUSION.md).
