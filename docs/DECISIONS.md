# RustCraft — project decisions

This document summarizes choices already made by the project owner and is intended to survive across Codex sessions.

## Confirmed direction

- **Project:** RustCraft, an independent Minecraft-inspired voxel sandbox.
- **Initial scope:** Minecraft Classic-like experience as a starting point; gradually expand toward Infdev/Alpha/Beta/Release-inspired systems and eventually original ideas.
- **Visual and gameplay direction:** recognizable, authentic retro block-world feel, without intentionally recreating easily fixable historical bugs.
- **Language:** Rust rather than Java or C.
- **Engine:** Bevy is preferred for app/rendering/input; game simulation, voxel model and core rules should have a modular boundary.
- **Open-source assistance:** allowed; use appropriate, documented licenses and existing crates rather than writing every low-level facility from scratch.
- **Repository:** `ArturTrumpfheller/rustcraft`, made public by the project owner on 2026-10-21 to simplify cloning. Early-development source visibility does not imply a packaged public game release.
- **Own code/documents license:** MIT OR Apache-2.0. Third-party components and artwork need separate checks.
- **Implementation responsibility:** Codex will implement code with user review and testing. ChatGPT planning conversations do not automatically transfer into Codex sessions.
- **Development approach:** incremental, testable changes, preserve working baselines, support Git branches and pull requests.

## Priorities

1. Produce an actual playable and editable block world first.
2. Maintain sensible architecture so later gameplay systems do not require total rewrites.
3. Test, profile and iterate; do not claim parity with Minecraft before verifying behavior.
4. Keep a viable path to publishing a distinct open-source project with its own assets, branding and additional ideas.

## Deferred decisions

- Exact Bevy/Rust/toolchain versions and graphics backend.
- Chunk dimensions and persistence format.
- Final branding and asset style for public distribution.
- Multiplayer protocol, mod/plugin architecture, redstone compatibility specifics.
- Timing of packaged releases, third-party notice bundles and final asset licensing (the source repository is already public).

Resolve these incrementally while preserving backward-compatible migration paths when practical.

## Platform decisions

- Primary platforms from first working slice: **Void Linux glibc x86-64** and **Windows 10/11 x86-64**.
- Support both **Wayland and X11** on Linux where Bevy/wgpu allow it.
- Void Linux musl, older GPU hardware and additional CPU architectures are experimental; do not promise them without testing.
- Keep the simulation OS-agnostic and rely on the graphics library's supported native backends. See `docs/PLATFORMS.md`.

## Slice 1 implementation decisions

- Use Rust **1.99.1**, pinned in `rust-toolchain.toml`, and Bevy **0.20.0**, pinned in the workspace manifest. Commit the application lockfile. Revisit versions together in a tested dependency-upgrade PR.
- Start with two workspace members: the dependency-free `voxel_core` library and the Bevy `rustcraft` client. Do not create speculative empty subsystems.
- Use **16³** chunks, Y-up integer block coordinates, Euclidean handling of negative coordinates and checked coordinate conversions. X varies fastest in storage, followed by Z and Y.
- Use an original fixed four-chunk test world and a simple closed block enum for slice 1. This resolves the initial scene scope, not seeded generation or a persistent registry format.
- Generate visible faces across chunk boundaries with engine-neutral mesh buffers. Use original vertex colors instead of textures. Greedy meshing is deferred until profiling justifies it.
- The first-person camera flies without collision. Capture starts on click; Escape and focus loss release it. Physics, editing, inventory and saving remain subsequent slices as scoped in the handoff.
- One-time synchronous construction of four small chunks is an explicit bounded prototype tradeoff. Streaming or interactive remeshing must not extend this into unbounded frame-loop work.
- Enable X11 and Wayland together. Preserve Bevy/wgpu's supported per-platform backend selection and omit unused audio/gamepad/UI/import features.


## Slice 2 implementation decisions

