# RustCraft — Codex agent instructions

This is a **publicly accessible, early-development** Rust project intended to become an independent, open-source voxel sandbox. We start with a Minecraft Classic-inspired prototype and iterate toward survival, richer world generation, multiplayer, and original ideas. It is **not** an existing Minecraft source port.

## Required reading before changing code

- `README.md`: identity, stack, current status and licensing
- `docs/ROADMAP.md`: milestone scope and acceptance criteria
- `docs/ARCHITECTURE.md`: technical separation and future components
- `docs/DECISIONS.md`: decisions already made with the project owner
- `docs/THIRD_PARTY.md`: dependency and asset policy
- `docs/CODEX_HANDOFF.md`: historical handoff for completed slice 1; **do not** treat it as the scope of new tasks

The repository files are the persistent source of project context. Do not assume access to previous ChatGPT conversations. For new development, read the **current GitHub issue** and its acceptance criteria first; that issue supersedes historical handoffs. Start from the latest `main` instead of reusing a finished feature branch.

## Technical rules

- Use **Rust**. Prefer **Bevy** for presentation and platform integration; keep voxel storage, world generation and simulation logic as Bevy-independent as reasonably possible.
- Use Cargo and straightforward, current, mutually compatible library versions. Commit `Cargo.lock` for this game application and record the chosen toolchain/version.
- Start with the smallest playable vertical slice. Do not build a large set of empty crates or speculative abstractions.
- Keep chunk data and world coordinates consistent and covered by tests; use deterministic seeds for world generation.
- Do not block the rendering loop on expensive world-generation, save or mesh jobs without a clear bounded prototype rationale.
- Aim for classic-looking visuals and faithful game feel, but do not intentionally preserve old bugs that add nothing to gameplay.
- Prioritize readable, maintainable modules over clever code or premature micro-optimizations.
- Do not copy Mojang/Microsoft proprietary source, Minecraft textures, sounds, trademarks, or other protected game assets into this repository.

## Licensing and publication

- Newly authored source code and documentation: **MIT OR Apache-2.0**; retain both root license files and use the same SPDX expression in Cargo manifests.
- Check each third-party dependency's license, and record direct dependencies / vendored code / included assets in `docs/THIRD_PARTY.md` before a release.
- Use self-created or appropriately licensed temporary assets. Do not assume licensing for artwork matches code licensing.
- The project owner explicitly made the repository **public** in October 2026. Do not change visibility, publish binaries/releases, or push secrets without explicit authorization. Public repositories expose commits, branches, PRs and history; check all additions for private data, licenses and proprietary assets.

## Implementation workflow

1. Inspect the current branch, files, constraints, and the relevant roadmap milestone.
2. Make changes on a focused feature branch when working through GitHub; leave `main` stable for reviewed changes.
3. Implement and test an end-to-end slice; do not mark unfinished behavior as complete.
4. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` whenever a Cargo workspace exists. If a command cannot run, explicitly state why.
5. Check the resulting diff for unexpected files, private data, and accidentally bundled third-party assets.
6. Update documentation and roadmap checkboxes only for functionality actually implemented and verified.
7. Provide a concise report of what changed, commands tested, remaining limitations, and how to launch/test locally.
8. Prefer a Pull Request for review rather than merging into `main` without explicit user approval.

## Working with this project's owner

Communicate results and important questions in German. Technical identifiers and repository documentation may be in English. When details are unspecified, make well-reasoned reversible defaults; avoid interrupting progress for minor choices. Be explicit about assumptions and about runtime tests that were not possible.

## Cross-platform baseline (mandatory)

- Treat **Void Linux glibc x86-64 and Windows 10/11 x86-64** as first-class targets from the beginning. Read `docs/PLATFORMS.md` before picking renderer features or native crates.
- Bevy/wgpu must use supported platform-appropriate graphics backends; do not hardcode Vulkan as the only usable backend, or require a Windows-specific graphics API on Linux.
- Support X11 and Wayland on Linux; test mouse capture/unlock, keyboard and window behavior on both when possible.
- Keep paths, assets and save locations OS-neutral; explicitly document per-platform build/run requirements.
- CI should run Cargo checks on both Ubuntu and Windows. Headless CI never substitutes for actual GUI testing on Void and Windows.
- Void musl and the owner's older GPU are experimental targets, **not** a v0.1 compatibility requirement.
