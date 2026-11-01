# Slice 30: integration stabilization

Generator **9 remains the default and all v1–v9 procedural output is frozen**.
The current architecture is retained: coordinate-only planning, authoritative
regional overlays (including Air), one bounded terrain worker and three optional
chunk surfaces. No save migration, new generator or streaming redesign is added.

A bounded read-only audit and consolidated integration tests now exercise seeds
0/42/2026 at 45 fixed locations, signed and extreme coordinates, full-height
point/sampler/chunk parity, dry vegetation support, crown anchors, quiet natural
water and regeneration. Adjacent-region lifecycle tests preserve edits, inventory
and identity through save, eviction, reverse loading and restart. Existing journal
fault injection remains authoritative and now covers natural-water buckets in
both v8 and v9. A v9 shoreline ECS regression reclaims all three surfaces before
recreating them after eviction.

The halo-volume diagnostic now uses the saved generator's actual height, rather
than reporting three slabs for a sixteen-slab column. It counts logical generated
volume, including compact descriptors, not executed point queries. Release AO
checks permit only two positive-f32 ULP of brightness rounding and exact alpha;
integer light, visibility, topology and AO endpoint requirements remain exact.
Linux and Windows CI explicitly run these optimized AO regressions.

[Audit, matched CPU baseline, bounds, limitations and acceptance](performance/slice30/README.md).

---

# Slice 29: bounded vegetation and chunk cutouts

Generator9 layers local species/plant plans over unchanged v8 geology and water.
Candidate roots share bounded hydrology/cave/ground caches; plants reuse column
climate/material data. Plans are sampled consistently across signed chunk halos,
then rasterized off-thread without loaded-neighbor dependencies.

A third optional chunk mesh batches two crossed quads per plant, sharing the
expanded original atlas, terrain shader and daylight/fog with a retained mask
material. Terrain stays opaque; water retains its separate blended path. Full
publication, invalidation, asset accounting, rebasing, eviction and teardown cover
all three surfaces; water-only jobs preserve plants. No entity per generated plant.
The existing support/inventory and fluid/drop transactions cover new decoration,
with unchanged save envelopes and generator identities. [Full contract](VEGETATION.md).

---

# Slice 28: procedural hydrology and quiet water installation

Generator8 composes bounded `worldgen::hydrology` surface plans with existing biome
columns, supported tree anchors and v7 cave primitives. The18x18 sampled column
uses an additional20x20 water ring to prove immediate wet-bank caps. Independent
point queries and chunk rasterization share the same geography and cave-water
membership. Generation is seed/version/absolute-coordinate based; no residency,
RNG stream, traversal order or world-sized map participates.

A per-generator64-entry FIFO cache holds immutable cave-pool cells. Each candidate
is bounded to8192 cells in an81x81 box below source level-52; candidates reaching
a box edge are rejected. Metadata/cache construction is CPU-only and remains on
the existing worker. Cached voxels and saved overlays retain their current
ownership, stale-result rejection and dirty-region capacity checks.

Generated source slabs compact to uniform chunks. `wake_installed_fluids` tests
actual downstream/side consumers before enqueuing a source's dependencies;
unchanged source interiors and their dry upper surfaces remain asleep. Flowing
states and changed/deferred boundaries retain ordinary lifecycle wakeups. Queue
capacity/budget are unchanged, and water never generates/loads missing chunks.

Uniform enclosed water slabs need no geometry or retained light cache. The
existing separate water mesher culls internal faces and delays corner sampling
until a visible face needs it. Terrain meshes/materials, lighting transmission,
collision, buckets and underwater gameplay use their existing paths. Saved Air,
construction and fluid edits override the generated baseline; only these changes
are persisted through manifest5 / region1 and the existing session journal.

[Hydrology contract](WORLDGEN.md), [source semantics](WATER.md), [measurements,
images, limitations and native acceptance](performance/slice28/README.md).

---

# RustCraft architecture

## Slice 27: bounded three-dimensional caves

New worlds select generator7; generators1–6 retain exact output and bounds.
`worldgen::caves` plans jittered local systems with Q4 swept curved branches,
lobed ellipsoidal chambers, descending room connections and actual chamber-to-
surface entrances. A broad admission field leaves quiet solid regions. At most
a4×4 candidate-cell neighborhood supplies a column halo; conservative primitive
bounds restrict rasterization. Point queries use an allocation-free visitor.
Metric coefficients are computed once per primitive, with only integer math.

