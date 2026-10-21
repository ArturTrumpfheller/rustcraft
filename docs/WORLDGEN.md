# Finite world generation — version 1

The implementation is original RustCraft code, MIT OR Apache-2.0, in `voxel_core::worldgen`. No Bevy, shared RNG, noise library or copied game assets are used. This is a finite first generator, not an infinite-world/biome system.

## Reproducibility contract

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

The seed environment variable is consulted only by the lazy new-world factory after confirming that neither primary nor backup exists. A loaded v1/v2 snapshot never invokes the generator or parses a seed override. New worlds are saved once even without edits. Actual generated blocks are authoritative thereafter; v2 does not store seed/generator metadata or reconstruct terrain. See [the save/upgrade contract](SAVE_FORMAT.md).

## Performance and next boundary

Generation runs once before App/window creation and logs seed, version, resident chunk count and elapsed time. Initial mesh construction runs once at Startup and logs elapsed time; normal frames only rebuild edited chunk faces as before. Generation allocates at most ~192 KiB of chunk block payload plus the height/tree plan, not a second full volume. Meshing, renderer buffers and save encoding have separate transient allocations. The save worker clones only when a save is requested; no generation or disk I/O runs in the render loop.

Measured debug/cloud timings and functional coverage are recorded in [VALIDATION.md](VALIDATION.md); they are not desktop GPU benchmarks. Larger worlds will need region/halo planning (especially trees), worker scheduling for generation/meshing, memory budgets and streaming. Keep global coordinate sampling deterministic when introducing those systems. Do not extend this finite cache or synchronous mesh startup into unbounded work.
