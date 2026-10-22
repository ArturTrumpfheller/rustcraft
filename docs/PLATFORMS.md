# RustCraft platform support and development setup

Status: **Slice 8 texture acceptance**. The owner accepted Slices 1–7 on Void. Linux cloud compilation and headless tests are distinct from native Void/Windows play-testing. See the PR validation report for commands actually executed. The pinned Rust toolchain is 1.99.1 and Bevy is 0.20.0.

## Supported platforms and targets

| Target | Priority | Build target | Notes |
| --- | --- | --- | --- |
| Void Linux **glibc**, x86-64 | Primary | `x86_64-unknown-linux-gnu` | Owner's main desktop and hands-on runtime test platform |
| Windows 10/11, x86-64 | Primary | `x86_64-pc-windows-msvc` | Native Windows CI build/test; a human must verify the GUI |
| Other modern glibc Linux distros, x86-64 | Best effort | `x86_64-unknown-linux-gnu` | CI uses Ubuntu; different distros may need runtime libraries |
| Void Linux **musl**, x86-64 | Experimental | `x86_64-unknown-linux-musl` | Requires its own build/test, especially native libraries and audio |
| Older GPUs lacking current Vulkan/DX12 capabilities | Not guaranteed | N/A | Do not claim compatibility based on a successful compilation |

Do **not** assume a glibc-linked executable will run on musl, or that a Windows executable will run natively on Linux. Distribute/build distinct platform artifacts. All targets use the same cross-platform game logic.

## Rendering, windowing and input

- Use Bevy/wgpu's supported platform-specific graphics backend discovery; do not require Windows users to install Vulkan solely to start the game.
- Support **X11 and Wayland** on Linux, including reliable mouse capture/release and keyboard handling on both. Do not assume an X server or a particular desktop environment.
- On Linux, AMD GPUs require appropriate Mesa Vulkan drivers and the Vulkan loader. On Windows, DX12 is generally available with supported drivers; actual backend selection is the renderer's responsibility.
- Native visual runtime behavior cannot be established by headless CI alone; test startup, cursor lock, fullscreen/windowed mode, resize and rendering manually on Void and Windows.

## Void Linux glibc development

The upstream Bevy [Linux dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md) document the essential Void packages:

```sh
sudo xbps-install -S pkgconf alsa-lib-devel libX11-devel eudev-libudev-devel
```

When building with Wayland support, also ensure the development libraries are available:

```sh
sudo xbps-install -S wayland-devel libxkbcommon-devel
```

