# RustCraft architecture

Status: **v0.1 / slice 6 bounded streaming implemented**. Later gameplay systems remain proposals.

## Slice 6 design note (written before implementation)

- **Modes and authority:** historical v1/v2 saves retain finite full-snapshot semantics and fully resident data. New worlds use generator v2, a bounded 128×128×48 volume and v3 identity (u64 seed + generator version) with a sparse authoritative edit overlay, including explicit Air. The resident chunk map is a disposable cache; eviction never deletes overlay entries, including unsaved ones. No automatic legacy migration.
- **Lifecycle:** desired columns transition absent → pending → resident → evicted. A modest configurable radius plus a permanently resident spawn column bounds residency. One worker job generates a column plus a one-cell face halo and builds three engine-neutral meshes. The main thread polls completion, installs only still-desired results at the captured edit revision, and releases obsolete CPU/GPU data. A bounded desired set replaces an unbounded task queue. Obsolete in-flight work is discarded, not synchronously joined by frames.
- **Queries and safety:** in-volume pending chunks are an explicit solid barrier for physics/rays and reject edits; outside the finite world is known Air. Spawn uses known resident ground. A pending teleport pauses physics, then checks clearance after load. A small spawn column is prepared before App creation and remains pinned for safe R/void recovery.
- **Meshing:** workers sample deterministic generator + overlay in a halo, independently of which neighbors are resident. Edits invalidate owning and face-neighbor columns; stale revision results cannot overwrite edits. GPU handles are retained for remesh and explicitly removed on eviction. Legacy meshing stays unchanged.
- **Persistence/crash consistency:** v3 saves identity + the complete overlay, not the current resident subset. Empty overlays reproduce unvisited chunks. F5/autosave/final close retain the synced temporary/backup/rename/lock protocol; failure retains the in-memory dirty overlay and cancels close. Unsaved edits survive eviction, but forced termination still loses changes since the last completed save; there is no per-click durable journal. Existing files are never rewritten on load. Unknown versions/identities stop safely.
- **Caps and progression:** default radius 2 (configurable 1–3), at most 150 resident vertical chunks including the spawn pin, one generated/meshed column job plus thin face halo, bounded 128×128 edit volume, and no speculative streaming framework. Overlay memory grows only with edited cells and remains bounded by the world volume. Shared immutable overlay snapshots avoid copying resident data for saves; copy-on-write during active jobs is an explicit bounded prototype tradeoff. Measure timings, peak resident blocks, meshes and queues before claiming performance. Regional disk storage, infinite extent, cancellation within jobs and per-edit journaling remain future work.

## Current workspace

```text
Cargo.toml             workspace and pinned Bevy dependency
Cargo.lock             committed application dependency graph
rust-toolchain.toml    Rust 1.99.1, rustfmt and Clippy
crates/voxel_core/      storage, coordinates, finite worldgen, surface buffers, physics, targeting, edits and persistence
client/                Bevy application, rendering, fixed-step player adapter, camera/input and interaction feedback
```

`voxel_core` has no Bevy types; its only external dependency is `crc32fast` for save checksums. The `rustcraft` binary depends on it and Bevy 0.20.0. No empty server, simulation, networking or worldgen crates are created.

## Coordinates and storage

Y is up; integer block positions identify unit cells spanning `[x, x+1]`, `[y, y+1]`, `[z, z+1]`. Each chunk contains 16³ blocks in a contiguous vector. X varies fastest, then Z, then Y. `LocalPos` validates all three bounds at construction, so get/set operations cannot accept an invalid local coordinate.

`BlockPos::split` uses Euclidean division/remainders: block -1 belongs to chunk -1 at local 15. Conversion back to block coordinates and neighbor offsets use checked arithmetic. The supported block-coordinate space is signed 32-bit; unrepresentable positions are rejected rather than wrapping.

`Block` is a small closed enum (air, grass, dirt, stone, wood, leaves), serving as the first block-ID/material definition. A separate `BuildBlock` selection enum only allows Grass/Dirt/Stone/Wood/Leaves in slots 1–5. Persistence maps materials explicitly to stable IDs (Air=0, Grass=1, Dirt=2, Stone=3, Wood=4, Leaves=5); enum representation is not a save format. An extensible registry remains deferred.

`World` owns a resident chunk cache in a `BTreeMap`. Legacy missing chunks are Air. Streamed in-volume missing chunks are pending: `ready` is false, collision/raycast queries see a solid barrier, and edits are rejected. Outside the mode-specific finite bounds is known Air. Streamed worlds additionally own a stable identity and an Arc-backed sorted edit overlay, including Air. Cache installs/evictions do not advance the edit revision; overlays remain when cache data is removed. `demo_world` creates four adjacent chunks with a fixed terraced layout and a stone landmark. It remains a legacy/test fixture. New client worlds use `worldgen::Generator` with an explicit u64 seed and versioned configuration.

