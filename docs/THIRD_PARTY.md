# Third-party dependencies and asset inventory

Project-created code and documentation are **MIT OR Apache-2.0**, at the recipient's option. See the root LICENSE-MIT and LICENSE-APACHE files. This does not relicense dependencies or separately licensed artwork.

## Direct dependencies (slice 6; unchanged from slice 4)

| Component | Version | Upstream | SPDX license | Notices / use |
| --- | --- | --- | --- | --- |
| Bevy | 0.20.0 (exact pin) | https://github.com/bevyengine/bevy | MIT OR Apache-2.0 | Engine/rendering/window/input in `client`; retain chosen upstream license and applicable notices when distributing it. |
| crc32fast | 1.5.3 (exact pin) | https://github.com/srijs/rust-crc32fast | MIT OR Apache-2.0 | CRC-32 checksums in `voxel_core::persistence`; retain chosen upstream license/notice when distributing it. |
| voxel_core | 0.1.0 (workspace) | This repository | MIT OR Apache-2.0 | Project-authored engine-independent library, not vendored third-party code. |

`Cargo.lock` records exact transitive versions and checksums. No third-party source is copied or vendored. Slices 2 and 3 added only project-authored physics/controller/interaction code. Slice 4 adds `crc32fast` (Cargo metadata declares MIT OR Apache-2.0); its `cfg-if` dependency already existed in the graph. No asset or Bevy feature changes are required. Bevy's native renderer selection remains enabled; application features omit audio, gamepad, UI and external scene/texture import. All project-authored persistence code remains MIT OR Apache-2.0. Slice 5 adds only original Rust code/colors/bitmap glyphs; no crates, Bevy features, lockfile changes, downloaded fonts or art are added.

Slice 6 adds only project-authored Rust streaming/persistence/adapter code and generator-v2 composition, still MIT OR Apache-2.0. No crate, lockfile, engine feature, texture, font or asset additions. The PR #12 native-review corrections add only project-authored contact, lifecycle, local invalidation, cached remeshing, metrics and regression-test code under the same license.

## Dependency license review

The initial resolved Linux and Windows MSVC dependency metadata was inspected. The graph contains permissive license expressions (MIT, Apache-2.0, BSD-2-Clause/BSD-3-Clause, ISC, Zlib, MIT-0, BSL-1.0 and alternatives including Unlicense/CC0), plus the following additional conditions:

- `unicode-ident`: `(MIT OR Apache-2.0) AND Unicode-3.0`; retain the Unicode notice as well.
- `bevy_mikktspace`: `Zlib AND (MIT OR Apache-2.0)`; retain the Zlib notice in addition to the chosen Rust portion's license.
- `dpi`: `Apache-2.0 AND MIT`; comply with both notices.
- `rustix` / `linux-raw-sys` offer `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`; a compatible alternative may be selected.
- Windows `clipboard-win` and `error-code` use BSL-1.0 (Boost Software License); preserve applicable license text when distributing them.
- `winit`, `accesskit_winit` and some shader/font support crates are Apache-2.0; preserve applicable upstream notices on distribution. Some metadata uses legacy `/` spellings for MIT/Apache alternatives.

No copyleft-only crate was identified in the reviewed graph. This is an initial dependency selection review, not a complete binary-distribution notice bundle. Recheck the graph after dependency changes and produce the full license/NOTICE bundle before shipping distributable binaries or asset packages. OS-provided graphics/window libraries remain system dependencies with their respective licenses.

## Asset provenance

| Asset | Author / origin | License | Attribution |
| --- | --- | --- | --- |
| Grass, dirt, stone, wood and leaves vertex colors | Original values authored in `crates/voxel_core/src/chunk.rs` for RustCraft | MIT OR Apache-2.0 | Root project licenses |
| Crosshair, selection swatches/frame and target wire cube | Original procedural geometry in `client/src/interaction.rs` | MIT OR Apache-2.0 | Root project licenses |
| Original 5×7 bitmap save-status lettering and panel mesh | Authored in `client/src/save_hud.rs` | MIT OR Apache-2.0 | Root project licenses |
| Seeded terrain, integer noise/hash composition, cave/ramp and tree geometry | Authored in `crates/voxel_core/src/worldgen.rs` | MIT OR Apache-2.0 | Root project licenses |
| Terraced demo geometry and stone landmark | Original procedural layout in `crates/voxel_core/src/world.rs` | MIT OR Apache-2.0 | Root project licenses |

There are no downloaded textures, music, sounds, fonts, logos, models or Mojang/Microsoft assets. Engine-provided shaders and platform window decorations are dependencies, not project-authored assets; their upstream licensing remains applicable.

## Policy for future changes

- Record dependency name, version/commit, upstream URL, SPDX identifier and required notices when adding third-party code.
- Prefer MIT, Apache-2.0 or other compatible permissive dependencies; review copyleft packages before copying or linking code.
- Preserve copyright, attribution, NOTICE and license files.
- Independently implement mechanics; do not copy proprietary game source, textures, sounds, logos or branding.
- Document each asset's author, origin, license and attribution obligations separately from code.
- The source repository is already public: review every new committed source/asset for licensing and sensitive data. Before packaged releases, audit target-specific/transitive dependencies, included assets and Git history and assemble required distribution notices.
- The owner made the repository public on 2026-10-21. Never change visibility or publish binary releases without explicit owner approval.

## Planned pixel-art asset policy

The intended aesthetic is classic, blocky **Minecraft-inspired** pixel art, but the repo must not include extracted Minecraft textures, edited proprietary originals, or packs whose authors do not explicitly permit the planned use.

For an external texture pack, check its precise license/permission covers redistribution in this **public GitHub repository**, game distribution, editing/derivative use and intended commercial use if applicable. Save its name, author, direct source URL, license text/version, modification notes and required credit. A free download, a fan-made pack, or "Minecraft-style" alone is **not** sufficient permission. Avoid assets with incompatible usage conditions. Prefer original art or clearly licensed works such as CC0/appropriately attributed CC BY assets when their terms fit. Never assume code's MIT OR Apache-2.0 license applies to the artwork.
