# RustCraft platform support and development setup

Status: **slice 4 platform requirements and acceptance procedure**. Linux cloud compilation and headless tests are distinct from native Void/Windows play-testing. See the PR validation report for commands actually executed. The pinned Rust toolchain is 1.99.1 and Bevy is 0.20.0.

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
