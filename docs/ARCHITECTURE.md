# RustCraft architecture

Status: **v0.1 / slice 2 implemented**. Later gameplay systems remain proposals.

## Current workspace

```text
Cargo.toml             workspace and pinned Bevy dependency
Cargo.lock             committed application dependency graph
rust-toolchain.toml    Rust 1.99.1, rustfmt and Clippy
crates/voxel_core/      storage, coordinates, fixed world, surface buffers and player physics
client/                Bevy application, rendering, fixed-step player adapter and camera/input
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

## Player physics (`voxel_core::physics`)

The controller has no Bevy or external dependencies. `Player` holds feet-centre position, velocity and grounded state as plain data. `MovementInput` is one tick's world-space horizontal direction, sprint flag and jump request; `from_yaw` converts local keyboard axes. `Player::tick` always advances **1/60 second**. No camera transform is authoritative for physics.

The player's AABB is 0.6 blocks wide/deep and 1.8 high, with eye height 1.62. Position/velocity and collision calculations use f64; rendering converts to f32 only at the presentation boundary. Units are blocks/metres and seconds, Y up. These are independently chosen tuning values, not a claim of Minecraft physics parity.

`PhysicsConfig::default` is the single tuning location:

| Parameter | Value |
| --- | --- |
| Walking / sprint speed | 4.3 / 5.6 blocks/s |
| Ground acceleration / braking | 55 / 60 blocks/s² |
| Air acceleration with directional input | 12 blocks/s² |
| Gravity | 24 blocks/s² |
| Jump speed | 8.5 blocks/s |
| Terminal fall speed | 55 blocks/s |

Horizontal input is normalized, with acceleration toward the target velocity and fast grounded braking. In air, releasing input retains horizontal momentum; directional input supplies bounded air control. Ground support is checked before jumping and after movement. The tiny support probe is 0.000001 blocks; it does not snap to lower terraces or give a gameplay-sized coyote interval. Jump commands are ignored in air.

Collision sweeps the complete AABB movement on Y, X, then Z against `World::block`, clipping each displacement to the nearest solid face. Swept ranges include intermediate blocks, preventing destination-only tunneling. Other axes use open overlap intervals with a 1e-8 contact tolerance, allowing contact with floors/walls without snagging parallel motion. The fixed axis order gives wall sliding and repeatable resolution; it can bias exact corner contacts. There are no per-tick allocations, mesh collision structures or whole-world scans: only the nearby swept voxel ranges are queried. Missing chunks remain air, including beyond the finite island.

Contracts: configuration values and player state must be finite, dimensions/speeds positive, and spawn/reset must not intersect solids. This is a static-voxel kinematic controller, not a general rigid-body/depenetration solver. Future block placement must reject edits intersecting the player. Replaying the same commands is regression-tested, but cross-platform bitwise floating-point lockstep is not promised.

## Fixed simulation, input and camera adapters

`client/player.rs` retains `PlayerState` (current body plus previous fixed position), `PlayerSettings` and `ControllerInput` as resources. Physics runs in `FixedUpdate` at 60 Hz. Bevy's normal bounded time catch-up handles slow render frames; excess wall time after a long stall is not treated as a single large physics step.

`client/camera.rs` samples buttons and relative mouse motion **before** the fixed loop, after Bevy updates input. Jump/reset edges are latched across render frames without a physics tick, consumed once, and cleared when controls pause. Holding Space never repeats a jump. Mouse sensitivity is independent of delta time; pitch is clamped and movement follows yaw only. Hover motion on the capture frame is ignored to prevent a view jump.

After the fixed loop, the camera interpolates previous/current feet positions using the fixed clock's overstep fraction, adds eye height and applies the immediately updated view rotation. This smooths translation at differing frame rates at the cost of one physics tick of latency. The simulation never reads the interpolated transform.

Capture starts on click; Escape or focus loss releases it. Re-focusing requires another click. In this local single-player prototype, release pauses the player simulation (including falling), preserving velocity until resumed; pending edges are cleared. A future authoritative multiplayer/server simulation must not inherit this local pause policy.

Spawn is feet `[0.5, 4.0, 10.5]`, above the low terrace and facing -Z toward the island. R resets body, previous position, velocity, pending jump and view; falling below Y=-32 resets the body automatically. Resetting both positions prevents interpolation across the map. There is no debug-flying mode, automatic step-up, collision crouch, block editing or save system.

## Platform and dependency choices

Only the Bevy subsystems needed for 3D rendering, desktop windows and keyboard/mouse input are enabled, including **both X11 and Wayland**. Audio, gamepad, UI/font, scene import and external asset formats are not enabled by this application. Bevy/wgpu retains its native graphics backend selection; the client does not force Vulkan on every platform. See `docs/PLATFORMS.md` and `docs/THIRD_PARTY.md`.

## Next boundaries

Add block targeting and interaction on the existing world/physics boundary before richer gameplay. Seeded world generation and persistence should become separate modules when introduced. A future versioned save format needs explicit block IDs and migration policy. Do not persist ECS entities or derive the simulation model from mesh geometry. Networking, server authority, mods and a general item registry remain deferred.