The existing v5/v6 height/material/tree stages and16-descriptor column worker
remain. V7 deep chunks begin uniform Stone and materialize only for actual cave
cells; upper sky remains uniform Air. Actual mixed source descriptors determine
the existing sparse light span, so deep cave surfaces/torches use the established
lighting, greedy/AO, invalidation and streaming paths. Manifest5, region1, player
bindings, record limits and journal recovery add supported identity7 without
changing envelopes. No world migration or new block/item IDs are introduced.
[Algorithm, roof/foundation and spawn rules](WORLDGEN.md);
[quality/performance, genuine captures and native acceptance](performance/slice27/README.md).

## Slice 26: versioned vertical worlds

New regional worlds select generator6 with Y[-64,192); all historical identities
retain Y[-16,32). `bounds::WorldHeight` supplies generation, streaming, validation,
lighting, collision/recovery and persistence limits. Ready uniform Air/Stone
chunks are allocation-free descriptors; pending descriptors remain solid barriers.
Mixed chunks materialize on demand and empty again without losing readiness.
One existing async column worker installs16 descriptors, evaluates only three
surface slabs, and skips empty/enclosed geometry and empty light caches. V6
lighting trims only proven opaque bottom / known-Air upper planes, preserving
exact sky/torch floods. Existing manifests/region/player/journal envelopes and
record budgets remain unchanged. Historical architecture sections below describe
their original slices; this section supersedes their fixed-height statements.
See [vertical contracts, budgets and native checklist](VERTICAL_WORLD.md) and
[measurements](performance/slice26/README.md).

## Slice 24 terrain and identity design

New worlds use local generator4 / manifest5; existing manifest4 remains frozen
at generator3. `RegionIdentity { seed, generator, nonce }` travels through region
reads/writes, world snapshots and the existing player/redo-journal binding. V5 is
an explicit supported-generator envelope using the existing64-byte layout; region
format1 already contains the generator field. Validation stays strict. Future
local versions extend an explicit supported list and algorithm branch, not a new
storage architecture. No saved world is implicitly converted.

`worldgen::terrain` separates v4 geographical shape from the shared, version-gated
local cave/material/tree stages. Large384/512-block warp/geography fields smoothly
blend buildable lowlands,96/40-block hills and connected skewed ridges. Q10 integer
math, bounded18×18 column plans and world-coordinate trees retain order-independent
point/chunk output. Frozen gen1–3 goldens remain unchanged. V4 tree placement checks
slope, support and crown clearance. Existing edited-block spawn/respawn needs no
physics or world mutation.

Y[-16,32), three-chunk streaming/light/mesh/save bounds, the single terrain worker,
cache/resource ownership, dirty-region authority and failure barriers stay intact.
No map-sized planning, frame-loop noise evaluation or new dependency is added.
See [WORLDGEN](WORLDGEN.md) for the vertical audit, later-slice boundaries and
separate height-expansion proposal; [measurements and acceptance](performance/slice24/README.md)
for actual costs and visual evidence.


## Slice 23 fluid behavior

`voxel_core::fluid` retains synchronous event-driven relaxation and selects
horizontal supply with a four-step, remaining-level-bounded BFS (at most41
visited cells). A transient byte-per-cell cache reuses routes; level changes
invalidate individual entries, terrain/source edits invalidate a radius-four
diamond on the edit/upper plane, and chunk lifecycle changes cover the same
expanded footprint including corners. Sources and saves keep their Slice22 IDs
and meanings. Worker meshing, fluid budget/backpressure and rendered water are
unchanged. `physics::Player` adds a collision-checked one-shot ballistic exit
only for Jump toward a nearby clear one-block ledge; normal movement is intact.
See [WATER](WATER.md) for contracts and [performance](performance/slice23/README.md)
for matched baseline measurements and native acceptance still required.

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

For current extended-height generators6–9, the conservative dense voxel payload
is25×16×4096 =1,638,400 bytes at default radius2 and49×16×4096 =3,211,264 bytes at
radius3 (one-byte in-memory Blocks). An81-column worker cache or source can hold
1,296 descriptors /5,308,416 dense voxel bytes each; actual uniform descriptors
allocate no voxel array. Exact applied light samples cap at784×5,832 =4,572,288
bytes. A maximal light solve is bounded to142×142×256 =5,161,984 cells per channel;
solid masks, flood queues, maps, mesh and driver storage are additional. Source,
pristine cache and main-thread storage are separate owners. These are conservative
payload bounds, not measured RSS. See Slice30 for representative measured scenes.

