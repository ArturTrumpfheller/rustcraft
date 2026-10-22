# RustCraft development roadmap

This document tracks intended milestones, not promised delivery dates. Implement and test one vertical slice at a time.

## v0.1 — Classic-style prototype

Slice 6 adds bounded dynamic chunk caching/rendering, expanded 128×128 new worlds and v3 seed/version/edit-overlay persistence. Legacy snapshots remain exact finite worlds. Full v0.1 is not complete. The owner tested slices 1–5 on Void and their Linux/Windows CI passed. Current coverage is recorded in `docs/VALIDATION.md`; new native play-tests are still needed. Serialization/recovery and deterministic generation tests exist; infinite streaming and richer gameplay remain deferred.

- [x] Cargo workspace and pinned Bevy dependency
- [x] Launch client with a first-person walking player and interpolated camera
- [x] Initial closed block-ID enum and simple voxel materials (extensible registry deferred)
- [x] Chunk storage and visible-face meshing, including chunk-boundary culling
- [x] Terrain generation from deterministic seed (versioned finite hills/valleys, connected caves and simple trees)
- [x] Solid block collision, gravity and jumping (fixed 60-Hz AABB controller; boundary and tunneling regression tests)
- [x] Block targeting, breaking and placement (five-block DDA, validated player-safe edits and dirty neighbor remeshing)
- [x] Persistent world saves (versioned full snapshots, F5, dirty-only autosave, safe close and recoverable replacement)
- [ ] Full inventory/hotbar (five unlimited block-selection slots exist)
- [x] Tests for indexing and serialization, corruption and save-failure recovery
- [x] Tests for deterministic seeded generation, bounds, cave access, tree seams and spawn

**Exit criterion:** Start a world, walk around, place and break blocks, save, restart, and recover changes.

## v0.2 — Indev / Infdev-inspired foundations
- [x] First bounded streaming: configurable radius, one worker, cache/GPU eviction, safe pending barriers and persistent edits
- [ ] Infinite/region-based streaming and per-chunk disk storage
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
