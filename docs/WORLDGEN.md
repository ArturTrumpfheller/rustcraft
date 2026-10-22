# World generation — frozen v1/v2 and local v3

## Current generator v3 (v4 regional worlds)

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
textures, biomes, water, ore systems, growing trees or new lighting features.

## Historical frozen generators

The sections below document the retained older modes; new worlds use v3 above.


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
