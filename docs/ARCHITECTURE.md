# RustCraft architecture

Status: **v0.1 / slice 6 bounded streaming implemented**. Later gameplay systems remain proposals.

## PR #12 native-review correction design (recorded before implementation)

Remove the unconditional spawn pin from normal residency. R/void recovery moves to
the spawn request, pauses simulation while that column is pending, then resolves
safe ground and resets interpolation from actual edited blocks. Distant spawn
CPU/GPU resources follow the ordinary radius; persistence is still independent.

Use one contact tolerance for collision and clearance/recovery. The pre-fix deterministic
diagonal-edge trajectory backtracks at tick 26 because reconstructing
the clipped x=2.3 AABB yields 1.9999999999999998 and triggers false recovery.

Separate new-column generation from cached-chunk remeshing. Keep transient local
mesh dependency versions (own chunk plus face changes), independent of the global
save revision. Batch connected dirty meshes and publish them together only when
their dependencies are current. Prioritize visible dirty geometry over new loads,
reuse cached voxel data, and sample only missing face cells off-thread. Coalesce
edits while a bounded job runs; never clear newer invalidations. Measure old/new
seam workloads, discarded jobs, dispatch/install time, dirty latency and resource
counts. Save bytes, generator identities, world extent and textures stay unchanged.

## Initial Slice 6 design note (historical, before native review)

The original pin/global-revision decisions below are superseded by the correction
design above and the current lifecycle/meshing sections below.

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

Legacy startup retains at most 48 chunks. Streamed startup prepares only three spawn chunks before the App. Those chunks later follow the ordinary radius, with no permanent CPU or render pin. One worker handles new-column generation or cached mesh batches, and the main thread installs at most one result per frame. Each successful `World::set_block` returns a `BlockChange`; equal assignments are no-ops. Legacy edits remain **X/Z [-32,32), Y [-16,32)**, streamed v3 edits **X/Z [-64,64), Y [-16,32)**. Streamed pending data is not editable; trusted whole-chunk insertion is initialization/cache import, not a player edit API.

`BlockChange::affected_chunks` identifies the owner plus face-sharing neighbors at local coordinates 0/15; diagonal chunks are unnecessary. `World::mesh_revision` advances only those output dependencies for actual edits and survives cache eviction. These transient tokens do not enter saves; the existing global revision still governs persistence. Legacy dirty resident chunks rebuild synchronously as before. Streamed dirty chunks coalesce while a worker runs. The scheduler picks the nearest connected dirty component before new loads; all targets in that component are validated and published together in one Update. A newer edit affecting any target rejects that whole batch without clearing its newer dirty entries. Unrelated edits no longer invalidate useful jobs. No-longer-desired results cannot recreate evicted resources. A valid load also removes duplicate requests discovered during center movement.

Cached remeshing snapshots only target and available face-neighbor chunks into engine-neutral data. The worker reuses these voxels and samples generator + captured edits only for missing face cells; it never regenerates complete columns for edits. New-column jobs retain the deterministic three-chunk + thin-halo generation path. Main frames poll without joining unfinished work. A large connected edit batch remains bounded by the finite resident set; no unbounded queue, busy-wait or synchronous world generation is introduced.

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

Legacy generation/meshing remains bounded at startup. New-column workers sample center chunks plus adjacent face cells (15 vectors at most, ~60 KiB block payload), returning three chunks and meshes. Cached mesh jobs use the separate snapshot path above. Desired membership and local output tokens are checked before any install, independently of the global save revision. There is no idle-frame generation/meshing, whole-world copy for a normal click, or frame-loop disk I/O. This remains a finite prototype, not an infinite-world abstraction.

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

Collision sweeps the complete AABB movement on Y, X, then Z against `World::block`, clipping each displacement to the nearest solid face. Swept ranges include intermediate blocks, preventing destination-only tunneling. Other axes use open overlap intervals with a 1e-8 contact tolerance, allowing contact with floors/walls without snagging parallel motion. The fixed axis order gives wall sliding and repeatable resolution; it can bias exact corner contacts. There are no per-tick allocations, mesh collision structures or whole-world scans: only the nearby swept voxel ranges are queried. Legacy missing chunks and positions outside the finite world are Air. Streamed pending in-volume chunks behave as solid barriers. `fits_in_world` and `cache_ready` use the same open intervals/contact tolerance as the sweep, preventing ULP-sized face contact from triggering clearance recovery; meaningful penetration still fails. `cache_ready` checks occupied player cells; pending teleports pause simulation, then recover a clear position before ticking if loaded terrain overlaps the body.

