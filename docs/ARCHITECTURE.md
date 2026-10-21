# RustCraft architecture

Status: **v0.1 / slice 1 implemented**. Later gameplay systems remain proposals.

## Current workspace

```text
Cargo.toml             workspace and pinned Bevy dependency
Cargo.lock             committed application dependency graph
rust-toolchain.toml    Rust 1.99.1, rustfmt and Clippy
crates/voxel_core/      storage, coordinates, fixed world, surface mesh buffers
client/                Bevy application, rendering adapter and camera/input
```

`voxel_core` has no external dependencies and no Bevy types. The `rustcraft` binary depends on it and Bevy 0.20.0. No empty server, simulation, networking or worldgen crates are created.

## Coordinates and storage

Y is up; integer block positions identify unit cells spanning `[x, x+1]`, `[y, y+1]`, `[z, z+1]`. Each chunk contains 16³ blocks in a contiguous vector. X varies fastest, then Z, then Y. `LocalPos` validates all three bounds at construction, so get/set operations cannot accept an invalid local coordinate.

`BlockPos::split` uses Euclidean division/remainders: block -1 belongs to chunk -1 at local 15. Conversion back to block coordinates and neighbor offsets use checked arithmetic. The supported block-coordinate space is signed 32-bit; unrepresentable positions are rejected rather than wrapping.

`Block` is a small closed enum (air, grass, dirt, stone), serving as the first block-ID/material definition. Persistent numeric IDs, an extensible registry and serialization are future work; enum representation is not a save format.

`World` owns chunks in a `BTreeMap`, giving repeatable iteration. Missing chunks are air. `demo_world` creates four adjacent chunks with a fixed terraced layout and a stone landmark. It is not a seeded terrain algorithm.

## Mesh and rendering boundary

`mesh_chunk` reads authoritative world data and emits engine-neutral positions, flat normals, RGBA colors and u32 triangle indices. It emits faces only against air, consulting neighboring chunks at boundaries. Vertices are local to a chunk; the client applies the chunk-origin transform. Counter-clockwise triangle winding and neighbor culling are regression-tested.

The client retains authoritative `World` data in a `VoxelWorld` resource and converts these buffers into Bevy `Mesh` assets, with one entity per chunk and one shared rough white `StandardMaterial`. The original per-block colors come from vertex attributes. A directional sun, ambient light and sky background illuminate the scene. Reinhard tonemapping avoids lookup-table texture assets. No external texture, font or model files are needed.

Construction/meshing is synchronous **once at startup**, bounded to four 16³ chunks (16,384 block slots). There is no repeated mesh generation in the frame loop. This is intentional for the fixed first slice; streaming terrain or edits must introduce bounded/asynchronous mesh jobs and dirty-chunk tracking. Neighbors must also become dirty when a boundary block changes.

## Camera and input

The presentation-only camera is free-flight, not a physics player. Keyboard movement is relative to yaw; pitch never changes horizontal movement speed. Combined movement is normalized, frame time is applied to keyboard motion, and large post-pause deltas are bounded. Relative mouse motion already contains a frame's displacement and is not multiplied by delta time.

Pointer capture is opt-in via left click. Escape and focus loss release capture and stop all camera motion. Re-focusing alone does not recapture. Bevy/winit chooses the supported native pointer mode; the application requests locking and uses winit's confined fallback where applicable. This requires real X11/Wayland/Windows testing. Camera reset recovers from flying through or away from the world.

## Platform and dependency choices

Only the Bevy subsystems needed for 3D rendering, desktop windows and keyboard/mouse input are enabled, including **both X11 and Wayland**. Audio, gamepad, UI/font, scene import and external asset formats are not enabled by this application. Bevy/wgpu retains its native graphics backend selection; the client does not force Vulkan on every platform. See `docs/PLATFORMS.md` and `docs/THIRD_PARTY.md`.

## Next boundaries

Add collision/gravity and interaction as engine-independent rules before richer gameplay. Seeded world generation and persistence should become separate modules when introduced. A future versioned save format needs explicit block IDs and migration policy. Do not persist ECS entities or derive the simulation model from mesh geometry. Networking, server authority, mods and a general item registry remain deferred.