For AMD Vulkan, the official [Void graphics driver handbook](https://docs.voidlinux.org/config/graphical-session/graphics-drivers/amd.html) recommends:

```sh
sudo xbps-install -S vulkan-loader mesa-vulkan-radeon
```

Rust/Cargo and a working C/C++ build toolchain are also needed (e.g. rustup plus Void build tools). Package selections may vary by Bevy version and enabled features. You can check the graphics installation with `vulkaninfo` if the Vulkan tools are installed.

A working Vulkan driver is important for the owner's modern Radeon graphics card; performance and correct rendering still need actual testing.

## Windows development

Use an up-to-date Rust toolchain with the MSVC target and the Microsoft Visual C++ Build Tools (Windows SDK / C++ build tools) according to the [official Bevy setup instructions](https://bevy.org/learn/quick-start/getting-started/setup/). The Windows build target is `x86_64-pc-windows-msvc`. Building from a Linux machine for that target is *not* assumed to work without a suitable cross toolchain; Windows CI is the default Windows compilation check.

## File paths, saves and assets

- Do not hardcode Linux-only `/home/...`, Windows-only drive letters, separators or case-sensitive file paths.
- Use platform-appropriate application data directories for user saves/settings (e.g. XDG data paths on Linux and AppData locations on Windows) and a versioned save format.
- Bundle or resolve resources relative to the application consistently; do not depend on whichever folder happens to be the user's working directory.
- Avoid dependencies on Bash, systemd, glibc-specific extensions or other system facilities in game logic.

## CI requirements

The Rust workflow checks both **ubuntu-latest** and **windows-latest** for supported workspace builds, Clippy and tests when Cargo.toml exists. Linux jobs install required native libraries first. It does **not** prove the game window actually opens or the renderer works on every machine.

## Early release checklist

- [ ] Void Linux glibc: `cargo run` and manual 3D scene/input test
- [ ] Windows x86-64: build and manual 3D scene/input test
- [ ] Wayland and X11 smoke tests, where available
- [ ] Document required drivers and save/config locations
- [ ] Package separate executable builds and include compatible assets/licenses
- [ ] Verify all supported GPU driver/API assumptions against real hardware

## Historical slice 1 manual test

On a real graphical desktop, run `cargo run --locked -p rustcraft` from the repository root:

1. Confirm a sky background, a continuous terraced grass/dirt/stone island and a stone landmark. Inspect from above and below; shared chunk boundaries should not expose internal faces.
2. Click inside the window. Move the mouse and use WASD; verify diagonal speed, pitch limits, Space/left Ctrl elevation and left Shift acceleration. R restores the overview.
3. Press Escape. Confirm a visible free cursor and that keyboard/mouse no longer move the camera. Click again to recapture.
4. Switch to another window. Confirm capture releases, and returning requires another click. Test resizing and closing the window.
5. Repeat in an X11 session and a Wayland session on Void glibc, then on Windows 10/11 with the native MSVC toolchain. Record GPU/driver and backend in the test report.

No collision, block editing or saves are expected in this slice. Passing headless ECS tests is not evidence that native pointer capture, presentation or graphics drivers work. Windows compilation is covered by the CI matrix when it runs; it is not a Linux cross-compilation claim.

The minimal Bevy feature selection does not enable audio or gamepads yet, so ALSA and udev development libraries are not required by this slice. The earlier broader dependency list remains useful for future engine features. No custom Vulkan-only setting is imposed by the client.

## Slice 2 manual test

Use `cargo run --locked -p rustcraft` in a real graphical session. The build dependencies and pinned toolchain above remain unchanged.

1. Confirm the original four-chunk scene and stone landmark still render. Click to capture the mouse; the player falls a short distance onto the low terrace. Looking around must not move the body or alter walking speed.
2. Walk with WASD; hold left Shift to sprint. Check smooth starts/stops, equal diagonal speed, and mouse pitch limits at both high and low FPS. Left Ctrl no longer flies down; there is no flying mode.
3. Walk toward a terrace without jumping: the solid side must stop you. Walk diagonally into walls/landmarks and check sliding. Jump with Space to climb a one-block terrace. Holding Space must not bounce repeatedly; pressing again in air must not create another jump.
4. Stand still, cross x=0/z=0 chunk seams, and explore negative x/z. Look for shaking, catches or floor penetration. Core tests also cover negative -16 boundaries, ceilings and high-speed sweeps that the current open-air scene cannot conveniently reproduce manually.
5. Walk off the island. Gravity should continue until automatic respawn below Y=-32; R while captured also restores the safe spawn and clears velocity. Neither reset should animate the camera across the map.
6. Escape must release/show the cursor and pause local walking/gravity. Focus loss does the same. Returning requires a new click, without a view jump or stale queued jump. Resize the window and close it normally.
7. Repeat on Void glibc X11 and Wayland, then Windows 10/11 MSVC. Record GPU/driver/backend and input issues. Slice 1's successful native test does not substitute for this new physics/input acceptance.

CI continues to build/lint/test Ubuntu and Windows. Local software-rendering checks do not establish physical GPU performance or native Wayland/Windows cursor behavior. Block edits, inventory and persistence remain outside slice 2.

## Slice 3 manual test

Build/run `cargo run --locked -p rustcraft` from `feat/slice-3-block-interaction` using the unchanged Void/native dependency setup above. **Edits are not saved; closing the game loses them.**

1. Click to capture; that first click must not mine/place. Aim down at nearby blocks: the crosshair and black target outline should agree. No target should appear beyond five blocks. Change view/FPS and verify the highlight follows the displayed eye.
2. Left click repeatedly to break, right click to place on the face. Try top/side/underside faces, negative coordinates and x=0/z=0 chunk seams. Newly hidden/exposed neighboring faces must update immediately, with no seam holes.
3. Use 1/2/3 for grass/dirt/stone. Check the swatches/selected frame and title. A selection is not an inventory; no items are consumed. Empty clicks do nothing; holding a button does not auto-repeat.
4. Try placing into feet, head or body corners: it must be rejected. Mine the supporting floor and confirm falling/collision uses the changed data. Walk/jump on newly placed blocks. Try extending outside the initial island into a missing chunk, then remove the new cells again; empty geometry should be safe.
5. Walk, sprint, jump, reset and resize the window. Build at the old spawn from a distance, then R: the player should recover in clear space above, while edits remain. Editable bounds are X/Z [-32,32), Y [-16,32); placement beyond them is ignored.
6. Escape/focus loss must release capture and prevent edits. Right click while released must do nothing. Returning requires a fresh left capture click without mining. R and simultaneous click must not edit the old position.
7. Close/restart and confirm the original four-chunk scene returns. Repeat on Void glibc X11/Wayland and Windows MSVC, recording GPU/driver/backend and any cursor/editing issues. Software X11 coverage does not establish these native GUI configurations.

## Slice 4 manual test

Use the current slice-4 branch and unchanged native build dependencies. On Void, isolate test data from real worlds:

```sh
RUSTCRAFT_DATA_DIR="$HOME/.local/share/rustcraft-slice4-test" cargo run --locked -p rustcraft
```

The normal Linux location is `${XDG_DATA_HOME}/rustcraft` when absolute, otherwise `$HOME/.local/share/rustcraft`. Windows uses `%LOCALAPPDATA%\RustCraft`, falling back to `%APPDATA%\RustCraft`; in PowerShell set `$env:RUSTCRAFT_DATA_DIR = Join-Path $env:LOCALAPPDATA 'RustCraft-Slice4-Test'` for isolation. The absolute override is a directory, not a filename. The startup log shows the resolved `world.rcw` path.

1. With a new directory, confirm `New demo`, the original terrain and working capture/mouse/walking/sprint/jump. F5 without edits should not create `world.rcw`. The lock file alone is normal.
2. Add Grass/Dirt/Stone, replace terrain through break/place, remove several original blocks, cross x=0/z=0 and positive/negative 16-block seams, and extend into previously missing chunks. Check immediate rendering and safe placement. Press F5, wait for `Saved`, close and relaunch using exactly the same data directory. Confirm `Loaded`, all edits and correct solid collision. Empty spaces must stay empty; R must preserve edits.
3. Change blocks again, release the mouse and wait at least 30 seconds. Confirm `Saved` and updated file. Restart and verify these changes. Idle intervals must not continually rewrite the file/backup.
4. Edit once more and immediately close normally without F5. Wait for process exit, restart and verify the last edit. Build over the original spawn from a distance, save and restart: the player must start in clear space above the new blocks.
5. While the game runs, starting a second process with the same directory should fail with a lock error. After clean exit it should open normally. Never delete the lock file while a session runs.
6. With all instances closed, copy the entire test directory. Only in the disposable copy, truncate/damage `world.rcw`; launching with that copy must stop with an error and preserve it. Restore `world.rcw.bak` by following README's preserve/copy recovery instructions. A backup appears after the second successful changed save; it restores the previous generation.
7. Test resize, Escape, focus changes, targeting and player-overlap rejection again. Repeat on native Void X11/Wayland and Windows, recording GPU/driver/backend. CI tests file replacement/locking on both OSes, but it does not test desktop GUI input or real power loss.

Do not force-kill during saves. On save failure the game stays open and dirty; fix the problem and use F5 or close again. Player position is deliberately reset, not persisted. No multi-world menu, migration, cloud save or inventory persistence is included. See [format and durability limits](SAVE_FORMAT.md).

## Slice 5 manual test

Use the slice-5 branch and the same Void glibc / Windows MSVC dependencies. Native X11/Wayland and Windows GUI tests are still required. **Use fresh disposable directories**, never an actual world for fault injection.

```sh
git fetch origin
git switch feat/slice-5-seeded-worldgen
test_root="$(mktemp -d /tmp/rustcraft-slice5.XXXXXX)"
RUSTCRAFT_DATA_DIR="$test_root/seed-42" RUSTCRAFT_WORLD_SEED=42 cargo run --locked -p rustcraft
RUSTCRAFT_DATA_DIR="$test_root/seed-2026" RUSTCRAFT_WORLD_SEED=2026 cargo run --locked -p rustcraft
```

Keep the shell/variable or record the printed paths to resume the same test worlds. For persistent tests outside /tmp, choose a new absolute directory under your home, without reusing your real `rustcraft` data directory. Windows PowerShell can use `$env:RUSTCRAFT_DATA_DIR = Join-Path $env:TEMP ('RustCraft-' + [guid]::NewGuid())` and `$env:RUSTCRAFT_WORLD_SEED = '42'` before `cargo run --locked -p rustcraft`.

1. Start a fresh world with seed 42. Confirm creation logs (seed/version/timings), coherent terrain, recognizable trees and the in-game status panel. Wait for green `SAVED` **without editing**. Close/restart in the same directory: confirm `LOADED` and the same landscape. Change the override seed in that same directory; it must still load the exact saved world.
2. Start seed 2026 in a separate fresh directory. Terrain should differ; starting a second fresh directory with seed 42 should reproduce the first seed's landscape. Invalid/overflowing seeds should prevent new-world creation, but should not prevent loading an existing valid snapshot.
3. Capture/look, walk, sprint and jump across hills and x/z chunk seams. Find the access ramp near x=-10,z=20 and explore toward the cave around z=-10. Check headroom, collision, descending/jumping back up and wider chambers; finite world edges are intentional. R and falling below Y=-32 must choose safe ground from this world, not fixed demo height or tree tops.
4. Select 1–5, break/place Wood/Leaves around chunk boundaries, and try player-overlap placement. Both new blocks are solid/opaque. Five swatches and selected frame should track the keys. Confirm immediate mesh updates and standing on new blocks.
5. F5: observe yellow saving then green success. Release the cursor and verify the panel remains visible. Edit and wait for the 30-second autosave check, then restart. Make another edit and close normally without F5; verify it after restart. Clean intervals must not repeatedly change the save/backup.
6. With the game closed, make a disposable directory copy of a historical v1 world. Load it with a different seed; its blocks/deletions and file bytes must remain unchanged. After an edit/F5, expect v2 with exact prior v1 bytes in `world.rcw.bak`. Keep an external copy if older-client rollback matters: the ordinary backup is only one generation. Do not test upgrades on the only copy of valuable constructions.
7. In a disposable world, test a save-path failure and confirm persistent red `SAVE ERROR - F5 RETRY`, intact prior primary and no false success. Fix the cause and retry F5; the panel should turn green. Normal close on save failure must stay open for retry. Follow README's preserve/copy recovery procedure for corrupt files; never silently start over on invalid data.
8. Repeat on native Void X11/Wayland and Windows. Record driver/GPU/backend, pointer capture/focus/resize, cave traversal and frame performance. Software X11/CI coverage does not verify physical GPU performance or compositor behavior.

## Slice 6 manual test

Use the same Void glibc / Windows MSVC prerequisites. Choose **fresh absolute directories** to try the new v3 mode; actual v1/v2 constructions remain legacy finite worlds and are not expanded.

```sh
git fetch origin
git switch feat/slice-6-chunk-streaming
test_root="$(mktemp -d /tmp/rustcraft-slice6.XXXXXX)"
RUSTCRAFT_DATA_DIR="$test_root/streamed" RUSTCRAFT_WORLD_SEED=42 \
  RUSTCRAFT_LOAD_RADIUS=2 cargo run --locked -p rustcraft
```

Keep this shell/path for restarts. Windows PowerShell can use `$env:RUSTCRAFT_DATA_DIR = Join-Path $env:TEMP ('RustCraft-Slice6-' + [guid]::NewGuid())`, `$env:RUSTCRAFT_WORLD_SEED = '42'`, `$env:RUSTCRAFT_LOAD_RADIUS = '2'`. Use radius 1 for a smaller cache / more frequent load boundaries, or 3 for a larger view; values outside 1–3 fail clearly in streamed mode.

1. Confirm streamed mode, saved identity seed/gen2, an initial three-chunk spawn column and subsequent pending count dropping. The world spans 128×128 blocks. The title shows resident chunks/pending columns/seed/version/radius; the in-game panel still reports saving/success/error. Unedited v3 creation writes one 40-byte identity file, then remains clean while exploring.
2. Capture/look, walk/sprint/jump across x/z chunk boundaries and beyond the former ±32 limits. Old columns should unload and new columns load without unbounded mesh/asset growth. Try returning along the same route. Newly sampled terrain/trees/caves must match and seams must have consistent faces. A pending frontier blocks movement/editing safely until ready; do not expect infinite terrain beyond ±64.
3. Place Wood/Leaves and break generated blocks in several columns, including negative coordinates. Travel far enough to evict those columns, then return **before saving**: edits must still exist in the in-memory overlay. F5, wait for green `SAVED`, close/restart with the same data directory and verify all edits again. Repeat with autosave and with normal close without F5.
4. Change the environment seed for that same v3 path: it must use the stored identity. Move far enough that the origin column leaves the radius: no distant spawn island should remain visible. R/void recovery must request that evicted column, pause safely until it is ready and resolve edited spawn ground without an interpolated flight. Check mouse release/focus/resize, slots 1–5, rejected player-body placement and collision on placed blocks. Pending/overlap teleport edge cases are covered by headless ECS tests; there is no public teleport command.
5. In a disposable copy of a historical v1/v2 directory, confirm `Legacy finite 64x64`. Explore its exact saved terrain and empty space; no generation fills missing chunks. Idle/F5 without edits must not change primary bytes. There is no legacy-to-v3 migration in this slice. Keep external backups before deliberate legacy edits if older-client rollback matters.
6. Test failures only in disposable directories. On an unwritable/obstructed backup path, edits must remain dirty, including after eviction; normal close stays open and F5 can retry after fixing the problem. Corrupt/new-version data must stop startup without regeneration/replacement. Follow README's preserve/copy recovery instructions. A forced kill can lose unsaved changes since the last completed save; it is not the save/eviction test protocol.
7. Repeat on native Void X11/Wayland and Windows. Record GPU/driver/backend, actual frame feel, loading stalls/frontier behavior and memory. Cloud software-rendered timings are not desktop GPU guarantees. The world is bounded, edits stay in a bounded central RAM overlay, and there is no per-chunk disk cache or per-edit crash journal yet.

### PR #12 native-review retest

Continue on `feat/slice-6-chunk-streaming` and use a fresh test directory (or an
external copy of a previous test world with all instances closed). Keep the same
absolute `RUSTCRAFT_DATA_DIR` for restarting; choose another directory for another
seed. Never diagnose by deleting a real save or its lock while running.

- At radius 1, walk several columns away and look back: spawn meshes must disappear.
  Return normally and then repeat using R and a fall below Y=-32. Blocks built or
  removed near spawn must remain, and respawn must choose clear edited ground.
- Place two blocks that touch diagonally and push diagonally into the shared
  corner on level ground and terraces. Repeat near x/z=0 and ±16, slide away along
  a wall, then jump. Check both the body and camera for repeated backtracking.
- Mine and rebuild quickly along a positive and negative chunk seam. Check both
  faces, collision, slots 1–5 and rejected body-overlap placement. Repeat many
  cycles, leave until eviction, return before F5, then save/close/restart.
- Logs at `RUST_LOG=info` expose load jobs, mesh targets, discards, peak queue,
  resident/mesh counts, logical buffer bytes, worker/install/update maxima,
  dirty latency and a rolling 128-frame p95. Frame intervals include rendering/
  driver scheduling and are not GPU timestamps. Idle frames should do no new
  terrain work; resource counts should return to the same values after cycles.

Record resolution, radius, GPU/driver, debug versus release build and X11/Wayland.
The cloud software-rendering comparison in VALIDATION does not predict native GPU
performance. No world extent, generator identity, save format or texture changes
are introduced by this correction.

## Slice 7 manual test (Fish)

Use the existing Void glibc / Windows MSVC toolchain and drivers; no dependencies
or graphics backend requirements changed. Linux supports X11 and Wayland. With
Fish, choose an isolated absolute path and keep it for restarts:

```fish
git fetch origin
git switch feat/slice-7-large-world-streaming
set test_dir (mktemp -d /tmp/rustcraft-slice7.XXXXXX)
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=2 RUST_LOG=info cargo run --locked -p rustcraft
# After normal close, visit far coordinates in the same world:
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_START_CHUNK="1000,-1000" RUSTCRAFT_LOAD_RADIUS=2 RUST_LOG=info cargo run --locked -p rustcraft
```

1. Confirm generator 3 / regional 65536×65536 mode, identity saved, and the
   resident count settling to 75 chunks at radius 2. The title shows world
   position; rendering/HUD should stay precise near ±16000 and near corner
   chunks `2047,2047` / `-2048,-2048`. Values outside -2048..2047 fail clearly.
2. Explore beyond ±64 on both axes, move repeatedly through column boundaries
   and compare loaded meshes/CPU/region cache counts. New terrain must match on
   return without origin meshes remaining visible. R and falling below Y=-32
   request origin safely. Check cursor capture/release, focus and resizing.
3. Mine/place rapidly across chunk and 64-block region seams. Check both faces,
   collision, target outline and body-overlap rejection. Diagonally touching
   block corners must stay stable while pushing/sliding/jumping. Repeated empty/
   refill cycles must not accumulate terrain assets or queues.
4. Modify distant blocks to Air and add Wood/Leaves. Leave several radii and
   return before F5; changes must survive eviction. Save, wait for green SAVED,
   close and repeat the same distant-start command. Also verify 30-second and
   pressure autosave, and final-close saving without F5.
5. Copy a historical world directory with all instances closed, and point only
   the disposable copy at the client. v1/v2 stays 64×64 and v3/gen2 stays 128×128;
   seed/start overrides must not expand it. Idle/F5 must leave bytes unchanged.
6. Exercise I/O/corruption recovery only in disposable copies. The error names
   the affected file; preserve it and the whole directory before restoring its
   matching backup. Never delete real region files to bypass an error. Dirty
   data must survive failed saves/eviction and final close must cancel until
   successful retry. Eight unsaved regions limit further new-region edits.
7. Logs report worker/load/dirty/update latency, cache/queue/buffer counts and
   rolling frame P50/P95/P99/FPS. Record resolution, driver/GPU, radius, debug/
   release build and X11/Wayland. Compare matched scenes; software Mesa is not
   a physical GPU benchmark. Repeat on Windows with equivalent isolated paths.

Back up **all** v4 files, including `regions/`, with the game closed. The manifest
alone is not a backup. For another seed use a separate fresh directory. The start
chunk is a session preference, not persisted player position; R remains origin.

## Slice 8 texture test (Fish)

The owner accepted Slice 7 on Void. The atlas is embedded and uses the unchanged
native rendering/window backends; Python/PNG support is unnecessary to run/build.
Authoring/CI regeneration uses Python 3's standard library only. Start in an
isolated directory (a fresh directory per seed):

```fish
git fetch origin
git switch feat/slice-8-retro-textures
set test_dir (mktemp -d /tmp/rustcraft-slice8.XXXXXX)
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=2 RUST_LOG=info cargo run --locked -p rustcraft
# After normal close, keep this same Fish shell/path:
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_START_CHUNK="1000,-1000" RUSTCRAFT_LOAD_RADIUS=2 RUST_LOG=info cargo run --locked -p rustcraft
```

1. Inspect all seven tiles close up: turf top; grass lip above earthy sides;
   plain dirt underneath; stone; upright bark; end-grain on both ends; opaque
   leaves. Pixel edges must remain sharp. Distant minification shimmer is a
   documented LOD0 tradeoff, not atlas-edge bleeding. No magenta slot/neighbor
   tile should appear on edges, steep angles, near/far starts or after resizing.
2. Click to capture, look/walk/sprint/jump, press 1–5 and check the five original
   colored swatches/white selection frame, crosshair and black target outline.
   Escape/focus loss must release/pause. Save HUD stays readable while released.
3. Mine/place rapidly at signed X/Y/Z chunk seams and 64-block region boundaries.
   Check exposed faces, shared-face removal, collision, player-overlap rejection
   and no diagonal jitter. Empty/rebuild chunks, travel several radii and return.
   Counts/mesh bytes should settle without repeated image/material growth.
4. Make far Air/Wood/Leaves edits, save F5, wait for green SAVED, close and repeat
   the far command. R reloads origin without pinning distant/spawn meshes. Legacy
   v1/v2/v3 tests must use disposable **copies**, preserving their original modes.
5. Compare against accepted Slice 7 with identical seed/save bytes, camera,
   resolution, radius, build mode and driver. Record frame P50/P95/P99, FPS,
   worker/queue/dirty latency, CPU/RSS and logical mesh bytes. UVs replace colors,
   so vertices shrink 40→32 bytes; one 32-KiB atlas adds a texture sample. Software
   Mesa results do not establish physical AMD GPU, Wayland or Windows behavior.
6. Repeat on native Void X11/Wayland and Windows. No backend is hardcoded by the
   application. Native input, driver/VRAM endurance and distant aliasing remain
   human acceptance checks; CI covers both platform builds and Rust/art tests.

Use a persistent new absolute directory under your home if the test world must
survive `/tmp` cleanup/reboot. Keep/record its path. Back up the **whole closed
world directory**, including `regions/`; a v4 manifest alone is not a backup.
No save format, generator, world extent, block ID or physics change is part of
this visual slice. Exact measurements and test limitations: VALIDATION.