The following smaller numerical bounds describe historical generators1–5:

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

## Slice 18 daylight atmosphere

`client::atmosphere` centralizes scene-linear colors, radius-derived distances,
a retained clip-space background triangle and SkyMaterial. A shared embedded
WESL module supplies identical sky radiance to sky and terrain before Bevy's
existing tonemapping. The terrain extension adds one 48-byte uniform; nearest
atlas sampling, AO/skylight COLOR and all core geometry remain unchanged.
Vertex brightness smoothly suppresses daylight fog in dark caves.

`scene` selects the active streaming radius (preferences for finite legacy worlds),
spawns a layer 0 sky per session and updates only changed uniforms in PostUpdate.
Sky rays discard translation; terrain distance subtracts the camera in the same
render-origin space. UI/menu/viewmodel layers keep their existing materials and
depth order. Session cleanup reclaims the mesh/entity/material; the shared shader
module remains global. No atmospheric world scan, worker, remesh, texture or
additional render pass is introduced. See [formulas, costs and evidence](SKY_FOG.md).

## Slice 19 diagnostics and simulation/input policy

`ControllerInput.simulation_active` permits the existing fixed 60-Hz physics tick;
`active` separately permits gameplay commands. Focused inventory ticks use neutral
`MovementInput`, allowing gravity, collision and braking without UI-driven movement,
jump, respawn or edits. The camera uses simulation activity for interpolation.
Pause/settings, focus loss and lifecycle finalization freeze physics. Modal-held
movement/sprint/sneak and jump keys require release before reuse; no synthetic
keyboard events are injected. Focus loss with inventory open enters Pause and
retains the inventory until explicit resume.

`debug_ui` owns one global retained mesh/material/surface and an isolated overlay
camera (order9, no color clear); the menu camera remains order10. Session cleanup
immediately hides/resets diagnostics, while global assets are reused across A/B/A.
F3 only acts in focused Playing outside inventory. Text/glyph geometry refresh at
5Hz plus toggle; a reusable buffer holds bounded diagnostics. World f64 feet,
Euclidean signed block/chunk/local coordinates and constant-time chunk/map lengths
are the sources. FPS uses valid real frame intervals with a 0.5s EMA; startup displays
unavailable data explicitly. No render-origin coordinate, save path, world scan,
new asset per frame, renderer feature or dependency is added.

## Slice 20 block light and non-cube blocks

`Block` separates collision, light opacity, cube geometry, targeting and emission.
Torch is noncolliding, transparent, targetable and emits14; `is_solid` remains a
collision compatibility alias. Placement checks Air occupancy separately. Single-
cell support validation in `World::set_block` rejects removing a torch's floor or mounting wall
until the torch is collected, keeping the existing inventory/save transaction.

The existing `Skylight` worker field now retains two independent scalar channels.
Skylight sampling through `level` stays unchanged; `block_level` reports artificial
light and `combined_level` returns their maximum. Both use the same bounded
opacity scan/FIFO and captured full-height neighborhood. Exact 18³ cached samples
pack both nibbles into the original byte, so resident light-cache memory and
worker publication/dependency rules remain unchanged. Nonzero artificial sources
allocate one additional temporary byte per domain cell; no-source jobs skip it.

Greedy terrain keys use combined integer brightness plus existing tile/AO keys.
Twelve-quad torch models share each chunk mesh/material; the tip has full vertex
brightness. Inventory and hand reuse slot7 without terrain entities or point lights.
Emission and independent sampling provide an extension point for future sources;
RGB channels and tinting are intentionally deferred. See TORCHES/LIGHTING.

### Slice20 selection and orientation refinement

The core shared `shape` module supplies static shaft/tip boxes plus an orthonormal
mount transform. DDA tests those boxes and continues through empty cell space;
meshing and client outline use the same transform. World block IDs7–10 encode
four wall mounts and map back to Torch item6. Support validation protects all
five attachments with authoritative overlay checks for pending neighbors.
Orientation affects geometry alone; emission/opacity/channel buffers and worker
revision/dirty/publication rules are unchanged. The Bevy selection mesh updates
only on shape changes. Independent viewmodel presentation profiles provide item
pose/scale/rotation/origin without changing motion or allocating each frame.

### Final Slice20 bounded support-removal transaction

