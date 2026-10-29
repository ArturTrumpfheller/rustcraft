# RustCraft development roadmap

## Slice 24 — Terrain Generation v2

- [x] Explicit generator4 for new worlds; frozen gen1–3 output and manifest4/gen3
- [x] Manifest5 generator identity through regions, catalog, player and redo recovery
- [x] Multi-scale lowlands/hills/connected ridges with smooth transitions and local plans
- [x] Existing vertical bounds, cave foundations, tree clearance and deterministic safe spawn
- [x] Independent goldens, multi-seed quality, signed seams and persistence regressions
- [ ] Native Void X11/Wayland / RX9060 XT and Windows GUI acceptance

Evidence, benchmarks and reproducible scenes: [Slice24](performance/slice24/README.md).
Separate follow-ups: Slice25 biomes, Slice26 caves v2, Slice27 natural water,
Slice28 vegetation/details; vertical expansion requires its own engineering slice.


This document tracks intended milestones, not promised delivery dates. Implement and test one vertical slice at a time.

Slice14 adds paged world selection, named seeded creation, separate bundles,
metadata rename, reversible archive and explicit validated legacy-copy import.
It reuses Slice13's save/cleanup barrier; see [WORLDS](WORLDS.md) and [MENUS](MENUS.md).
Slice15 adds persistent user settings; see [SETTINGS](SETTINGS.md) for exact options, limits and acceptance.

- [x] Title/pause settings, versioned configuration, FOV/mouse/keys, live radius and safe display preview
- [ ] Native Slice15 Void/Windows GPU, compositor and DPI acceptance

Slice19 adds F3 diagnostics, non-pausing inventory, contextual pause/settings
scrims and Java-like ground movement speeds. See [testing guide](DEBUG_GAMEPLAY.md).

- [x] Bounded retained F3 pixel overlay with authoritative signed coordinates
- [x] Separate fixed simulation from gameplay input; inventory falls/lands, Pause freezes
- [x] Contextual 45% pause/settings scrim with opaque title/catalog settings
- [x] Measured 4.317 / 5.612 / 1.295 blocks/s and normalized 30/60/144-FPS replay
- [ ] Native Slice19 Void X11/Wayland and Windows acceptance

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
- [x] Slice23 bounded downhill water routing and collision-checked one-block water exit (Issue #40)
- [ ] Slice23 native Void X11/Wayland, RX9060 XT performance and Windows GUI acceptance — see [WATER](WATER.md) and [measurements](performance/slice23/README.md)
- [x] First bounded streaming: configurable radius, one worker, cache/GPU eviction, safe pending barriers and persistent edits
- [x] First local large-world rollout with versioned regional storage and bounded caches
- [ ] Further coordinate expansion and opt-in legacy migration
- [x] Terrain Generation v2 (Slice24)
- [ ] Cave Generation v2 (Slice26)
- [x] Initial voxel skylight and bounded light/mesh updates
- [x] Emitted block light / persistent standing/wall torches, bounded worker updates and matched fixtures
- [x] Slice20 original lighting/rendering/persistence/performance accepted on native Void Linux
- [x] Precise torch selection, four wall mounts and item-presentation profiles
- [x] Slice20 precise selection/wall mounting/lighting/saving refinement accepted on native Void
- [x] Atomic inventory-backed support removal with all dependent torches; original block pose restored
- [ ] Final Slice20 support-removal/held-pose native acceptance — see [checklist](TORCHES.md)
- [x] Initial fixed inventory/item model
- [ ] General reusable item/block registry, tools and crafting
- [x] Initial profiling, matched baseline comparisons and explicit active/dirty memory budgets
- [x] Initial greedy terrain meshing with atlas repetition and exact light merge boundaries (native acceptance pending)
- [x] Vertex voxel AO, conservative greedy compatibility and diagonal edit invalidation (native acceptance pending)
- [ ] Native Slice17 Void X11/Wayland and Windows AO acceptance — see [checklist](AMBIENT_OCCLUSION.md)
- [x] Procedural daylight gradient and radius-dependent terrain fog with dark-cave protection
- [x] Slice18 owner acceptance on native Void Linux
- [ ] Remaining Slice18 Windows / explicit X11+Wayland sky/fog acceptance — see [checklist](SKY_FOG.md)
- [ ] Dense-region/bulk-edit profiling and further native GPU optimization

- [x] Slice22 event-driven water, regional persistence, translucent surfaces, bucket interactions and basic swimming
- [x] Slice22 torch washing, persistent bounded item pickup and inventory item catalog
- [ ] Native Slice22 Void X11/Wayland and Windows water acceptance — see [checklist](WATER.md)

## v0.3 — Alpha-inspired survival
- [ ] Health, damage and respawning
- [ ] Mining progress, tools and basic crafting
- [ ] Trees, ores and resource collection
- [x] Persistent fixed-step day/night, procedural sun/moon/stars and independent rendered sky/block light
- [x] Slice21 baseline accepted on native Void Linux / RX9060 XT (~240 FPS, VSync off; owner report)
- [ ] Slice21 sun/moon polish visual acceptance; explicit X11/Wayland and Windows acceptance — see [checklist](DAY_NIGHT.md)
- [ ] First mobs

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
