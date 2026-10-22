# RustCraft

**RustCraft** is a public, experimental Minecraft-inspired voxel sandbox written in Rust. The current v0.1 prototype displays a finite, procedurally generated block world with a first-person **walking player**, gravity, jumping, voxel collision and editable blocks. The long-term goal is a Classic-style editable world, followed by survival and original sandbox mechanics.

RustCraft is an independent project, not a Minecraft source port, and is not affiliated with or endorsed by Mojang or Microsoft.

## Current prototype

- Deterministic seeded terrain, rolling hills/valleys, a walkable cave network with surface entrance, and blocky trees.
- 16³ chunks streamed in a **65,536×65,536×48** new-world rollout with local generation and regional saves; configurable load radius, bounded background generation/meshing and cache eviction.
- Grass, dirt, stone, wood and opaque solid leaves represented by original vertex colors; no downloaded game assets.
- At most one surface mesh per rendered chunk; faces shared by solid blocks are removed, including at chunk boundaries.
- Mouse look, yaw-relative WASD walking, sprinting, grounded jumping and safe respawn.
- Fixed 60-Hz engine-independent AABB physics, wall sliding and interpolated camera movement.
- Precise five-block targeting, visible target outline, click-to-break/place and 1–5 block selection.
- Engine-independent raycasts and validated edits; authoritative blocks change immediately and connected dirty meshes update together off-thread.
- Versioned legacy snapshots / new-world identity and edit overlays, F5 saving, 30-second dirty-only autosave and final save on normal window close.
- Checksummed files, recoverable replacement with a previous-generation backup and an exclusive world lock.
- Coordinate, collision, targeting, mutation and rendering regression tests.

This is **v0.1 / slice 7**, not the complete v0.1 milestone. **World edits survive restart.** Further coordinate expansion, richer world generation and a full inventory/hotbar are still future work. Walking is the default and there is no debug-flight mode. One-block terraces require jumping; automatic step-up is not implemented. See [roadmap](docs/ROADMAP.md) and [architecture](docs/ARCHITECTURE.md).

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

New regional worlds permit edits within **X/Z [-32768,32768), Y [-16,32)**. Legacy v3 worlds retain **X/Z [-64,64)**; v1/v2 snapshots retain **X/Z [-32,32)** with the same Y bounds. Outside the corresponding box, edits are rejected. Pending chunks temporarily block collision/rays and reject edits until ready; they are never treated as a hole in the floor. R preserves your edits and finds clear sky-exposed ground near the starting area, avoiding tree tops and unsupported columns. Loading uses the same search; player position, velocity and selected slot are not saved.

Movement speed is independent of diagonal input. Mouse sensitivity does not depend on frame time. The window title also lists the essential controls.

## New worlds and seeds

`RUSTCRAFT_WORLD_SEED` accepts a decimal unsigned 64-bit integer (0 through 18446744073709551615). The deterministic default is **2026**. New regional worlds use **generator version 3**, saved as part of their identity. Generators v1/v2 remain frozen for existing identities; existing worlds never select a new generator. It is read **only when no primary or backup exists**; changing it never changes a loaded world. Invalid seeds stop creation with a clear error. To explore another seed, use a different, preferably fresh absolute `RUSTCRAFT_DATA_DIR`; never delete your real save to switch seeds.

Terrain uses coherent height variation with grass/dirt/stone layers. Caves connect across chunks and have a stair-stepped surface entrance near x=-10, z=20, descending toward the east/west tunnel near z=-10. Trees use supported 4–5-block trunks and simple crowns. Wood and leaves are both **solid and opaque**; there is no leaf decay, transparency, water, biomes, ore distribution or lighting propagation. The outer terrain is a finite cut-off edge; falling below the world respawns you safely. The historical demo remains a library test fixture.

New v4 worlds save **seed + generator version + world identity** in a manifest and all edits in regional overlays, including explicit Air. Unmodified chunks reproduce locally from that identity. Loaded chunks and clean region maps are disposable caches; unsaved region maps remain pinned in RAM until successfully saved. v1/v2 saves remain full snapshots with no seed metadata and never gain generated terrain. The [generation contract](docs/WORLDGEN.md) explains deterministic arithmetic, limits and future extension.

## Streaming and compatibility

A fresh data directory creates a **v4 regional world** with generator v3 and
X/Z [-32768,32768), Y [-16,32). There is no map-sized startup allocation.
`RUSTCRAFT_LOAD_RADIUS=1`, `2` (default) or `3` selects a radius independently of
world extent: at most 75 chunks by default, 147 at radius 3. Distant voxel data,
meshes, GPU assets and clean regional overlays unload. Spawn is not pinned.

One background terrain worker loads regional edits, locally generates new columns
and builds meshes. Cached edits use coalesced connected dirty batches; nearby
changes precede new loading, and motion favors forward work within near rings.
Current local dependency tokens prevent stale/partial seam publication without
rejecting unrelated work. Physics sees edits immediately; presentation may lag
by a worker job. Pending/failed chunks are safe solid barriers and reject edits.

