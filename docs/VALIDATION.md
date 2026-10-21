# RustCraft validation

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
