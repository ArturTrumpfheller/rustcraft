# Codex handoff — first coding task

## Starting state

RustCraft is currently documentation-only: no `Cargo.toml`, source crates, game client or game assets are present. The repository is private. A GitHub Actions workflow is present and will run Rust checks once a root Cargo manifest exists.

**Start with milestone v0.1 in `docs/ROADMAP.md`.** The full milestone includes a playable Classic-style prototype; do not try to implement the whole long-term roadmap in a single coding task.

## Suggested first deliverable: v0.1 / slice 1

Implement a minimal, runnable Rust + Bevy application that demonstrates:

1. A reproducible Cargo project/workspace with a chosen compatible Bevy version and committed lockfile.
2. A window displaying a small 3D voxel scene made of multiple contiguous block faces, not only disconnected cubes.
3. A basic first-person camera with WASD + mouse look and a straightforward way to release/reacquire the mouse cursor.
4. An initial data representation for block IDs and world/chunk coordinates, minimally sufficient for the rendered scene.
5. Clear local launch instructions for Linux and tests for coordinate/indexing logic.
6. Fully original procedural/color or properly licensed textures; do not use proprietary Minecraft assets.

A minimal fixed world is acceptable in the first slice. More complex terrain generation, mining, collision and saves are *later tasks* unless they can be implemented and validated without destabilizing slice 1.

## Acceptance checks

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` succeed in a configured development environment.
- `cargo run` (or documented `cargo run -p <client_package>`) is the intended executable entrypoint, and the developer explains how to inspect the rendered scene.
- Source clearly separates voxel core concepts from visual presentation, at least by module.
- `README.md` includes instructions; the architecture document reflects *actual* implementation choices.
- No proprietary assets or unreviewed copyleft code introduced.
- Submit a PR or a reviewable branch, with an honest summary of tests and any unverified GUI behavior.

## Subsequent slices, separate tasks

- Visible-face / greedy meshing and chunk boundary handling.
- Seeded terrain generation and chunk lifecycle.
- Collision, gravity, jumping, block targeting / interaction.
- Inventory/hotbar and reliable world persistence.

## Practical setup

For **Codex Cloud**, select the GitHub repository `ArturTrumpfheller/rustcraft` in its environment. For **Codex CLI**, run it from a local clone of that repository. Repository files are available in either environment once connected; prior ChatGPT conversation text is not assumed available.
