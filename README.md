# RustCraft

**RustCraft** is a private, experimental Minecraft-inspired voxel sandbox written in Rust. The first v0.1 slice displays a small, contiguous block world with a controllable first-person **free-flight camera**. The long-term goal is a Classic-style editable world, followed by survival and original sandbox mechanics.

RustCraft is an independent project, not a Minecraft source port, and is not affiliated with or endorsed by Mojang or Microsoft.

## Current prototype

- Four adjacent 16×16×16 chunks, fixed terraced terrain and a stone landmark.
- Grass, dirt and stone represented by original vertex colors; no downloaded game assets.
- One surface mesh per chunk; faces shared by solid blocks are removed, including at chunk boundaries.
- Mouse look, horizontal WASD flight, vertical movement, speed boost and camera reset.
- Engine-independent voxel library with coordinate, storage and mesh regression tests.

This is **v0.1 / slice 1**, not the complete v0.1 milestone. There is no collision, gravity, block editing, seeded terrain, hotbar or persistence yet. The camera can pass through blocks. See [roadmap](docs/ROADMAP.md) and [architecture](docs/ARCHITECTURE.md).

## Build and run on Linux

Install [rustup](https://rustup.rs/) and a C/C++ toolchain. The repository selects **Rust 1.99.1** (including rustfmt and Clippy) through `rust-toolchain.toml`; Cargo installs it when needed. Bevy is pinned to **0.20.0**, and `Cargo.lock` records the dependency graph.

On Void Linux **glibc** x86-64:

```sh
sudo xbps-install -S base-devel pkgconf libX11-devel libxcursor-devel \
  libXi-devel libXrandr-devel wayland-devel libxkbcommon-devel
# For a supported AMD GPU:
sudo xbps-install -S vulkan-loader mesa-vulkan-radeon
```

On Debian/Ubuntu:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libx11-dev libxcursor-dev \
  libxi-dev libxrandr-dev libxkbcommon-dev libwayland-dev libvulkan-dev
```

Use a working X11 or Wayland desktop session and a supported graphics driver. The Linux package names can vary by distribution. See [platform details](docs/PLATFORMS.md), including Windows prerequisites and experimental targets.

From the repository root:

```sh
cargo run --locked -p rustcraft
```

The first Bevy build can take several minutes. To limit CPU and memory consumption, use `CARGO_BUILD_JOBS=4 cargo run --locked -p rustcraft`. No asset folder, service, credentials or environment file is required. Closing the window exits the application.

In the prepared Codex cloud machine, first run `. /workspace/.rustcraft-env/activate.sh` in each shell. A cloud build alone does not establish native graphical compatibility.

## Controls

| Input | Action |
| --- | --- |
| Left click inside the focused window | Capture/hide the mouse and enable camera controls |
| Escape | Release/show the mouse and pause camera input |
| Window focus loss | Release mouse; click again after returning |
| Mouse | Look around; pitch is clamped |
| W / A / S / D | Move horizontally relative to view direction |
| Space / left Ctrl | Fly up / down |
| Left Shift | Fly faster |
| R while captured | Reset camera to the initial overview |

Movement speed is independent of diagonal input. Mouse sensitivity does not depend on frame time. The window title also lists the essential controls.

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

Run only the engine-independent tests with `cargo test --locked -p voxel_core`. Client tests exercise actual Bevy scene and input systems without initializing a GPU or window. They do not substitute for real input/rendering checks.

The [slice 1 validation report](docs/VALIDATION.md) records the executed checks and remaining platform coverage.

[Manual acceptance checks](docs/PLATFORMS.md#slice-1-manual-test) cover the visible scene, pointer capture/release, focus changes and platform behavior. CI checks Linux and Windows compilation, formatting, Clippy and unit tests; it does not establish GUI compatibility on Void or Windows.

## Licensing and development policy

Project-authored source code and documentation are **MIT OR Apache-2.0**; see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Third-party dependencies and artwork keep their own licenses. See [dependency and asset inventory](docs/THIRD_PARTY.md).

Keep the repository private and `main` stable. Develop features on topic branches and submit pull requests for review. No proprietary game source, textures, sounds or logos are included.