Contracts: configuration values and player state must be finite, dimensions/speeds positive, and spawn/reset must not intersect solids. This is a static-voxel kinematic controller, not a general rigid-body/depenetration solver. Player placement now rejects edits intersecting the player; `fits_in_world` also supports spawn clearance checks. Replaying the same commands is regression-tested, but cross-platform bitwise floating-point lockstep is not promised.

## Fixed simulation, input and camera adapters

`client/player.rs` retains `PlayerState` (current body, previous fixed position and pending-respawn flag), `PlayerSettings` and `ControllerInput` as resources. Physics runs in `FixedUpdate` at 60 Hz. Bevy's normal bounded time catch-up handles slow render frames; excess wall time after a long stall is not treated as a single large physics step.

`client/camera.rs` samples buttons and relative mouse motion **before** the fixed loop, after Bevy updates input. Jump/reset edges are latched across render frames without a physics tick, consumed once, and cleared when controls pause. Holding Space never repeats a jump. Mouse sensitivity is independent of delta time; pitch is clamped and movement follows yaw only. Hover motion on the capture frame is ignored to prevent a view jump.

After the fixed loop, the camera interpolates previous/current feet positions using the fixed clock's overstep fraction, adds eye height and applies the immediately updated view rotation. This smooths translation at differing frame rates at the cost of one physics tick of latency. The simulation never reads the interpolated transform.

Capture starts on click; Escape or focus loss releases it. Re-focusing requires another click. In this local single-player prototype, release pauses the player simulation (including falling), preserving velocity until resumed; pending edges are cleared. A future authoritative multiplayer/server simulation must not inherit this local pause policy.

The preferred spawn column is near (0,10), facing -Z. `physics::spawn_position` scans nearby known resident sky-exposed ground, skips tree tops and checks the actual player AABB. The spawn column is disposable like all other columns. If absent, R/void moves the request above its finite volume, freezes physics and blocks targeting/editing while the ordinary scheduler loads it. Ground selection runs only after readiness; current/previous positions reset together, so there is no interpolated flight or search over unrelated remote terrain. Initialization, R and void recovery all use restored/generated blocks, never a fixed demo Y. A fully empty snapshot has no supported spawn; it is preserved and uses a clear-air fallback rather than creating terrain. R resets body, previous position, velocity, pending jump and view; falling below Y=-32 resets the body automatically. Resetting both positions prevents interpolation across the map. There is no debug-flying mode, automatic step-up or collision crouch. Block editing is handled by the separate interaction adapter and never reads mesh geometry for gameplay.

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

## Streaming bounds and diagnostics after native review

Radius 2 retains at most 75 chunks / 307,200 block payload bytes; radius 3 at
most 147 / 602,112 bytes, with no extra spawn pin. Desired and dirty sets are
bounded by the finite column/chunk set and only one worker result is in flight.
Ordinary mesh snapshots copy a target and available face neighbors. In the worst
large connected batch the worker may materialize up to the finite 192 chunks
(786,432 payload bytes, plus one transient border copy), excluding mesh buffers.
New-column jobs retain the smaller 15+3-vector peak. Snapshot work runs before
dispatch on the main thread and is explicitly measured; generation/meshing stays
off-thread. Native dense-overlay copy-on-write and large bulk-edit latency still
need profiling before extending world bounds.

Idle-transition logs include resident/mesh/buffer counts, peak queued work, load
attempts, mesh targets, discarded jobs, summed worker wall time and worker/install/
stream-update/dirty-latency maxima. A fixed 128-sample ring records frame intervals;
p95 is computed only when logging an idle transition. Frame intervals include
rendering and driver scheduling, not measured GPU execution. Tests assert bounded
assets/entities and buffers across repeated edits and travel. See VALIDATION for
reproducible before/after CPU and graphical measurements.
