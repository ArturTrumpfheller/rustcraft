# Third-party dependencies and asset inventory

Project-created code and documentation are **MIT OR Apache-2.0**, at the recipient's option. See the root LICENSE-MIT and LICENSE-APACHE files. This does not relicense dependencies or separately licensed artwork.

## Direct dependencies (slice 10; unchanged from slice 4)

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
| UI swatch colors / historical terrain vertex colors | Original values authored in `crates/voxel_core/src/chunk.rs` for RustCraft | MIT OR Apache-2.0 | Root project licenses |
| Crosshair and target wire cube (historical selection swatches removed) | Original procedural geometry in `client/src/interaction.rs` | MIT OR Apache-2.0 | Root project licenses |
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

## Slice 7 additions

Local generator-v3 planning, regional formats/storage, cache/scheduling changes,
floating-origin presentation and tests/benchmarks are original project-authored
MIT OR Apache-2.0 Rust/documentation. No new crate, lockfile, Bevy feature,
toolchain, texture, font, third-party source or imported asset was added.

## Slice 8 original terrain textures

The seven 16×16 tiles, editable `assets/terrain/source.json`, generated individual
PNGs, `atlas.png`, `atlas.rgba`, and `tools/generate_textures.py` are **original
RustCraft project artwork/code authored for Slice 8 through Codex-assisted
development, MIT OR Apache-2.0**. This explicit license covers the artwork as well
as code. No input/reference images, Minecraft files, third-party packs or external
art were used. Root license/notice obligations apply to future redistribution.
See [asset provenance and reproduction](../assets/terrain/README.md) for each tile,
source grids/palettes, generator and output layout. The in-game screenshot `docs/images/slice8-terrain.png` is a capture of this
original art/procedural geometry/bitmap UI, under the same project license.
No new game dependency, Bevy
feature, image decoder or toolchain change is required. Python standard library
is an optional authoring/check tool, not a game runtime/build requirement.

## Slice 9 original inventory UI

All new inventory/persistence/UI code, tests and documentation are original
RustCraft work, MIT OR Apache-2.0. Frames, navy/earth/copper palette and 3×5 bitmap
count/label glyphs are authored directly in `client/src/inventory_ui.rs`; these
editable geometry/tables are the UI source, with no downloaded font or texture.
Icons sample the already licensed original terrain atlas from Slice 8. No image,
crate, dependency graph, toolchain or engine-feature changes were introduced.
`docs/images/slice9-inventory.png` is an actual X11/software-rendered screenshot
of this original code/art, under the same project licenses. Platform decorations
and engine shaders retain their upstream licenses. No proprietary UI assets used.

## Slice 10 skylight

Light solver, worker/cache/invalidation, modulation factors, tests and documents
are original RustCraft code, MIT OR Apache-2.0. Screenshots
`docs/images/slice10-cave-closed.png` and `slice10-cave-open.png` show an original
software-rendered test tunnel using our existing atlas/UI under the same license.
No new images used by the game, external assets, fonts, shaders, dependencies or
feature/toolchain changes. Bitmap UI and terrain art retain Slice8/9 provenance.

## Slice 12 original hand and viewmodel

The single-cuboid arm, original16×16 skin/slate pixels reproducibly authored in
`client/src/viewmodel.rs::arm_image`, rendering/animation code, tests and docs
are original RustCraft work, **MIT OR Apache-2.0**, including the artwork.
Procedural Bevy cuboids use the normal dependency API. Held cubes share our
original Slice8 terrain atlas. No proprietary model, texture, skin layout,
source, font or animation code is imported/copied.

Visual behavior is calibrated against independently captured, unmodified official
Minecraft Java1.21.1 Demo footage, using wide Steve, plus publicly viewable Java
screenshots during investigation. Those executables, assets and reference images
remain outside Git and are not licensed as RustCraft artwork. Only measured
geometric landmarks and our independent implementation are recorded here.
`docs/images/slice12-revised-*.png` and `slice12-before-*.png` are actual RustCraft
captures of our own art/code, under the root licenses, as are the earlier
`slice12-viewmodel.png` and `slice12-cave.png`. No dependency/feature/shader or
external-asset distribution changes.

## Slice 13 original menus

`client/src/menu.rs` frames, stepped landscape, copper/slate/cream palette and
expanded bitmap glyph tables are original RustCraft artwork/code, **MIT OR
Apache-2.0**, including `docs/images/slice13-title.png` and `slice13-pause.png`
(actual Xvfb/Mesa captures). No imported font, image, proprietary menu art,
dependency, engine feature or lockfile change is added.

## Slice 15 settings

Configuration, menu sliders/controls, tests and documentation are original RustCraft
MIT OR Apache-2.0 work. Gray bevels and bitmap lettering reuse Slice14 artwork.
No new dependencies, lockfile, external fonts/art, renderer features or toolchain changes.

`docs/images/slice15-*.png` are actual Xvfb/Mesa RustCraft captures of these
original menus and the existing original terrain/hand, MIT OR Apache-2.0.
No reference game artwork or external font is included in them.

Slice15 UI polish adds original English UI text, slider adaptation, contrast
palette and OS-event-derived key labels. The actual Xvfb/Mesa captures named
`slice15-polish-*.png` show original RustCraft artwork, MIT OR Apache-2.0.
No dependencies, third-party artwork, fonts or keyboard libraries were added.