## Mesh and rendering boundary

`mesh_chunk` reads authoritative world data and emits engine-neutral positions, flat normals, RGBA colors and u32 triangle indices. It emits faces only against air, consulting neighboring chunks at boundaries. Vertices are local to a chunk; the client applies the chunk-origin transform. Counter-clockwise triangle winding and neighbor culling are regression-tested.

The client retains authoritative `World` data in a `VoxelWorld` resource and converts these buffers into Bevy `Mesh` assets, with one entity per chunk and one shared rough white `StandardMaterial`. The original per-block colors come from vertex attributes. A directional sun, ambient light and sky background illuminate the scene. Reinhard tonemapping avoids lookup-table texture assets. No external texture, font or model files are needed.

Legacy startup retains at most 48 chunks. Streamed startup prepares only the three chunks of a pinned spawn column; one background worker subsequently generates/meshes desired columns and the main thread installs at most one completed column per frame. Each successful `World::set_block` returns a `BlockChange` containing position and before/after values; equal assignments are no-ops. `Chunk::set` reports whether a validated local cell actually changed. Clearing absent air never creates storage. Solid edits create initially missing chunks lazily. Legacy edits remain **X/Z [-32,32), Y [-16,32)**. New streamed v3 worlds use **X/Z [-64,64), Y [-16,32)**. Missing cells in the streamed volume remain explicitly pending rather than editable air; trusted whole-chunk insertion is an initialization/import API, not a player edit.

`BlockChange::affected_chunks` identifies the owning chunk plus face-sharing neighbors when local coordinates are 0 or 15. Diagonal neighbors are unnecessary. The client accumulates these positions in a `BTreeSet`. Legacy mode rebuilds only dirty resident chunks in Update. Streamed mode queues affected columns for the single generation/mesh worker; it uses the current immutable overlay snapshot and rejects stale revision results. A boundary-corner edit affects at most four meshes. Mesh construction remains synchronous for these small, bounded per-click jobs; streaming/larger batches will require a job budget or asynchronous snapshots.

`ChunkRenders` retains at most one entity/handle per rendered chunk. Streamed ready-empty chunks need no render asset; becoming empty releases the old entity/mesh. Eviction explicitly despawns terrain entities and removes their assets, while edit data stays in the core overlay. Legacy empty geometry keeps its retained handles as before. MAIN_WORLD and RENDER_WORLD usages retain CPU buffers, replaced under the same asset handle on edits. Empty geometry hides the entity; it can be filled again without new handle allocation. Bounds are removed on rebuild so Bevy recomputes culling bounds after expansion. Repeated-edit tests check bounded asset counts, empty geometry and neighboring faces; idle frames do not remesh.

## Targeting and interaction (`voxel_core::raycast`, `voxel_core::interaction`)

`Ray` carries f64 eye origin, direction and maximum reach. Grid DDA normalizes the direction and returns the first solid cell, entry-face normal, physical distance and optional face-adjacent placement cell. Reach is inclusive with a 1e-10-block comparison tolerance; the default is five blocks, configured in the client's `Editor`, and the core bounds valid reach to [0,64]. Nonfinite/zero directions, invalid origins/reach and coordinate overflow return no hit.

Cells use half-open ownership `[n,n+1)`. An origin already in a solid cell returns distance zero, normal zero and no placement face. A negative ray starting on an integer boundary enters the negative cell at distance zero if the initial cell is air. Parallel axes do not step. Edge/corner ties within 1e-10 blocks step X, then Y, then Z, testing each intermediate cell at that distance. This deliberate contact/tie policy may select an edge-touch cell; it always produces a face-adjacent placement cell rather than an ambiguous diagonal offset.

The client publishes `ViewPose` while synchronizing the camera, before f64-to-f32 rendering conversion. Editing and highlighting consume that same interpolated eye and look direction after the fixed loop, once per render frame. This matches the crosshair's displayed pose, while collision checks use the authoritative current `Player` AABB. The client additionally rejects overlaps with the displayed body during interpolation/fast falling.

`interaction::edit` recomputes the ray hit against the current world rather than trusting a stale target. Breaking assigns air; placement requires a valid entry face, an empty adjacent cell, no positive-volume player overlap and editable bounds. Mere face contact is allowed. Empty/no-target edits are harmless; rejected operations leave storage unchanged. Public raw `set_block` permits trusted replacement, while player placement never replaces existing solids. World mutation is immediately authoritative for the next physics tick, independently of mesh presentation.

