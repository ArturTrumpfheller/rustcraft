# RustCraft platform support and development setup

Status: **slice 2 platform requirements and acceptance procedure**. Linux cloud compilation and headless tests are distinct from native Void/Windows play-testing. See the PR validation report for commands actually executed. The pinned Rust toolchain is 1.99.1 and Bevy is 0.20.0.

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
