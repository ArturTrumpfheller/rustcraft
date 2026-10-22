# RustCraft

**RustCraft** is a public, experimental Minecraft-inspired voxel sandbox written in Rust. The current v0.1 prototype displays a finite, procedurally generated block world with a first-person **walking player**, gravity, jumping, voxel collision and editable blocks. The long-term goal is a Classic-style editable world, followed by survival and original sandbox mechanics.

RustCraft is an independent project, not a Minecraft source port, and is not affiliated with or endorsed by Mojang or Microsoft.

## Current prototype

- Deterministic seeded terrain, rolling hills/valleys, a walkable cave network with surface entrance, and blocky trees.
- 16³ chunks in the finite editable volume X/Z [-32,32), Y [-16,32); no streaming yet.
- Grass, dirt, stone, wood and opaque solid leaves represented by original vertex colors; no downloaded game assets.
- One surface mesh per chunk; faces shared by solid blocks are removed, including at chunk boundaries.
- Mouse look, yaw-relative WASD walking, sprinting, grounded jumping and safe respawn.
- Fixed 60-Hz engine-independent AABB physics, wall sliding and interpolated camera movement.
- Precise five-block targeting, visible target outline, click-to-break/place and 1–5 block selection.
- Engine-independent raycasts and validated edits; changed chunks and boundary neighbors update immediately.
- Versioned world snapshots, F5 saving, 30-second dirty-only autosave and final save on normal window close.
- Checksummed files, recoverable replacement with a previous-generation backup and an exclusive world lock.
- Coordinate, collision, targeting, mutation and rendering regression tests.

This is **v0.1 / slice 5**, not the complete v0.1 milestone. **World edits survive restart.** Infinite streaming, richer world generation and a full inventory/hotbar are still future work. Walking is the default and there is no debug-flight mode. One-block terraces require jumping; automatic step-up is not implemented. See [roadmap](docs/ROADMAP.md) and [architecture](docs/ARCHITECTURE.md).

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
| 1 / 2 / 3 / 4 / 5 | Select grass / dirt / stone / wood / leaves |
| Escape | Release/show the mouse and pause the local player simulation |
| Window focus loss | Release mouse; click again after returning |
| Mouse | Look around; pitch is clamped |
| W / A / S / D | Walk relative to horizontal view direction |
| Space | Jump when grounded (press again for another jump) |
| Left Shift | Sprint |
| R while captured | Respawn on safe ground near the starting area and reset view/velocity |
| F5 while focused | Save changed world, including while the mouse is released |

The 0.6×1.8-block player spawns on or just above safe solid ground, with eyes 1.62 blocks above the feet. Walking accelerates toward 4.3 blocks/s; sprinting toward 5.6 blocks/s. Falling below Y=-32 automatically respawns the player. Escape or focus loss pauses the local simulation, including gravity; click again to continue.

The center crosshair aims at blocks within five blocks of the displayed eye. A black outline marks the current target; five colored swatches below the crosshair correspond to 1–5, with a white frame on the selected one. The window title also names the selected block. Empty clicks do nothing. Placement cannot replace solid blocks or intersect the current/displayed player body. Simultaneous buttons prioritize breaking.

Edits may create neighboring chunk storage within **X/Z [-32, 32), Y [-16, 32)**. Outside that box, edits are rejected. R preserves your edits and finds clear sky-exposed ground near the starting area, avoiding tree tops and unsupported columns. Loading uses the same search; player position, velocity and selected slot are not saved.

Movement speed is independent of diagonal input. Mouse sensitivity does not depend on frame time. The window title also lists the essential controls.

## New worlds and seeds

`RUSTCRAFT_WORLD_SEED` accepts a decimal unsigned 64-bit integer (0 through 18446744073709551615). The deterministic default is **2026**, generator version **1**. It is read **only when no primary or backup exists**; changing it never changes a loaded world. Invalid seeds stop creation with a clear error. To explore another seed, use a different, preferably fresh absolute `RUSTCRAFT_DATA_DIR`; never delete your real save to switch seeds.

Terrain uses coherent height variation with grass/dirt/stone layers. Caves connect across chunks and have a stair-stepped surface entrance near x=-10, z=20, descending toward the east/west tunnel near z=-10. Trees use supported 4–5-block trunks and simple crowns. Wood and leaves are both **solid and opaque**; there is no leaf decay, transparency, water, biomes, ore distribution or lighting propagation. The outer terrain is a finite cut-off edge; falling below the world respawns you safely. The historical demo remains a library test fixture.

New generated blocks are saved in full, so restarting never depends on regenerating from a seed. The seed is logged at creation; v2 deliberately does not store generator/seed metadata. The [generation contract](docs/WORLDGEN.md) explains deterministic arithmetic, limits and future extension.

## Saving, loading and recovery

RustCraft automatically loads **one local world** before creating graphics/physics. Only a missing save (and no backup) creates a seeded world. Existing saves are authoritative and never regenerated or rewritten on load, including historical v1 worlds. All solid materials, replacements, removals and newly created chunks are retained. A valid empty snapshot stays empty. The full snapshot is independent of future changes to the demo generator; see [format and failure policy](docs/SAVE_FORMAT.md).

