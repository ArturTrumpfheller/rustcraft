# RustCraft development roadmap

This document tracks intended milestones, not promised delivery dates. Implement and test one vertical slice at a time.

## v0.1 — Classic-style prototype

Slice 8 adds original 16×16 art and one shared atlas/material with compact UV-bearing meshes. The owner accepted Slice 7 on Void. Slice 7 added local generator v3, a large 65,536×65,536×48 rollout, bounded regional edit caching/persistence and floating-origin rendering. Slices 1–7, including the native Void corrections, are accepted. Existing v1/v2/v3 worlds stay in their original modes; no automatic migration. Full v0.1 is not complete. Current measurements and remaining native Slice-8 acceptance are in VALIDATION.

- [x] Cargo workspace and pinned Bevy dependency
- [x] Launch client with a first-person walking player and interpolated camera
- [x] Initial closed block-ID enum and original 16×16 terrain atlas (extensible registry deferred)
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
- [x] First local large-world rollout with versioned regional storage and bounded caches
- [ ] Further coordinate expansion and opt-in legacy migration
- [ ] More interesting terrain and caves
- [ ] Basic light propagation and chunk remeshing
- [ ] Inventory and reusable item/block registry
- [x] Initial profiling, matched baseline comparisons and explicit active/dirty memory budgets
- [ ] Dense-region/bulk-edit profiling and further native GPU optimization

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