- Keep physics as `voxel_core::physics`, a focused module with no new dependencies. The client adapts commands/time and presents the player; it does not own collision rules.
- Use feet-centre f64 coordinates, a 0.6×1.8-block AABB and eye height 1.62. Run commands at fixed 60 Hz, with yaw-relative acceleration, sprinting and grounded-only jump requests. All motion tuning is in `PhysicsConfig::default`.
- Resolve swept voxel collisions in fixed Y/X/Z order. Query authoritative neighboring blocks (missing chunks are air), clip to contact faces and preserve tangential movement. Static block overlap at spawn is not supported; automatic step-up and a general rigid-body engine are out of scope.
- Latch input edges before the fixed loop and interpolate camera translation afterwards. Mouse view remains immediate. Held Space does not auto-jump and pre-capture hover motion does not rotate the view.
- Replace free flight with walking as the only mode. R respawns; Y below -32 triggers automatic recovery. Pointer release/focus loss pauses the local prototype, including gravity; this must be revisited for an authoritative server.
- Retain Rust 1.99.1 / Bevy 0.20.0, both desktop Linux window backends, native Windows backend selection, the original scene and existing asset/dependency policy.

## Planned texture and visual asset direction

- The owner prefers **Minecraft-inspired retro pixel-art textures** that are independently drawn or distributed under an explicitly suitable license. Similar aesthetics do not authorize copying Minecraft's original texture pixels, audio, branding or extracted game resources.
- When implementing textures, require provenance and verifiable permission for **public source redistribution**, modification and distribution with future binaries; record exact authors, upstream URL, license/terms, attribution and modifications for every included asset.
- Treat source code's MIT OR Apache-2.0 license separately from the asset licenses. Default to original placeholders until licensed art is available; no asset-pack download/copy without review.


## Slice 3 implementation decisions

- Keep targeting/mutation in dependency-free `voxel_core`; use f64 DDA from the displayed interpolated eye, five-block default reach and explicit X/Y/Z tie ordering. Starting in solid permits breaking but supplies no placement face.
- Use one operation per mouse click, suppress capture/reset frames, prioritize breaking for simultaneous buttons and select grass/dirt/stone using 1/2/3. No inventory, mining timers or save/load are introduced.
- Bound editable cells to X/Z [-32,32), Y [-16,32). Create missing chunks lazily and return explicit change/no-op/error outcomes. Reject placement inside both authoritative and displayed player bodies or over existing solids.
- Rebuild only the changed chunk and face-neighbors at boundaries. Keep one retained CPU/GPU mesh handle per resident chunk, hide empty geometry and refresh culling bounds. Synchronous click jobs stay bounded; larger worlds will need scheduling budgets.
- Render an original procedural crosshair, target wire cube and three color swatches without additional crates, engine features, fonts or downloaded assets. Keep the existing original block colors; future pixel art follows the recorded provenance policy.
- Preserve edits across R/void respawn and choose clear space above an obstructed spawn. All changes are memory-only and disappear when closing the application.

## Slice 4 implementation decisions

- Keep persistence in the existing Bevy-independent `voxel_core::persistence` module. Use full-world sparse non-air snapshots, rather than overlays coupled to demo/worldgen revisions. Missing records mean Air; only a missing file with no backup creates the demo.
- Specify little-endian magic/version, stable explicit IDs, sorted signed world coordinates, strict bounded validation and CRC-32. Version 1 preserves the existing finite edit volume. Reject unknown versions/IDs without rewriting; future upgrades need explicit migration. See `SAVE_FORMAT.md` for the byte contract.
- Add only `crc32fast` 1.5.3 (MIT OR Apache-2.0), keeping Rust 1.99.1 and Bevy 0.20.0. No new assets or renderer features.
- Resolve Linux XDG/HOME and Windows LOCALAPPDATA/APPDATA per-user paths in the client, support an absolute `RUSTCRAFT_DATA_DIR`, and log the actual path. Core filesystem tests inject isolated directories.
- Keep transient mutation revisions, capture bounded immutable snapshots and serialize/sync on a single background save worker. F5 saves changed data; 30-second real-time autosave skips clean worlds. A completed old snapshot must not clear newer edits.
- Use same-directory exclusive temporary files, file flush/sync, replacement rename, previous-generation backup and Unix directory sync. Refuse corrupt/unknown primary replacement and require explicit recovery; do not silently fall back to a new demo. Hold an OS file lock to prevent concurrent RustCraft sessions on one world.
- Handle native window close explicitly, pausing controls and awaiting the latest successful save before exit. Save failure keeps dirty data and cancels close so F5 can retry. Forced termination and absolute power-loss durability are not promised.
- Load authoritative world data before startup meshes/physics and search upward for a clear spawn. Player position, selected block, inventory, world selection, cloud saves and seeded generation remain outside this slice.