Physics and targeting keep f64 world coordinates. A floating render origin keeps
meshes, camera and bitmap feedback precise at distant positions. The title reports
world position, resident/pending/dirty work, seed/version and radius. R/void reloads
an evicted origin and waits before finding clear ground from actual saved edits.

**Existing v1/v2 worlds remain finite 64×64 full snapshots; existing v3 worlds
remain 128×128 with generator v2.** They keep their exact blocks, removals and
saved identities. No automatic migration/expansion exists. A fresh directory is
required for the larger mode. `RUSTCRAFT_START_CHUNK=x,z` optionally starts a v4
session in a distant chunk; it is ignored for historical modes. R still returns
to origin, and player position is not persisted.

## Saving, loading and recovery

Data locations stay platform-specific: Linux uses an absolute XDG_DATA_HOME under
`rustcraft`, otherwise `$HOME/.local/share/rustcraft`; Windows uses LOCALAPPDATA
under `RustCraft`, falling back to APPDATA. Override either with an **absolute
RUSTCRAFT_DATA_DIR directory**. The actual `world.rcw` path is logged at startup.

F5, 30-second real-time autosave and normal-close saving remain available. New
worlds save their identity even before edits. Four dirty regions request an early
pressure save. At eight dirty regions, changes in another clean region are
rejected until saving succeeds; dirty data is retained while its graphics unload.
The panel shows yellow `SAVING...`, green `SAVED`, red `SAVE ERROR - F5 RETRY` or
`LOAD ERROR - F5 RETRY`. A final-save error keeps the window open for retry.

For v4, `world.rcw` is a fixed **64-byte identity manifest**, not the whole world.
Only dirty 64×64×48 region overlays are saved under sharded `regions/` paths.
They include explicit Air and validate identity, coordinates, sizes and checksums.
Each changed region uses synced temporary/backup/rename replacement with one
`.rcw.bak` previous generation. Saving is atomic **per file**, not one transaction
across all regions. Partial failures retain every dirty in-memory region for
retry; identical retries preserve the older backup. The unchanged manifest does
not rotate on each save. See [the full byte/failure contract](docs/SAVE_FORMAT.md).

Back up the **entire data directory with the game closed**. Copying only the v4
manifest does not back up regional changes. Never delete a live `world.lock`;
the OS lock prevents cooperating instances overwriting a world. Unknown/corrupt
files are preserved, never automatically replaced with generated terrain or a
backup. Region errors stop spawn loading or leave a visible safe pending frontier.

For recovery, close all instances, copy the entire directory externally, preserve
the damaged file, then explicitly restore its matching valid backup. A v4 region
needs that region's backup with matching world identity; a missing/damaged v4
manifest needs its matching external backup. Do not overwrite a `.damaged` copy
or remove regional files to make an error disappear. Backup restoration can lose
newer edits. Filesystem/hardware failure and forced termination can still lose
changes since successful saves; no per-click crash journal or absolute power-loss
guarantee exists. v1/v2/v3 retain their historical full-file recovery rules.

## Safe Void Linux test with Fish

Use a fresh directory and keep this shell/path for restarts:

```fish
git fetch origin
git switch feat/slice-7-large-world-streaming
set test_dir (mktemp -d /tmp/rustcraft-slice7.XXXXXX)
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=2 cargo run --locked -p rustcraft
```

After normal close, reuse that same directory at a distant position:

```fish
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_START_CHUNK="1000,-1000" RUSTCRAFT_LOAD_RADIUS=2 cargo run --locked -p rustcraft
```

This starts near X=16008, Z=-15992 on clear actual ground. Break/place across
chunk and region seams, travel away and return before saving, then F5 and wait for
`SAVED`. Close and repeat the same distant-start command to check restoration.
R returns to origin; repeat with `-1000,1000` and a different **fresh directory**
for another seed. Existing saves ignore seed overrides. These explicit test paths
do not touch normal saves. See [native acceptance](docs/PLATFORMS.md#slice-7-manual-test-fish).

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

Run only the engine-independent voxel and physics tests with `cargo test --locked -p voxel_core`. Client tests exercise actual Bevy scene/input/fixed schedules without initializing a GPU or window, including frame-rate comparisons at 30/60/144 FPS. They do not substitute for real input/rendering checks.

The [validation report](docs/VALIDATION.md) records the executed checks and remaining platform coverage.

[Manual acceptance checks](docs/PLATFORMS.md#slice-7-manual-test-fish) cover save/restart, the visible scene, pointer capture/release, focus changes and platform behavior. CI checks Linux and Windows compilation, formatting, Clippy and unit tests; it does not establish GUI compatibility on Void or Windows.

## Licensing and development policy

Project-authored source code and documentation are **MIT OR Apache-2.0**; see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Third-party dependencies and artwork keep their own licenses. See [dependency and asset inventory](docs/THIRD_PARTY.md).

The source repository is public; keep `main` stable. Develop features on topic branches and submit pull requests for review. Publishing packaged game releases is a separate decision. No proprietary game source, textures, sounds or logos are included.
