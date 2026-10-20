# RustCraft

**RustCraft** is a private, experimental Minecraft-inspired voxel sandbox written in Rust. The plan is to start with a small, classic-style block world, then incrementally add survival mechanics and later original features. No game implementation exists yet.

## Goals

- Preserve the recognizable retro voxel look and responsive gameplay while avoiding unnecessary historical bugs.
- Build modular, testable voxel simulation code with a clear separation from rendering.
- Start with a playable Classic-style prototype before expanding into survival, multiplayer, and custom mechanics.
- Prefer licensed open-source libraries over reinventing general infrastructure.

## Planned stack

- **Language:** Rust
- **Engine / application layer:** Bevy (version to be selected when implementation begins)
- **Voxel world and game simulation:** project-owned Rust modules
- **Build system:** Cargo workspace (to be added with the first implementation)

See [roadmap](docs/ROADMAP.md) and [architecture](docs/ARCHITECTURE.md). These are plans, not implemented features.

## Project status

**Planning / repository bootstrap.** Source crates, runnable client, and tests have not been added yet. The CI workflow will start Rust checks after a root `Cargo.toml` exists.

## Licensing

Project-authored **source code and documentation** are offered under **MIT OR Apache-2.0**, at your option. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Planned Cargo packages should declare `license = "MIT OR Apache-2.0"`.

Third-party code and assets retain their own licenses. **Textures, music, sounds, fonts, and other artwork are not automatically licensed under MIT/Apache-2.0** merely because they are stored here; each asset needs a clear origin and license before distribution.

RustCraft is an independent fan-inspired project and is not affiliated with or endorsed by Mojang or Microsoft. Do not add original Minecraft game code or proprietary assets.

## Development

The repository is currently **private**. Keep `main` stable; develop substantial features on topic branches and review/test them before merging. See [third-party policy](docs/THIRD_PARTY.md) for future publication hygiene.