## Slice 5 implementation decisions

- Add original, Bevy-independent integer worldgen in the existing core crate. Pin generator v1 and default seed 2026; use deterministic coordinate hashes/Q10 coherent noise, fixed finite bounds, a connected cave/ramp plan and spaced trees sampled across seams. No new crate/dependency or speculative streaming architecture.
- Accept `RUSTCRAFT_WORLD_SEED` as decimal u64 only when a world is genuinely absent. Existing valid snapshots bypass all seed parsing/generation and remain byte-for-byte untouched until changed saves. Preserve the demo as a library test fixture.
- Add solid, opaque Wood/Leaves and selection keys 4/5 with original colors. Keep IDs 1–3 unchanged; write format v2 with IDs 4/5 and strictly retain v1 reading. Upgrade on successful changed saves using the existing backup/atomic protocol, never on load. Full generated blocks are persisted; seed metadata is deliberately omitted.
- Explicitly request one initial background save for new generated worlds even without edits. Retain dirty/creation state on failure; idle loaded/created worlds are not rewritten. All previous save, lock, retry and final-close guarantees remain.
- Determine startup/R/void recovery from sky-exposed supported terrain, skipping tree tops and checking the actual player AABB. An empty legacy snapshot stays empty and uses a clear-air fallback.
- Add a camera-relative bitmap status panel using original 5×7 glyph meshes: yellow saving, green success, persistent red error/F5 retry. Keep it visible with the cursor released and reuse mesh/material handles. No imported fonts, textures or Bevy UI feature changes.
- Bound and measure generation/initial meshing; record golden cross-platform snapshots, v1/v2 compatibility, trees/caves/bounds/spawn and gameplay regressions. Native Void/Windows GUI acceptance remains distinct from software-rendered X11 and CI.

## Slice 6 implementation decisions

- Retain v1/v2 full snapshots as explicitly labelled legacy finite worlds, including missing Air and deleted terrain. No automatic expansion/migration. New worlds select streamed generator v2, bounded X/Z [-64,64), Y [-16,32); old generator-v1 fixtures stay unchanged.
- Write v3 identity (seed + generator version) and a canonical last-write overlay with explicit Air, rather than a dense world or resident-only snapshot. Cache eviction never removes edits; F5/autosave/final save captures the entire overlay. Refuse identity/mode/seed replacement and preserve all existing atomic/backup/lock rules. Forced-kill losses since the last completed save remain possible: no per-click durable journal.
- Use a radius-2 default (configurable 1–3), bounded desired/dirty sets and one generation/mesh worker. Following native review, spawn is no longer pinned: R/void requests the ordinary spawn cache and waits before ground selection. Results validate local mesh dependencies, so unrelated edits no longer discard useful work; stale or no-longer-desired work is discarded. Frames never join unfinished jobs. Logical cancellation is sufficient for bounded jobs; cooperative cancellation within jobs is deferred.
- Generate new columns from center chunks and a one-cell face halo using immutable generator/overlay snapshots. Remesh edits from cached target/face-neighbor chunks, sampling only missing face cells. Coalesce connected dirty meshes, prioritize nearby geometry and publish current connected batches atomically. Halo meshes are tested against full neighbors. Missing in-volume chunks are safe collision/ray barriers and reject edits; pending teleports pause and recover clearance after load.
- Keep canonical edit state independent of resident/GPU resources. Eviction removes block vectors, entities and mesh assets; ready-empty chunks need no GPU mesh. Existing legacy empty-mesh handle behavior remains unchanged. Shared overlay snapshots use bounded copy-on-write; regional storage/indexing and dense-edit latency work remain future steps.
- Measure reproducible lifecycle workloads, counts/payload/mesh buffers and actual job timings. Add visible title counts/seed/version/radius plus existing save HUD. Keep Rust/Bevy/dependencies/assets unchanged. Use the collision contact tolerance for clearance/recovery too, preventing ULP-sized diagonal contacts from triggering a corrective teleport. This is safe bounded streaming, not infinite-world parity; the larger-world/performance direction in issue #13 and later licensed textures remain separate work.

