# RustCraft validation

## Slice 6 — safe bounded streaming, expanded worlds and edit persistence

Validated locally on 2026-10-22, starting from latest `main` at `49023f7` after PR #10. Debian 13 glibc x86-64, Rust 1.99.1 / Bevy 0.20.0. The owner confirmed slices 1–5 on Void. This is a bounded streaming implementation, not an infinite-world claim or a native Void/Windows benchmark.

### Passed development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

**118 tests passed, none failed or ignored** (75 core + 43 client; no doctests defined). All 101 previous tests remain passing, with unknown-version fixtures adjusted because v3 is now supported. No dependency, engine feature, lockfile, toolchain or downloaded-asset additions. The existing Ubuntu/Windows matrix runs the same checks and golden fixtures; the PR records remote results.

- Generation/meshing: all frozen generator-v1 snapshots retained; new generator-v2 golden columns pin seeds 0, 42 and u64::MAX across expanded negative/positive coordinates. Reverse/repeated requests, full-neighbor reference versus thin-halo meshes, tree/cave/edit seams and finite bounds are covered.
- Lifecycle: every radius/center policy stays bounded and includes the spawn pin, including extreme travel coordinates. ECS tests move repeatedly between distant columns, release CPU blocks/entities/mesh assets, preserve unsaved Air and Wood edits, and verify unchanged edit revisions. Separate channel-barrier tests reject obsolete still-current-revision results and stale edit-revision results; neither may overwrite edits or resurrect evicted GPU resources. Repeated empty/refill cycles release assets without losing readiness.
- Safety/gameplay: pending floor/wall barriers prevent tunneling and reject raw/player edits, wall contact remains movable, teleports pause until ready then recover clearance, and void recovery uses pinned spawn. All existing walking/jump/input/targeting/placement/collision/remesh tests remain passing.
- Persistence: independent 40-byte v3 golden identity, explicit Air, malformed metadata/reserved flags/IDs/coordinates/CRC/truncation, full overlay snapshots without resident chunks, and identity/mode/seed replacement refusal. Evict → failed save → retry → restart tests retain removed and added cells in old/newly visited columns. Injected partial/sync/backup/pre-rename failures preserve the previous durable generation and dirty evicted overlay. Existing historical v1/v2 fixtures, locking/backup/atomicity/recovery tests still pass; a loaded v1 survives idle/F5/normal close byte-for-byte without migration.
- All files/tests/runs use isolated disposable directories or explicit absolute `RUSTCRAFT_DATA_DIR` paths outside the repository. No actual player saves are accessed or committed.

### Measured workload and budgets

Debug/cloud observations, not hard timing thresholds or release/GPU performance guarantees:

| Workload | Measured result |
| --- | --- |
| Headless seed-42 radius-1 tour, 56 column installs/remeshes, seven center visits, two edits and repeated eviction/return | ~2.92 s total; peak 30 resident chunks; longest isolated worker ~58.3 ms; final logical terrain buffers 2,030,808 bytes |
| Same tour before thin face-halo optimization | ~14.99 s total; longest worker ~342.7 ms; identical resulting meshes/data |
| Graphical default-radius-2 restart, seed 2026 | 75 resident chunks, 56 rendered terrain meshes; 307,200 block payload bytes; 4,804,056 logical terrain buffer bytes; longest worker ~88.6 ms; full desired set ready ~2.6 s after initial startup meshes |
| Pinned spawn preparation / three initial meshes in that run | ~31.2 ms / ~8.7 ms |
| Software-rendered process RSS sample after default-radius loading | 343,736 KiB (~336 MiB), including Bevy, Mesa/LLVM, driver, shaders and all application data; a sample, not measured peak RSS |
| New unedited v3 file / one edited cell | 40 / 54 bytes, independent of explored resident set |

