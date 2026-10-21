# RustCraft — project decisions

This document summarizes choices already made by the project owner and is intended to survive across Codex sessions.

## Confirmed direction

- **Project:** RustCraft, an independent Minecraft-inspired voxel sandbox.
- **Initial scope:** Minecraft Classic-like experience as a starting point; gradually expand toward Infdev/Alpha/Beta/Release-inspired systems and eventually original ideas.
- **Visual and gameplay direction:** recognizable, authentic retro block-world feel, without intentionally recreating easily fixable historical bugs.
- **Language:** Rust rather than Java or C.
- **Engine:** Bevy is preferred for app/rendering/input; game simulation, voxel model and core rules should have a modular boundary.
- **Open-source assistance:** allowed; use appropriate, documented licenses and existing crates rather than writing every low-level facility from scratch.
- **Repository:** `ArturTrumpfheller/rustcraft`, private during early development, potentially public later.
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
- Timing of any publication and final asset licensing.

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
