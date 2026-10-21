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