Default radius 2 retains at most 75 vertical chunks. The conservative configured ceiling is 150 chunks / 614,400 block payload bytes, excluding container overhead. The thin worker halo holds at most 15 chunk vectors / 61,440 payload bytes, with up to three additional returned chunk copies before the halo drops (~73,728 transient block bytes). Generator-v2 heights use ~64 KiB. Mesh buffers, driver allocations, save validation and the central edit overlay have separate budgets. Overlay records are capped by the 786,432-cell finite volume; dense-map copy-on-write latency is an explicit prototype limit, not profiled here. No per-frame generation, whole-world cloning or disk I/O; one worker/column result at a time.

### Graphical checks

Real client on Xvfb X11 + Mesa 25.0.7 llvmpipe Vulkan software rendering. The native XTest procedure placed Wood, checked v3 metadata/CRC and F5 success, walked/sprinted/jumped across a column boundary, observed new x=2 columns (beyond the original +32 range) loading while old columns were removed, and returned through R. Capture/Escape and existing save feedback remained functional. Pure movement/residency left saved overlay bytes unchanged. The edited column was the pinned spawn column in this graphical run; tests separately cover edited-column eviction before saving.

A fresh process with a different seed override and default radius 2 loaded the persisted identity (seed 2026/gen2), reached 75 chunks, displayed the saved Wood block and did not rewrite the file. A separate historical v2 procedural fixture ran in explicit `LEGACY 64x64` mode, ignored invalid seed/radius overrides and remained byte-identical through idle F5 and native close. Screenshots were inspected. Only expected software-adapter/Xvfb warnings occurred in the final runs; ready-empty chunks no longer submit empty GPU meshes. No screenshots, generated worlds or harness files are committed.

### Remaining limits / acceptance

