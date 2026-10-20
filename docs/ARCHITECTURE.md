# RustCraft architecture (proposal)

Status: design intent; no crates have been implemented.

## Guiding rules

1. Keep the voxel world and simulation independent from the rendering engine where practical.
2. Prefer explicit, versioned data formats and registries for blocks and items.
3. Make terrain generation reproducible by seed and configuration.
4. Avoid a global mutable world; define clear ownership of chunks and job results.
5. Profile before optimizing; preserve correctness tests during refactors.

## Candidate Cargo workspace layout

```text
crates/
  voxel_core/    # block IDs, chunk storage, coordinates, registries
  worldgen/       # seeded terrain, caves, features
  simulation/     # game ticks, lighting, block updates, physics
  game/           # items, crafting, entities, progression
  networking/     # protocols and state replication (later)
client/           # Bevy presentation, rendering, input and UI
server/           # headless server (later)
assets/           # original or appropriately licensed assets
tests/            # integration and regression tests
```

Create modules only as needed: a simpler initial workspace is preferable to empty crates.

## World and renderer boundary

The world owns authoritative chunk/block data; the renderer builds or updates meshes from snapshots or change notifications. Keep chunk meshing separate from terrain generation. Queue chunk I/O and generation off the frame-critical path without introducing unsynchronized writes.

## Planned vertical slice

First deliver an interactable test world with deterministic chunk data, visible faces, first-person controls, solid block collision, block edits, and save/load. Establish benchmarks after this slice exists.

## Open questions for implementation

- Exact Bevy version and minimum GPU requirements
- Chunk dimensions and world-coordinate conventions
- Vertex format, atlas/texture array, transparent-block handling
- Save format and future migration strategy
- Tick rate and networking protocol
