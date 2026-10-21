# RustCraft

**RustCraft** is a public, experimental Minecraft-inspired voxel sandbox written in Rust. The current v0.1 prototype displays a small, contiguous block world with a first-person **walking player**, gravity, jumping, voxel collision and editable blocks. The long-term goal is a Classic-style editable world, followed by survival and original sandbox mechanics.

RustCraft is an independent project, not a Minecraft source port, and is not affiliated with or endorsed by Mojang or Microsoft.

## Current prototype

- Four adjacent 16×16×16 chunks, fixed terraced terrain and a stone landmark.
- Grass, dirt and stone represented by original vertex colors; no downloaded game assets.
- One surface mesh per chunk; faces shared by solid blocks are removed, including at chunk boundaries.
- Mouse look, yaw-relative WASD walking, sprinting, grounded jumping and safe respawn.
- Fixed 60-Hz engine-independent AABB physics, wall sliding and interpolated camera movement.
- Precise five-block targeting, visible target outline, click-to-break/place and 1/2/3 block selection.
- Engine-independent raycasts and validated edits; changed chunks and boundary neighbors update immediately.
- Coordinate, collision, targeting, mutation and rendering regression tests.

This is **v0.1 / slice 3**, not the complete v0.1 milestone. **Edits exist only in memory and vanish when the game closes.** Seeded terrain, a full inventory/hotbar and persistence are still future work. Walking is the default and there is no debug-flight mode. One-block terraces require jumping; automatic step-up is not implemented. See [roadmap](docs/ROADMAP.md) and [architecture](docs/ARCHITECTURE.md).

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
| First left click after release | Capture/hide the mouse and resume; does not break/place anything |
| Left click while captured | Break the targeted block (one block per click) |
| Right click while captured | Place the selected block on the target face |
| 1 / 2 / 3 | Select grass / dirt / stone |
| Escape | Release/show the mouse and pause the local player simulation |
| Window focus loss | Release mouse; click again after returning |
| Mouse | Look around; pitch is clamped |
| W / A / S / D | Walk relative to horizontal view direction |
| Space | Jump when grounded (press again for another jump) |
| Left Shift | Sprint |
| R while captured | Respawn above the starting terrace and reset view/velocity |

The 0.6×1.8-block player spawns above solid ground, with eyes 1.62 blocks above the feet. Walking accelerates toward 4.3 blocks/s; sprinting toward 5.6 blocks/s. Falling below Y=-32 automatically respawns the player. Escape or focus loss pauses the local simulation, including gravity; click again to continue.

The center crosshair aims at blocks within five blocks of the displayed eye. A black outline marks the current target; three colored swatches below the crosshair correspond to 1/2/3, with a white frame on the selected one. The window title also names the selected block. Empty clicks do nothing. Placement cannot replace solid blocks or intersect the current/displayed player body. Simultaneous buttons prioritize breaking.

Edits may create neighboring chunk storage within **X/Z [-32, 32), Y [-16, 32)**. Outside that box, edits are rejected. R preserves your edits and searches upward for clear space if the original spawn has been built over. There is no save command: closing/restarting restores the original four-chunk scene.

Movement speed is independent of diagonal input. Mouse sensitivity does not depend on frame time. The window title also lists the essential controls.

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

Run only the engine-independent voxel and physics tests with `cargo test --locked -p voxel_core`. Client tests exercise actual Bevy scene/input/fixed schedules without initializing a GPU or window, including frame-rate comparisons at 30/60/144 FPS. They do not substitute for real input/rendering checks.

The [validation report](docs/VALIDATION.md) records the executed checks and remaining platform coverage.

[Manual acceptance checks](docs/PLATFORMS.md#slice-3-manual-test) cover the visible scene, pointer capture/release, focus changes and platform behavior. CI checks Linux and Windows compilation, formatting, Clippy and unit tests; it does not establish GUI compatibility on Void or Windows.

## Licensing and development policy

Project-authored source code and documentation are **MIT OR Apache-2.0**; see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Third-party dependencies and artwork keep their own licenses. See [dependency and asset inventory](docs/THIRD_PARTY.md).

The source repository is public; keep `main` stable. Develop features on topic branches and submit pull requests for review. Publishing packaged game releases is a separate decision. No proprietary game source, textures, sounds or logos are included.