## Slice 7 implementation decisions

- Start from accepted main `d2acca8`. Preserve all v1/v2 snapshot semantics and v3/gen2 output/bounds. New directories alone select v4/gen3; no automatic conversion.
- Make generator v3 local and deterministic with bounded column height/tree plans and world-coordinate caves. Roll out X/Z [-32768,32768), Y [-16,32), rather than claiming literally infinite coordinates. Further extent/algorithm changes need explicit version decisions.
- Store a constant identity manifest and computed/sharded 64×64×48 regional overlays, with world nonce, seed, generator, coordinates, stable IDs and CRC. Read only needed regions off-thread; never scan every visited file on startup or retain an all-world edit map.
- Pin unsaved overlay data, not distant voxel/GPU chunks. Limit to eight dirty regions, request saves at four, retain failed/newer revisions and reject further new-region edits at capacity. Regional COW is independent of total explored world size.
- Keep one bounded terrain worker: measured local generation is substantially cheaper; a pool is not yet justified. Preserve local tokens, connected dirty atomic publication and fast cached remeshing; prioritize motion inside near rings. Optimize interior mesh reads without changing emitted geometry.
- Retain file-level atomic temp/sync/backup/rename and session locks. Regions commit individually; partial multi-region failure is not a whole-world rollback, retains dirty RAM state and cancels normal close. Idempotent retries retain older backups. Forced termination still can lose unsaved changes.
- Use a floating presentation origin while keeping f64 world-space physics/rays. Add an optional v4-only distant startup chunk for testing; R still requests origin and does not keep its meshes pinned.
- No dependency/feature/toolchain upgrades or texture assets. Future larger extents, migration, dense-region and bulk-edit work remain distinct follow-ups, guided by measurements.

## Slice 8 implementation decisions

- Author seven original editable 16×16 pixel grids and sRGB palettes under explicit MIT OR Apache-2.0 art licensing; no imported/reference textures. Commit JSON, a standard-library Python compiler, PNG previews/atlas and matching raw bytes; CI checks regeneration.
- Embed one immutable 128×64 RGBA8 sRGB atlas, so the existing feature/dependency graph needs no PNG decoder or asset-directory lookup. One opaque rough terrain material and one mesh per rendered chunk remain; no per-block entity/material.
- Allocate 32×32 atlas slots with a centered 16×16 tile and eight-pixel edge extrusion. Nearest LOD0, one mip and no anisotropy prevent neighbor-tile filtering. Distant minification shimmer is an explicit tradeoff; per-tile mip chains/array textures are future work.
- Replace terrain vertex colors with engine-neutral UVs; keep UI colors, original CCW geometry and hidden-face culling. Precompute quad UVs to avoid repeated per-vertex atlas math. Terrain stride drops from 40 to 32 bytes, with identical index count.
- Keep lighting, floating origin, physics, block IDs, generation and every save-format byte contract unchanged. Validate geometry/UV seams, atlas resources and gameplay against accepted Slice 7, separating software rendering from native Void/Windows acceptance.

## Slice 9 implementation decisions

- Fixed nine hotbar + 27 main slots, stacks 64, explicit five item IDs, held cursor
  persisted; validated private stack fields and conservative no-drop behavior.
  Starter Grass/Dirt/Stone 64 and Wood/Leaves 32 is granted only at initialization.
- Stage the complete inventory result before validated world mutation; commit it
  only after success. Full inventory refuses mining; empty slot refuses placement.
  Retain current click-based mining, body/pending safety and mesh batching.
- E is modal: free pointer, paused gameplay, no click-through on transitions.
  Return held items all-or-nothing on close; if full keep open. Normal application
  close saves a held stack, automatically reopening the modal on restart.
