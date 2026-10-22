# RustCraft architecture

Status: **Slice 8 original terrain atlas**. The owner accepted Slices 1–7 on
Void before this work. World, storage and simulation contracts are unchanged. See DECISIONS for earlier choices and VALIDATION for
actual measurements and remaining native acceptance.

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
and local output tokens (owner plus face-neighbors). Equal assignments are no-ops.

The radius remains 1–3 (default 2): at most 147 chunks, default 75, regardless of
world extent. Desired/queued sets are bounded. One terrain job is in flight.
Connected dirty components coalesce and publish together only if all local output
tokens are current. Near dirty geometry precedes new loads. New loads prefer the
nearest rings and the direction of motion inside a ring. Unrelated edits do not
invalidate useful work. Obsolete work cannot recreate evicted data/assets; valid
installs satisfy duplicate requests discovered during movement.

Mesh jobs copy only cached targets and available face-neighbors, and sample only
missing border cells. They never regenerate complete columns for edits. Interior
mesh-neighbor tests read the already-borrowed block vector; only chunk faces query
the world map. Emission order/material/triangle semantics remain unchanged.

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

Default resident block payload is 307,200 bytes; radius 3 caps it at 602,112 bytes.
New-column jobs retain the thin 15-chunk halo plus three returned copies (~72 KiB
peak block payload). A maximal connected mesh batch is bounded by radius targets
and face neighbors (up to 231 vectors at radius 3, plus one border copy), rather
than the entire large world. Mesh buffers/driver allocations are separate and
scale with visible surface complexity, not travel distance.

A conservative regional cache cap is 28 maps (desired halos, one running request,
and eight dirty regions); each map has at most 196,608 records. Job snapshots refer
only to their footprint. No visited-region list, global disk index scan or
all-world edit map is retained in v4. Dense regional COW, map destruction and
large connected bulk meshes are still bounded but require further native profiling.
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
before App startup. It does not persist player position; R still returns to origin.

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

## Slice 8 texture pipeline

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