`World::plan_break` captures at most six validated removals in stack-backed
`EditBatch`. `edit_inventory` stages all collected items, then `commit_break`
revalidates the same authority/region budget and publishes the whole batch with
one world/region revision. The commit path is separate from fallible single-cell
mutation, unioning every geometry/light token without cloning the world. Raw
`set_block` remains single-cell and conservatively protects attachments; gameplay
uses the batch API. The Bevy adapter invalidates all returned changes in the same
input system so background jobs coalesce and retain existing stale-result/atomic
publication guarantees. Existing coherent save snapshots and paired session redo
journals persist all regional Air edits plus inventory, without format changes.

## Slice 21 authoritative time and presentation

`World.time` owns an engine-independent u64 day/u32 fixed-step clock. The same
60-Hz gate as physics runs it during inventory and freezes it in menus/focus
loss. Clock mutation does not advance block/mesh revisions. The save worker
captures it with inventory in player v2 and the existing redo journal; time-only
saves leave world roots/regions intact. SaveSession tracks captured time
separately from edits and handles later ticks as still unsaved.

COLOR retains AO/directional sky and block brightness plus geometric sky exposure
in the existing56-byte vertex. Greedy keys preserve both light channels. The
shader uses max(sky*daylight,block), with fog gated by sky exposure. At most two
retained80-byte uniforms update per frame. One procedural fullscreen sky triangle
adds the world-relative sun/moon and stable sparse stars. No clock tick queries
blocks, invalidates light caches or schedules terrain work. Existing stale
rejection and atomic connected publication are unchanged. See [DAY_NIGHT](DAY_NIGHT.md)
for equations, compatibility, costs, screenshots and native acceptance.

## Slice 22 water

`voxel_core::fluid` owns the packed one-byte block extension, validated water
state and bounded event-driven batch relaxation. Sources and flowing levels are
persisted through the existing regional overlays/journal; generator versions are
frozen. Chunk install/eviction wakes/prunes only local requests. The client shares
physics' pause gate, runs water at5 Hz and coalesces water-only mesh dirtiness.
Water-only streamed work reuses exact applied light, avoiding solid greedy work
and light floods. Separate retained `WaterMesh`/`WaterSurface` assets implement
animated alpha-blended surfaces; the original terrain vertex layout is intact.
See [WATER](WATER.md) for bounds, save compatibility, movement, sorting limits,
measurements and native acceptance.

Slice22 completion keeps the same solver. `raycast::TargetMode` separates normal
blocks from source collection; bucket exchanges stage inventory before edits.
`Block::fluid_drop` admits all Torch orientations while preserving solid barriers.
`World.drops` indexes at most4096 stationary stacks by chunk/cell; local pickup
and separate session entities preserve ownership across streaming and restarts.
Nonempty lists use bounded player-v3 records in the same world/inventory redo
journal; empty lists and old reads retain v2/v1 compatibility. Item palette grants
are additive/atomic and legacy item7 conversion is explicit. Emission changes use
the existing full lighting path; ordinary water still reuses cached light.


## Slice25 biome generation

`worldgen::biome` provides engine-neutral Q10 climate, classification, v5 shape,
coherent surface choice and vegetation density. `worldgen::local` version-gates
these inputs and caches18² per-column materials alongside heights for all three
vertical chunks; a temporary20² grid reuses slope neighbors. The established
cave sampler, tree geometry, worker and bounded terrain cache remain shared.
Historical v1–v4 outputs remain pinned and are never reinterpreted. F3 samples
one seed/coordinate query at the existing bounded refresh rate.

New worlds select generator5 through CURRENT_LOCAL_GENERATOR, stored explicitly
in manifest v5; saved identities select their original generation path. Existing
player binding/catalog/journal logic derives the manifest version from the actual
identity and retains exact generator matching. Only supported-generator validation
is extended. No persistent derived biome map or new envelope is needed.

Sand20/SnowyGround21 use the ordinary solid-block rules and item10/11. The
creative palette, retained held models and face mappings include both. Three
original pixel tiles extend the single embedded atlas to128×96 (48KiB CPU/GPU
pixel payload each, excluding driver overhead). Terrain greedy UVs, inventory,
viewmodel and dropped torches use that same layout. No extra material, render
pass, simulation behavior, native dependency or persistent cache is added.
See [WORLDGEN](WORLDGEN.md) and [validation](performance/slice25/README.md).
