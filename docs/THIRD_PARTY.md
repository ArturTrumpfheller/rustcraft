# Third-party dependencies and asset inventory

Project-created code and documentation are **MIT OR Apache-2.0**, at the recipient's option. See the root LICENSE-MIT and LICENSE-APACHE files. This does not relicense dependencies or separately licensed artwork.

## Direct dependencies (unchanged in slice 2)

| Component | Version | Upstream | SPDX license | Notices / use |
| --- | --- | --- | --- | --- |
| Bevy | 0.20.0 (exact pin) | https://github.com/bevyengine/bevy | MIT OR Apache-2.0 | Engine/rendering/window/input in `client`; retain chosen upstream license and applicable notices when distributing it. |
| voxel_core | 0.1.0 (workspace) | This repository | MIT OR Apache-2.0 | Project-authored engine-independent library, not vendored third-party code. |

`Cargo.lock` records exact transitive versions and checksums. No third-party source is copied or vendored. Slice 2 adds only project-authored physics/controller code under MIT OR Apache-2.0; no dependency, lockfile or asset additions are required. Bevy's native renderer selection remains enabled; application features omit audio, gamepad, UI and external scene/texture import.

## Dependency license review

The initial resolved Linux and Windows MSVC dependency metadata was inspected. The graph contains permissive license expressions (MIT, Apache-2.0, BSD-2-Clause/BSD-3-Clause, ISC, Zlib, MIT-0, BSL-1.0 and alternatives including Unlicense/CC0), plus the following additional conditions:

- `unicode-ident`: `(MIT OR Apache-2.0) AND Unicode-3.0`; retain the Unicode notice as well.
- `bevy_mikktspace`: `Zlib AND (MIT OR Apache-2.0)`; retain the Zlib notice in addition to the chosen Rust portion's license.
- `dpi`: `Apache-2.0 AND MIT`; comply with both notices.
- `rustix` / `linux-raw-sys` offer `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`; a compatible alternative may be selected.
- Windows `clipboard-win` and `error-code` use BSL-1.0 (Boost Software License); preserve applicable license text when distributing them.
- `winit`, `accesskit_winit` and some shader/font support crates are Apache-2.0; preserve applicable upstream notices on distribution. Some metadata uses legacy `/` spellings for MIT/Apache alternatives.

No copyleft-only crate was identified in the reviewed graph. This is an initial dependency selection review, not a complete binary-distribution notice bundle. Recheck the graph after dependency changes and produce the full license/NOTICE bundle before shipping binaries or making a public release. OS-provided graphics/window libraries remain system dependencies with their respective licenses.

## Asset provenance

| Asset | Author / origin | License | Attribution |
| --- | --- | --- | --- |
| Grass, dirt and stone vertex colors | Original values authored in `crates/voxel_core/src/chunk.rs` for RustCraft | MIT OR Apache-2.0 | Root project licenses |
| Terraced demo geometry and stone landmark | Original procedural layout in `crates/voxel_core/src/world.rs` | MIT OR Apache-2.0 | Root project licenses |

There are no downloaded textures, music, sounds, fonts, logos, models or Mojang/Microsoft assets. Engine-provided shaders and platform window decorations are dependencies, not project-authored assets; their upstream licensing remains applicable.

## Policy for future changes

- Record dependency name, version/commit, upstream URL, SPDX identifier and required notices when adding third-party code.
- Prefer MIT, Apache-2.0 or other compatible permissive dependencies; review copyleft packages before copying or linking code.
- Preserve copyright, attribution, NOTICE and license files.
- Independently implement mechanics; do not copy proprietary game source, textures, sounds, logos or branding.
- Document each asset's author, origin, license and attribution obligations separately from code.
- Before public release, audit all target-specific/transitive dependencies, included assets and Git history, and assemble required distribution notices.
- Keep the repository private until the owner explicitly decides otherwise.
