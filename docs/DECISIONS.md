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
