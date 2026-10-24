# RustCraft development roadmap

This document tracks intended milestones, not promised delivery dates. Implement and test one vertical slice at a time.

Slice14 adds paged world selection, named seeded creation, separate bundles,
metadata rename, reversible archive and explicit validated legacy-copy import.
It reuses Slice13's save/cleanup barrier; see [WORLDS](WORLDS.md) and [MENUS](MENUS.md).
Slice15 adds persistent user settings; see [SETTINGS](SETTINGS.md) for exact options, limits and acceptance.

- [x] Title/pause settings, versioned configuration, FOV/mouse/keys, live radius and safe display preview
- [ ] Native Slice15 Void/Windows GPU, compositor and DPI acceptance

## v0.1 — Classic-style prototype

Slice 12 adds an original first-person arm, atlas-textured selected block and
subtle movement/click feedback. Slices1–11 are merged and accepted on Void.
Existing large-world, save, lighting, inventory and movement contracts remain
intact. See VIEWMODEL and VALIDATION for architecture, costs and acceptance.

- [x] Named multiple worlds, safe selection/creation/rename/archive and legacy-copy import
- [x] Cargo workspace and pinned Bevy dependency
- [x] Launch client with a first-person walking player and interpolated camera
- [x] Initial closed block-ID enum and original 16×16 terrain atlas (extensible registry deferred)
- [x] Chunk storage and visible-face meshing, including chunk-boundary culling
- [x] Terrain generation from deterministic seed (versioned finite hills/valleys, connected caves and simple trees)
- [x] Responsive walk/Ctrl sprint, Shift crouch, safe supported ledges and headroom checks
- [x] Solid block collision, gravity and jumping (fixed 60-Hz AABB controller; boundary and tunneling regression tests)
- [x] Block targeting, breaking and placement (five-block DDA, validated player-safe edits and dirty neighbor remeshing)
- [x] Original first-person hand/held hotbar block with retained models and bounded animation
- [x] Persistent world saves (versioned full snapshots, F5, dirty-only autosave, safe close and recoverable replacement)
- [x] 9-slot hotbar + 27-slot inventory, stacks 64, transfers, block collection/consumption and paired persistence
- [x] Tests for indexing and serialization, corruption and save-failure recovery
- [x] Tests for deterministic seeded generation, bounds, cave access, tree seams and spawn

**Exit criterion:** Start a world, walk around, place and break blocks, save, restart, and recover changes.

## v0.2 — Indev / Infdev-inspired foundations
- [x] First bounded streaming: configurable radius, one worker, cache/GPU eviction, safe pending barriers and persistent edits
- [x] First local large-world rollout with versioned regional storage and bounded caches
- [ ] Further coordinate expansion and opt-in legacy migration
- [ ] More interesting terrain and caves
- [x] Initial voxel skylight and bounded light/mesh updates
- [ ] Emitted block light / torches
- [x] Initial fixed inventory/item model
- [ ] General reusable item/block registry, tools and crafting
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
