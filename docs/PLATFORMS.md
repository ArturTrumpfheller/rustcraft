# RustCraft platform support and development setup

Status: **platform requirements**, not a claim that the game already runs. The first Rust implementation must document which configurations were actually built and play-tested.

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