- Original geometry/glyph UI reuses the atlas: four retained meshes, two unlit
  materials, no image additions/dependencies. Cursor movement only moves transforms.
- Keep world formats v1–v4 and frozen generators intact; add player v1 sidecars
  and a bounded save-level redo journal under the existing world lock. Capture
  world/player together on one worker; replay valid intent before gameplay.
  Fail closed on corrupt/foreign/unknown/missing initialized data, never refill.
- Whole closed-directory backups only; no promise of per-click durability or
  cross-file atomicity. Initial identity-before-journal failure and old-client/
  external-region restoration limits are explicit in INVENTORY and SAVE_FORMAT.

## Slice 10 implementation decisions

- Keep direct vertical skylight at15; scatter through six-connected Air with -1
  per step, opaque leaves, full 48-block vertical span, fixed finite edge policy.
  Derived light is never serialized. No AO, emitters or dynamic sun in this slice.
- Exact local solve uses a 15-block horizontal margin and full square source
  columns. Fetch diagonal region inputs as well as face neighbors; unknown/corrupt
  input fails safely instead of becoming Air or light. Cache pristine generated
  halos, apply captured overlays and discard edited scratch after each job.
- Geometry and conservative lighting targets are distinct. Compare exact cached
  samples, not hashes, to skip unchanged meshes. Validate full-height local output
  tokens and publish changed meshes/samples together; retain old complete visuals.
- Use Bevy's built-in f32 RGBA COLOR path and one unlit atlas material, accepting
  32→48-byte terrain stride /42.1% quad payload. Remove global directional light
  and ambient brightness; fixed face factors and a small floor preserve retro
  readability. Existing UI stays unlit. Packed attributes are a measured future
  optimization, not an unreviewed shader rewrite.
- Legacy finite edits move to a bounded background mesh/light worker. Tests wait
  for rendering only when asserting meshes, preserving exact physics tick counts
  independently of OS thread/timer scheduling. CI exposes bounded test failures
  as annotations when full log storage cannot be accessed.
- This is incremental bounded batch rebuilding, not a fully persistent cell-level
  removal solver. Record additional CPU/load latency honestly; prioritize correct
  seams, capped caches, save compatibility and stable main-thread work first.

## Slice 11 movement decisions

- Tune ground movement to 5.0 walk /8.0 sprint /1.8 sneak blocks/s, acceleration65
  and braking75. Keep existing air control, gravity, jumping and contact epsilon.
- Left Ctrl sprints, Left Shift sneaks; a focused client bindings resource maps
  locomotion to engine-independent commands. Existing modal UI shortcuts stay.
- Crouch shrinks height1.8→1.5 and eye1.62→1.32, feet unchanged. Keep crouching
  until standing clearance exists. Actual posture controls all physics/edits.
- Restrict supported non-jumping crouch movement using ordinary support probes;
  partial support suffices, no step-down snap or airborne protection. Grounded
  stance entry caps speed immediately; air retains momentum and bounded control.
- Interpolate posture with feet and protect taller displayed transition bodies.
  Initial PR omitted optional FOV/head-bob effects; the owner-requested polish
  below adds FOV only. Formats, generators,
  dependencies and GPU resources stay unchanged; native feel needs owner testing.

### Slice 11 owner polish, same PR #28

- Owner accepted movement/sneak/fastbridging on native Void before these additions.
- Relative sprint FOV gain9%, exponential response10/s, max50-ms visual step.
  Respect current perspective base and return smoothly during local pauses; no bob.
- Default double-W uses configured forward,250-ms inclusive distinct edges and
  second-hold latch. Cancel gestures on conflicts, pauses, capture, reset/rebinding.
- Supersede Slice2's press-again-only jump policy: hold requests repeat only after
  actual landing, with pending short taps retained. Gate held jump until release
  across modal/focus/capture/reset/binding transitions, preventing phantom launches.
- Earn +1.2 horizontal speed at eligible sprint launches, max9.2; require run-up,
  positive directional alignment, overhead clearance and an unblocked step. Retain
  earned sprint momentum with bounded vector air control. No stacking, sneak,
  stationary, low-ceiling or pending-barrier boosts; core stays Bevy-independent.
