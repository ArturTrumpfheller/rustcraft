# RustCraft development roadmap

This document tracks intended milestones, not promised delivery dates. Implement and test one vertical slice at a time.

## v0.1 — Classic-style prototype

Slice 3 adds precise targeting and in-memory block breaking/placement to the walking four-chunk scene. Full v0.1 is not complete. Slice 1 was tested on Void by the owner and passed Linux/Windows CI. Current platform coverage is recorded in `docs/VALIDATION.md`; new native play-tests are still needed. Indexing/meshing/physics tests exist; serialization and seeded generation are still absent.

- [x] Cargo workspace and pinned Bevy dependency
- [x] Launch client with a first-person walking player and interpolated camera
- [x] Initial closed block-ID enum and simple voxel materials (extensible registry deferred)
- [x] Chunk storage and visible-face meshing, including chunk-boundary culling
- [ ] Terrain generation from deterministic seed
- [x] Solid block collision, gravity and jumping (fixed 60-Hz AABB controller; boundary and tunneling regression tests)
- [x] Block targeting, breaking and placement (five-block DDA, validated player-safe edits and dirty neighbor remeshing)
- [ ] Hotbar and persistent world saves
- [ ] Tests for indexing, serialization and deterministic generation

**Exit criterion:** Start a world, walk around, place and break blocks, save, restart, and recover changes.

## v0.2 — Indev / Infdev-inspired foundations
- [ ] Streaming and loading/unloading chunks
- [ ] More interesting terrain and caves
- [ ] Basic light propagation and chunk remeshing
- [ ] Inventory and reusable item/block registry
- [ ] Performance profiling and memory budgets

## v0.3 — Alpha-inspired survival
- [ ] Health, damage and respawning
- [ ] Mining progress, tools and basic crafting
- [ ] Trees, ores and resource collection
- [ ] Day/night cycle and first mobs

## v0.4 — Beta-inspired systems
- [ ] Biomes and improved worldgen
- [ ] Farming and more entities
- [ ] Early redstone-like block interactions
- [ ] Structures and expanded survival

## v1.0+ — Expanded sandbox
- [ ] Complex simulation rules and redstone
- [ ] Additional dimensions and progression
- [ ] Multiplayer/server-authoritative simulation
- [ ] Plugin/modding interfaces and original gameplay ideas

## Engineering requirements
- Preserve deterministic simulation where feasible.
- Separate gameplay state from presentation and rendering.
- Add regression tests before refactoring fragile behavior.
- Track external licenses and asset provenance from the beginning.
- No unlicensed Minecraft textures, sounds or source code.