- Linux: `$XDG_DATA_HOME/rustcraft/world.rcw`, or `$HOME/.local/share/rustcraft/world.rcw` when XDG_DATA_HOME is absent/empty/relative.
- Windows: `%LOCALAPPDATA%\RustCraft\world.rcw`, falling back to `%APPDATA%\RustCraft\world.rcw`.
- Override either platform with **an absolute directory** in `RUSTCRAFT_DATA_DIR`. Invalid overrides are errors. The resolved path is printed on startup. No save is stored relative to the executable or working directory.

Press **F5** to save changes, or wait for the **30-second** autosave interval. Autosave continues with the mouse released or the window unfocused. A new generated world is saved explicitly once even without edits; after that, unchanged worlds are not written. Normal window close pauses controls, waits for any ongoing save and writes the latest changes before exiting. A save failure cancels closing, keeps changes dirty and prints the cause; fix the path/disk problem and retry F5 or close again. The **in-game panel at the top left** shows `SAVING...` in yellow, `SAVED` in green or persistent `SAVE ERROR - F5 RETRY` in red, plus loaded/unsaved states. It remains visible with the cursor released; the title and logs also report status. Success appears only after completion. Save encoding and filesystem I/O run in a background thread.

Saves use a synced temporary file in the same directory followed by replacement; `world.rcw.bak` retains the previously valid generation once a second save is made. Never delete `world.lock` while a game runs: the OS lock prevents concurrent instances overwriting this world. A leftover lock file after exit is harmless. Temporary files are ignored on load. Disk/OS/filesystem failures and forced termination can still lose changes since the last completed save; this is not a power-loss guarantee. Do not force-kill the game while saving.

A corrupt, truncated or unsupported save **stops startup without overwriting it**. Recovery is deliberate rather than silent fallback. With all RustCraft instances closed, first copy the entire data directory somewhere safe. Then preserve the bad primary and copy the backup into its place, for example on Linux:

```sh
save_dir="${RUSTCRAFT_DATA_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/rustcraft}"
# Inspect this path first; use the startup log's path if XDG_DATA_HOME is relative.
mv -- "$save_dir/world.rcw" "$save_dir/world.rcw.damaged"
cp -- "$save_dir/world.rcw.bak" "$save_dir/world.rcw"
cargo run --locked -p rustcraft
```

Skip `mv` if the primary is already absent; do not overwrite an existing `.damaged` copy. Recovery loses edits newer than the backup. Without a valid backup, keep the files for diagnosis and choose a different data directory to start a new world. Windows follows the same copy/preserve procedure in Explorer or PowerShell. Both v1 and v2 snapshots load without modification. The next successful changed save writes v2, preserving the previous file in the ordinary one-generation backup. Wood/Leaves use new v2 IDs 4/5; IDs 1–3 retain their meaning. Older clients cannot read v2: keep an external copy before editing legacy worlds if you need rollback. Unknown versions are rejected safely.

For an isolated Void Linux acceptance test:

```sh
git fetch origin
git switch feat/slice-5-seeded-worldgen
test_root="$(mktemp -d /tmp/rustcraft-slice5.XXXXXX)"
RUSTCRAFT_DATA_DIR="$test_root/seed-42" RUSTCRAFT_WORLD_SEED=42 cargo run --locked -p rustcraft
# A second, independent world:
RUSTCRAFT_DATA_DIR="$test_root/seed-2026" RUSTCRAFT_WORLD_SEED=2026 cargo run --locked -p rustcraft
```

Keep `test_root` in this shell, or record the printed save paths. These fresh directories isolate the tests from real worlds. Re-run the same seed/directory command to resume; a seed override (even an invalid one) is ignored once a valid world exists. Place several materials, remove some terrain and press F5. Wait for `Saved`, close and run the same command again: the title should say `Loaded` and the edits should remain visible and solid. Make another edit, wait at least 30 seconds to test autosave; then edit once more and close without F5 to test final saving. Restart after each case. Check mouse look, walking/jumping, placement rejection inside the body and edits around chunk boundaries. See [the complete manual checks](docs/PLATFORMS.md#slice-5-manual-test).

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

Run only the engine-independent voxel and physics tests with `cargo test --locked -p voxel_core`. Client tests exercise actual Bevy scene/input/fixed schedules without initializing a GPU or window, including frame-rate comparisons at 30/60/144 FPS. They do not substitute for real input/rendering checks.

The [validation report](docs/VALIDATION.md) records the executed checks and remaining platform coverage.

[Manual acceptance checks](docs/PLATFORMS.md#slice-5-manual-test) cover save/restart, the visible scene, pointer capture/release, focus changes and platform behavior. CI checks Linux and Windows compilation, formatting, Clippy and unit tests; it does not establish GUI compatibility on Void or Windows.

## Licensing and development policy

Project-authored source code and documentation are **MIT OR Apache-2.0**; see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Third-party dependencies and artwork keep their own licenses. See [dependency and asset inventory](docs/THIRD_PARTY.md).

The source repository is public; keep `main` stable. Develop features on topic branches and submit pull requests for review. Publishing packaged game releases is a separate decision. No proprietary game source, textures, sounds or logos are included.