Only captured, focused, active-camera input can edit. A new capture suppresses both buttons for that frame; subsequent left clicks do not suppress mouse look. R/reset frames also suppress edits. Mouse edges trigger one operation per click, with breaking taking priority if both buttons arrive together. There are no hold-to-mine timers, inventory consumption or sounds.

Feedback uses original procedural mesh geometry and existing `StandardMaterial`: center crosshair, five color swatches with a selected frame, and a slightly expanded target wire cube. The title names the selected slot/material. Camera-relative feedback is sized in logical screen pixels using the current perspective FOV/window height. No UI/font/shader asset import or new Bevy features are required.

## Seeded generation and save feedback

`worldgen::GenerationConfig` contains an explicit u64 seed and generator version. Version 1 stays frozen at 64×64; version 2 samples a bounded 128×128 area with additional deterministic tree anchors. A 16-KiB / 64-KiB finite height cache uses three scales of Q10 integer smoothstep value noise. Stable coordinate hashing replaces a shared random stream. Layering and a connected cave/ramp plan are sampled in world coordinates; deterministic spaced tree anchors are planned before chunks are emitted. Trees cross seams without mutating neighboring chunks. Chunk generation is immutable/order-independent, skips out-of-bounds requests and only stores nonempty chunks. Golden snapshots cover seeds 0, 1, 42, 2026 and u64::MAX on both CI platforms. See [the contract and tradeoffs](WORLDGEN.md).

Legacy generation/meshing remains bounded at startup. New streamed worlds prepare one small spawn column before App construction; a single worker samples center chunks plus only the adjacent face cells needed by `mesh_chunk` (15 cached chunks at most, ~60 KiB of block payload, rather than 27 complete neighbors). This avoids redundant neighbor generation. Worker results carry the captured edit revision and desired-column identity; stale/obsolete results are discarded. There is no regeneration, chunk meshing or whole-world clone each frame. Future streaming needs regional planning/halos and asynchronous generation/meshing budgets; this finite cache is intentionally not an infinite-world abstraction.

`client::save_hud` renders original 5×7 bitmap lettering as camera-relative colored quads on a dark panel. It updates an existing mesh only when status changes, remains visible without mouse capture, scales using logical screen pixels and reuses one material/handle. Saving is yellow, success green and persistent errors red with F5 retry. No UI/font subsystem, external assets or dependency/feature changes are needed.

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

Collision sweeps the complete AABB movement on Y, X, then Z against `World::block`, clipping each displacement to the nearest solid face. Swept ranges include intermediate blocks, preventing destination-only tunneling. Other axes use open overlap intervals with a 1e-8 contact tolerance, allowing contact with floors/walls without snagging parallel motion. The fixed axis order gives wall sliding and repeatable resolution; it can bias exact corner contacts. There are no per-tick allocations, mesh collision structures or whole-world scans: only the nearby swept voxel ranges are queried. Legacy missing chunks and positions outside the finite world are Air. Streamed pending in-volume chunks behave as solid barriers. `cache_ready` checks occupied player cells; pending teleports pause simulation, then recover a clear position before ticking if loaded terrain overlaps the body.

Contracts: configuration values and player state must be finite, dimensions/speeds positive, and spawn/reset must not intersect solids. This is a static-voxel kinematic controller, not a general rigid-body/depenetration solver. Player placement now rejects edits intersecting the player; `fits_in_world` also supports spawn clearance checks. Replaying the same commands is regression-tested, but cross-platform bitwise floating-point lockstep is not promised.

## Fixed simulation, input and camera adapters

`client/player.rs` retains `PlayerState` (current body plus previous fixed position), `PlayerSettings` and `ControllerInput` as resources. Physics runs in `FixedUpdate` at 60 Hz. Bevy's normal bounded time catch-up handles slow render frames; excess wall time after a long stall is not treated as a single large physics step.

`client/camera.rs` samples buttons and relative mouse motion **before** the fixed loop, after Bevy updates input. Jump/reset edges are latched across render frames without a physics tick, consumed once, and cleared when controls pause. Holding Space never repeats a jump. Mouse sensitivity is independent of delta time; pitch is clamped and movement follows yaw only. Hover motion on the capture frame is ignored to prevent a view jump.

After the fixed loop, the camera interpolates previous/current feet positions using the fixed clock's overstep fraction, adds eye height and applies the immediately updated view rotation. This smooths translation at differing frame rates at the cost of one physics tick of latency. The simulation never reads the interpolated transform.