Native Void glibc X11/Wayland and Windows GUI tests, physical-GPU frame performance, real compositor behavior and dense-edit copy-on-write profiling remain outstanding. Follow [the isolated manual procedure](PLATFORMS.md#slice-6-manual-test). This is a bounded 128×128×48 new-world mode; legacy worlds remain finite and fully resident. No opt-in legacy expansion, infinite terrain, region files, per-chunk disk cache, per-edit durable journal or cancellation inside a running job. Mesh presentation can lag authoritative edits by a worker job; pending frontiers deliberately block interaction/motion. Unsaved edits survive eviction in RAM, but forced termination may lose changes since the last completed save. Existing filesystem/power-loss/backup limitations remain in [SAVE_FORMAT.md](SAVE_FORMAT.md).

## Slice 5 — seeded finite terrain, caves, trees and compatible saves

Validated locally on 2026-10-21, starting from latest `main` at `353daac` after PR #8. Environment: Debian 13 glibc x86-64, Rust 1.99.1, Bevy 0.20.0. The owner confirmed slices 1–4 on Void; native slice-5 Void/Windows play-testing is separate.

### Passed development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

**101 tests passed, none failed or ignored** (66 core + 35 client; no doctests defined). All 86 previous tests remain passing, with legacy expectations extended for five selection slots and v2 writing. No new crates, Bevy features, toolchain changes, lockfile changes or downloaded assets were needed. The existing Ubuntu/Windows CI matrix runs these checks and the same golden fixtures; the PR records remote outcomes.

- Determinism: full-snapshot golden CRCs for seeds 0, 1, 42, 2026 and u64::MAX; repeat generation, reversed chunk order, distinct seeds and unsupported generator versions. Terrain tests cover coherent height range/slopes/layers, connected bottom foundation, finite bounds, empty/out-of-bounds chunk handling and extreme coordinate reads.
- Caves/trees/spawn: continuous headroom along the stair-stepped entrance and cross-world cave; supported trunks, clear crowns, spacing and cross-chunk foliage; 24 seeds with actual player physics settling on clear supported ground. ECS startup/R/void recovery use generated terrain instead of the old demo Y. Wood/Leaves cull neighboring faces across positive/negative seams and are physically solid.
- Compatibility: independent historical v1 fixtures (mixed materials, negative coordinates and removed terrain) load without byte changes or factory calls; a changed save upgrades to v2 and retains exact v1 bytes in the backup. v1 rejects new IDs, v2 round-trips Wood/Leaves, unknown formats abort without regeneration/overwrite. Existing CRC/truncation/invalid data, injected atomic-write failures, locking and recovery tests remain passing.
- Lifecycle/gameplay: a newly generated unedited world saves once, survives restart, ignores a different override seed and skips idle rewrites. Edits/removals/new materials persist after restart. First-creation failure remains dirty and prevents silent close; F5 retries. Strict seed parsing is tested without mutating process environment. Existing input/capture/physics/edit/remesh regressions remain passing; keys 4/5 place/break the expected materials.
- Save HUD: all used glyphs and status/color mappings, actual ECS updates while uncaptured, repeated saving/error/success transitions and resize with one retained mesh/material. No per-frame text mesh allocation when status is unchanged.

### Graphical and executable checks

Used the real client on Xvfb X11 + Mesa 25.0.7 llvmpipe Vulkan software rendering. All runs used explicit isolated absolute data directories outside the repository; actual player saves were never read or modified. Inspected screenshots of terrain/trees, five selection swatches, yellow `SAVING...`, green `SAVED` and persistent red `SAVE ERROR - F5 RETRY`.

The XTest procedure exercised initial generation and saving **without edits**, capture/look, Wood placement through autosave, breaking, F5, Leaves placement saved by normal native close, Escape release and visible feedback while paused. Independent Python `struct`/`zlib` checks verified v2 records/CRC and material-count changes. A real backup-path I/O error left the primary byte-for-byte intact, displayed the error, and successfully retried with F5 after removing the obstruction. Previous-generation backup bytes were checked. Native close exited successfully.

Restarting with an intentionally invalid seed override loaded the final saved world unchanged; it did not invoke seed validation/generation. Walking changed the rendered view after load; jump/reset/cursor inputs were also exercised, with detailed movement invariants covered by ECS/core tests. A second fresh world with seed 42 matched its golden snapshot and differed from the seed-2026 world. The real executable rejected an overflowing seed for a missing world without writing a primary/backup.

Measured debug/cloud startup: seed 2026 generated 34 resident chunks in ~126 ms and built initial meshes in ~233 ms; seed 42 generated 36 chunks in ~151 ms and meshed in ~230 ms. Files were ~1.29 MB and ~1.21 MB respectively. These are bounded startup observations, not benchmarks of the owner's GPU, release builds or peak renderer memory. Only expected software-adapter/Xvfb warnings and intentionally induced save errors occurred.

### Remaining acceptance and deliberate limits

Native Void glibc X11/Wayland and Windows GUI tests, physical GPU/frame performance, compositor/pointer behavior and hands-on cave traversal remain outstanding. Follow [the isolated manual procedure](PLATFORMS.md#slice-5-manual-test). Software rendering and headless CI do not establish these outcomes.

The volume remains finite; caves use a simple connected tunnel/ramp plan, trees have no growth/decay and leaves are solid/opaque. No streaming, biomes, water, seed metadata, player/inventory persistence or new lighting system. Loaded snapshots are authoritative and v1 upgrades only on changed saves. Backups retain one generation; old clients cannot read v2. Power-loss/forced-termination limitations from slice 4 remain documented in [SAVE_FORMAT.md](SAVE_FORMAT.md).

## Slice 4 — versioned save/load and autosave

Validated locally on 2026-10-21, starting from `main` at `6026f74` after PR #6. Environment: Debian 13 glibc x86-64, Rust 1.99.1, Bevy 0.20.0. The owner confirmed slices 1–3 on Void; this report does not claim a native slice-4 Void or Windows play-test.

### Passed development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

**86 tests passed, none failed or ignored** (58 core + 28 client; no doctests defined). All 65 previous tests remain passing. The sole new crate is `crc32fast` 1.5.3; Cargo metadata confirms MIT OR Apache-2.0 and the inventory/lockfile are updated. The Ubuntu/Windows workflow retains all four required checks; remote results are recorded on the PR.

- Format: independent golden little-endian/CRC fixture; canonical full-snapshot round trips; all materials, deleted/replaced terrain, negative/boundary coordinates, new chunks and valid empty worlds. Decode validates every truncated prefix of an edited-world snapshot, trailing bytes, corrupted payload, magic/version/count/length, unknown/zero IDs, duplicate/unsorted positions, unsupported coordinates and oversized input.
- Filesystem: missing save and stale temps, restart after replacement, previous-generation backup, exclusive lock/release, unsupported imports, invalid paths, oversized disk files and real backup-directory failure. Injected partial-write, post-sync, backup-commit, pre-rename and post-commit errors test preservation, cleanup and honest failure reporting. Corrupt primary is never overwritten; missing primary with backup refuses a new demo; explicit preserve/copy backup recovery is tested.
- Client/ECS: platform path resolution without mutating process environment, clean/no-op frames and manual save, real-clock dirty autosave while unfocused, no idle rewrites, F5 restart, an older in-flight snapshot retaining newer dirty edits, latest-revision save before close, failure canceling close and successful retry, input pause/resume around final saving, restored mesh generation and authoritative collision, and safe spawn above saved blocks.
- All persistence tests use unique disposable directories, not actual per-user saves. A real executable test additionally rejected a truncated save before graphics initialization with a nonzero exit and byte-for-byte preservation of the damaged file.

### Graphical save/restart test

Ran the real client in Xvfb X11 using Mesa 25.0.7 llvmpipe Vulkan software rendering, with an isolated absolute `RUSTCRAFT_DATA_DIR` outside the repository. A local XTest harness exercised capture/look, block selection and placement, F5 save, a terrain change saved by the actual 30-second timer while the cursor was released, and a final edit saved solely by native window close. Binary magic/version/CRC were checked independently with Python `struct`/`zlib`; each backup matched the preceding generation. The native close exited successfully after the final primary changed.

A fresh client process reported `Loaded` and read exactly the final snapshot. Inspected screenshots showed the saved stone placement; every terrain pixel matched the pre-close view after restoring the same view direction (the small crosshair/swatch area was excluded because Escape feedback can differ for one rendered frame). Mouse capture/look and walking/jumping still worked after load; movement did not rewrite the save. Headless ECS tests separately assert restored blocks are solid, meshes use restored data, and obstructed spawns are clear.

Only expected software-adapter/Xvfb warnings occurred. This establishes functional save/render/input coverage, not physical-GPU performance or real-compositor behavior. No generated saves, logs, screenshots or test harness assets are committed.

### Remaining acceptance and limits

Native slice-4 Void X11/Wayland and Windows GUI checks remain outstanding. Follow [the manual procedure](PLATFORMS.md#slice-4-manual-test), including corruption recovery in a disposable copy. The existing CI matrix exercises filesystem replacement/locking and builds/tests on Ubuntu and Windows; headless CI does not establish desktop input behavior.

One bounded world is saved; player position, inventory, world selection, seeded generation and migration remain deferred. Unknown formats stop safely. Backups retain one generation, not unlimited history. File syncing/rename cannot promise survival of arbitrary hardware/filesystem failure or forced termination; Windows does not perform Unix directory fsync. See [format and failure policy](SAVE_FORMAT.md).

## Slice 3 — targeting, breaking and placement

Validated locally on 2026-10-21, starting from `main` at `31928c4` after PR #4 and the updated public-repository/asset policy. Environment: Debian 13 glibc x86-64, Rust 1.99.1, Bevy 0.20.0. The owner confirmed the previous walking prototype under Void; this is not a new native Void play-test claim.

### Passed development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

**65 tests passed, none failed or ignored** (45 core + 20 client; no doctests defined). No dependency, feature or lockfile changes were needed.

- DDA: all axis directions, positive/negative chunk seams, scaled directions, empty/missing cells, inclusive reach and misses, inside-solid starts, integer boundaries, parallel axes, diagonal/near-tie rules, invalid/nonfinite input and i32 overflow.
- Mutation/edit rules: change/no-op reporting, bounds without unnecessary allocation, selection slots, repeated clicks, player feet/head/corner overlap and allowed contact, inside-solid placement rejection, out-of-bounds placement, missing neighbor storage, dirty face neighbors and updated physics collision.
- ECS/input: capture/refocus click suppression, inactive/unfocused/released controls, no active camera, reset frames, empty clicks, simultaneous-button priority, selected-block/title/target feedback, configurable reach, protected authoritative body despite interpolated eye, floor breaking followed by actual falling and rejected body placement. View rotation still works on subsequent mining clicks.
- Rendering: geometry changes through actual click handling, positive/negative seam remeshing, safe empty meshes, 50 repeated boundary remove/refill cycles with stable mesh handles/counts, and no idle remeshing. Existing voxel/physics/FPS tests still pass. Respawn also searches above newly built spawn obstacles.

### Passed graphical smoke test

Ran the client on Xvfb X11 with Mesa 25.0.7 llvmpipe Vulkan software rendering. Inspected screenshots of the crosshair, three color swatches/selection frame, black target outline and placed dirt geometry. A local XTest harness verified:

1. Captured input and selecting slot 2 update the block feedback.
2. Right-click placement changes rendered geometry; left-click breaking restores the original scene.
3. Five more place/break cycles restore the same rendered image.
4. Walking still changes the view while the overlay is active.
5. Mining the supporting floor changes graphics and makes the player fall.
6. Placement into the standing player is rejected without changing the rendered scene.
7. Escape/focus loss release capture; an uncaptured right click does not edit; refocus does not capture automatically.
8. Closing the native window exits successfully.

The renderer had only expected software-adapter/Xvfb settings warnings. No packaged assets or GPU backend settings were added to the application. The software test is not a performance benchmark or a physical-driver compatibility claim.

### Remaining platform checks

The existing Ubuntu/Windows GitHub Actions matrix runs formatting, Clippy, tests and builds on the PR; consult its live checks for the remote outcome. Native slice-3 GUI checks on Void X11/Wayland and Windows remain outstanding, including real pointer/compositor behavior, window resizing and physical GPU performance. Use [the manual procedure](PLATFORMS.md#slice-3-manual-test).

Edits remain in memory only and disappear on exit. Editable coordinates are X/Z [-32,32), Y [-16,32); missing chunks are air. Persistence, mining timers, inventory, world generation and texture packs are outside this slice.

## Slice 2 — walking, gravity, jumping and collision

Validated locally on 2026-10-21: Debian 13 glibc x86-64, Rust 1.99.1, Bevy 0.20.0, starting from `main` at `e83fa40` after PR #2. The owner reported that slice 1 worked on Void and that its Ubuntu/Windows CI passed; those results are baseline evidence, not native validation of slice 2.

### Development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

All local checks passed. **34 tests passed, none failed or ignored** (25 core tests and 9 client tests; no doctests defined).

- The 11 original voxel/indexing/meshing tests remain passing.
- 14 physics tests cover ten seconds of jitter-free floor rest, jump/landing/apex, rejected air jumps and stale support, ceilings, normalized acceleration/sprinting/braking, yaw, wall sliding/corners, ledges, negative and positive X/Z chunk seams, walls approached from both sides, high-speed floor/wall/ceiling sweeps, terminal falls onto a single block at negative Y, climbing a one-block terrace, air momentum and a 6,000-tick command replay with collision-invariant checks.
- 8 camera/controller tests exercise actual Bevy scheduling: identical simulation states at 30/60/144 FPS, queued jump edges across zero-tick frames and consumption across multi-tick frames, no held-key auto-jump, pause/focus/cursor transitions, pitch/mouse frame-rate independence, capture without hover jumps, reset/void recovery without interpolated teleports and movement using reset yaw immediately. The original scene plugin test still checks four nonempty meshes and the shared material.

### Graphical/input regression check

The client was run with `cargo run --locked -p rustcraft` on Xvfb X11 and Mesa 25.0.7 llvmpipe Vulkan software rendering. Screenshots of standing and jumping onto the terrace were inspected. A local XTest harness checked native pointer capture, relative mouse look, walking, solid-terrace blocking without jitter, jump plus forward movement onto the terrace, settling, Escape release, paused movement and focus-loss/re-entry behavior. The native close request exited successfully.

The software adapter and Xvfb display/settings warnings are expected. This does not benchmark physical GPU performance or verify a desktop compositor. Backend selection was supplied to the smoke-test process only; the application keeps normal native backend discovery.

### Coverage limits and follow-up

The existing Ubuntu/Windows GitHub Actions matrix runs all required Cargo checks on this PR; consult its live result for remote CI status. Native slice-2 play-testing on Void X11/Wayland and Windows remains required using [the manual procedure](PLATFORMS.md#slice-2-manual-test). In particular, physical input feel, compositor pointer behavior and actual GPU drivers are not established by this software-rendered run.

Physics assumes a valid, non-intersecting initial player AABB and finite positive tuning. It does not depenetrate a player spawned inside blocks. Automatic step-up, crouching, block editing and save/load remain outside this slice. Command replay is repeatable in these tests, not a guarantee of cross-platform floating-point lockstep.

## Historical slice 1 validation

Validated on 2026-10-20 in the Codex cloud environment: Debian 13 glibc x86-64, Rust 1.99.1, Bevy 0.20.0. This is not a Void Linux or Windows play-test report.

### Passed development checks

From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
```

All completed successfully. There were **16 passing tests**, with no failures or ignored tests (the library has no documentation tests):

- 11 `voxel_core` tests: exhaustive chunk indexing, bounds, negative coordinate boundaries and round trips including i32 limits, checked overflow, world lookup, fixed scene repeatability, empty/single/adjacent block geometry, all-axis inter-chunk culling, cavity faces, outward triangle winding and vertex attributes.
- 5 client tests: the real scene plugin creates four nonempty mesh entities and one material; click/Escape/focus cursor transitions; movement gating; normalized diagonal flight and reset; frame-time-independent mouse look and pitch clamping.

The tests execute engine-independent logic and actual Bevy ECS systems. They do not initialize a graphics backend.

### Passed graphical smoke check

The unmodified client was launched with `cargo run --locked -p rustcraft` in an isolated Xvfb X11 display using Mesa 25.0.7 **llvmpipe software Vulkan** (LLVM 19.1.7). Backend selection was supplied only to that local test process; the application does not hardcode Vulkan.

A screenshot was inspected: a lit, connected terraced voxel surface, stone landmark and sky are visible. A local XTest harness then verified:

1. A left click acquires the native X11 pointer grab.
2. Relative mouse input changes the rendered view.
3. WASD input changes the rendered view.
4. Escape releases the native pointer grab.
5. Movement keys after release leave the rendered image unchanged.
6. Focus loss releases capture; regaining focus does not recapture it.
7. A native window-close request exits the application successfully.

Xvfb emits expected display/settings warnings, and Bevy warns about the software adapter. This is functional rendering/input smoke coverage, not a performance benchmark or a guarantee for physical graphics drivers.

### Outstanding platform acceptance

- Native Void Linux glibc with the owner's graphics driver.
- A real Wayland session, including pointer locking and focus transitions.
- Native Windows MSVC compilation and GUI testing. The repository's GitHub Actions matrix is configured for Ubuntu and Windows; no remote CI result is claimed here.
- Manual resize, physical input feel, fullscreen behavior and supported GPU/driver combinations.

Follow the [manual test procedure](PLATFORMS.md#slice-1-manual-test). Full v0.1 gameplay is intentionally incomplete: collision/gravity, block interaction, seeded generation, hotbar and save/load are later slices.