Capture starts on click; Escape or focus loss releases it. Re-focusing requires another click. In this local single-player prototype, release pauses the player simulation (including falling), preserving velocity until resumed; pending edges are cleared. A future authoritative multiplayer/server simulation must not inherit this local pause policy.

The preferred spawn column is near (0,10), facing -Z. `physics::spawn_position` scans nearby known resident sky-exposed ground, skips tree tops and checks the actual player AABB. The streamed spawn column remains pinned, so R/void recovery never waits on unbounded loading. Initialization, R and void recovery all use restored/generated blocks, never a fixed demo Y. A fully empty snapshot has no supported spawn; it is preserved and uses a clear-air fallback rather than creating terrain. R resets body, previous position, velocity, pending jump and view; falling below Y=-32 resets the body automatically. Resetting both positions prevents interpolation across the map. There is no debug-flying mode, automatic step-up or collision crouch. Block editing is handled by the separate interaction adapter and never reads mesh geometry for gameplay.

## Persistence (`voxel_core::persistence`, `client::persistence`)

Versions 1/2 remain full-world snapshots with missing-record-is-Air semantics. They stay finite and are never implicitly expanded/migrated. Version 3 stores **seed + generator v2 + a full sparse last-write overlay**, including ID 0 (Air), within the expanded volume. Unmodified/unvisited chunks regenerate from the frozen identity; loaded resident chunks are not persisted. Saving the entire overlay ensures edits to evicted chunks are included, even when their cache/GPU data is absent. The v3 cap is 786,432 records / 11,010,088 bytes; unknown identities/versions, reserved flags, invalid bounds/IDs, ordering and CRC errors are rejected. Store replacement also refuses identity/mode changes. See [the format contract](SAVE_FORMAT.md).

The core owns format validation, stable block IDs, checksums, bounded reads, exclusive session locking and filesystem commit/recovery rules, without Bevy or environment lookup. `SaveStore::open_with` accepts an absolute directory and a lazy new-world factory. `open` retains the old demo factory as a library/legacy-test convenience. The client resolves per-user Linux/Windows paths or `RUSTCRAFT_DATA_DIR`, loads before constructing the App, selects explicit legacy/v3 mode, and injects authoritative `VoxelWorld` before startup meshes, collision and safe player initialization. Player position and presentation state are intentionally transient.

`World` maintains a process-local revision that advances only on actual mutations (and changed legacy chunk imports). Revisions are never serialized. Loaded revisions start clean. A generated world has an explicit first-creation pending flag: one background save is requested even without edits, and only a completed commit clears it. Failure keeps the new world dirty and blocks normal close until retry succeeds. Legacy saves clone the bounded full world. Streamed saves capture only identity, revision and the immutable edit-map Arc, excluding resident vectors; encoding, sorting, reading, syncing and replacing files run on one background thread. Frames poll completion and never join an unfinished job. A completed older revision cannot mark newer edits clean. F5 requests can queue behind a running job; autosave uses real time, even when local player simulation is paused. At most one save is in flight, so no jobs race the backup/primary.

The native close request is handled explicitly instead of Bevy's automatic window deletion. Controls pause, in-flight work completes, any newer dirty revision is saved, and only then is `AppExit` sent. Failures remain dirty and cancel closing so the user can retry. Forced process termination is outside that protocol. A Drop guard only drains an already-running worker if the App is destroyed by another runner. Feedback never reports success before the synced commit returns.

The primary is never truncated in place. A validated old generation is atomically backed up before the new synced temporary file replaces it; Unix additionally syncs directory metadata. Corruption/unsupported versions stop startup and preserve files. Backups are restored explicitly, never silently regenerated or discarded. See [the byte format, limits and failure matrix](SAVE_FORMAT.md), including Windows and power-loss limits.

## Platform and dependency choices

Only the Bevy subsystems needed for 3D rendering, desktop windows and keyboard/mouse input are enabled, including **both X11 and Wayland**. Audio, gamepad, UI/font, scene import and external asset formats are not enabled by this application. Bevy/wgpu retains its native graphics backend selection; the client does not force Vulkan on every platform. See `docs/PLATFORMS.md` and `docs/THIRD_PARTY.md`.

## Next boundaries

Extend generation and gameplay on the existing worldgen/ray/edit/physics/persistence boundaries. Keep existing snapshots independent of new terrain defaults, and implement explicit migration before accepting a new format. Do not persist ECS entities or derive simulation state from mesh geometry. Networking, server authority, mods and a general item registry remain deferred.
