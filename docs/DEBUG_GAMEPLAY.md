# Slice 19 — F3 debug overlay and gameplay polish

Based on merged Slice 18 `f381ce196bb3162fa3e834b412b943375d24facf`.
No save-format, generator, atlas, skylight/AO, sky/fog shader, fixed timestep,
engine dependency or graphics-backend changes. Torches, emissive block light,
lighting-engine changes and time of day remain outside this slice.

## Controls and diagnostics

**F3** toggles original pixel diagnostics while playing, without releasing mouse
capture or interrupting movement/look. Holding F3 does not repeat. F3 stays reserved
outside configurable gameplay bindings and is ignored in settings/key capture.
The overlay hides in inventory, Pause/settings and outside a world; its toggle
resets on session teardown/load and is never written to settings or saves.

The compact panels leave the save feedback, crosshair and hotbar unobstructed.
They show version/dev slice, exponentially smoothed FPS/frame milliseconds,
authoritative f64 feet XYZ, floored block XYZ, horizontal chunk XZ, local XYZ,
cardinal direction/yaw/pitch, render radius, resident chunks and retained mesh
slots. Horizontal speed, actual grounded/crouch state and sprint **input** are
also shown. Yaw 0 faces north(-Z), positive yaw turns west; positive pitch looks up.
Negative block/chunk boundaries use floor and Euclidean division/remainder.
Local Y wraps within the 16-block vertical chunk. No render-origin position is used.

“Chunks loaded” counts resident voxel chunks, including empty chunks. “Mesh slots”
counts retained chunk mesh entries; it is **not** a frustum-visible/draw-call count.
Streamed empty geometry has no slot; legacy meshes may retain an empty slot.
No unavailable GPU timings, VRAM estimates, light diagnostics, absolute save paths,
usernames or environment values are shown. Unknown startup FPS/radius are labeled
unavailable rather than invented. FPS smooths valid real frame intervals with a 0.5s
response; zero/nonfinite samples are ignored and extreme stalls are bounded.

## Inventory, Pause and focus

**E** opens inventory and frees the pointer. The existing fixed 60-Hz simulation
continues: airborne players fall/land; collision, ground braking and ordinary air
momentum remain active. Inventory mouse/Shift-click/slot controls keep working.
Inventory input cannot rotate the camera, move, jump, respawn, mine or place.
Momentum already earned in air is retained by normal physics until collision or
landing; opening inventory is not an emergency midair brake.

Closing inventory recaptures the pointer when it was captured before opening.
Movement/sprint/sneak and jump keys held during modal/capture transitions need
release before reuse. Double-forward sprint needs a fresh pair of presses. Mouse
opening/closing clicks and pending jump/respawn commands cannot replay as actions.
Escape closes inventory first; another Escape opens Pause. Held-stack/no-room
handling and paired saving are unchanged.

**Pause** and **Options entered from Pause** freeze player physics and show the
rendered world through a 45% dark scrim, with opaque high-contrast buttons. The
main title, world catalog/management and Options entered from Title retain the
fully opaque pixel mosaic. Render order is world+sky/fog → hand/HUD → F3 → menus;
F3 and hand hide when menus open. Overlay/menu cameras never clear world color.

Focus loss, including with inventory open, enters Pause and freezes physics.
Returning focus alone does not resume. Explicit Resume returns to the retained
inventory if one was open. Loading/final saving/teardown never advance physics;
the existing worker/save/lock barriers and failure recovery remain in force.

## Movement measurements

Targets on ordinary flat ground (one block ≈ one metre):

| Mode | Target | Measured after acceleration, at 30/60/144 render FPS |
| --- | --- | --- |
| Walk | 4.317 blocks/s | 4.317000, cardinal and diagonal |
| Sprint | 5.612 blocks/s | 5.612000, cardinal and diagonal |
| Sneak | 1.295 blocks/s | 1.295000, cardinal and diagonal |

The Bevy adapter test warms up for one second, measures displacement for four
seconds, checks speed within 1e-7 and compares complete final player states exactly.
Ground acceleration/braking, air control, gravity, jump impulse, collision and
sprint FOV are unchanged. Sprint launch bonus scales from 1.2 to 0.8418 blocks/s,
retaining 15% headroom; cap 6.4538 prevents the previous 9.2 pace surviving the reduction.
The 100-block course from rest takes 23.200s walking, 17.867s sprinting and 15.733s
sprint-hopping (averages 4.310/5.597/6.356). Existing no-standing/roof/wall bonus,
normalized diagonal, support, ledge and collision assertions remain active.

Reproduce independently:

```sh
cargo test --locked -p rustcraft steady_ground_speeds -- --nocapture
cargo test --locked -p voxel_core flat_course_sprint_jumping -- --nocapture
cargo test --locked -p voxel_core twenty_block_movement_profile -- --nocapture
cargo test --locked -p rustcraft airborne_inventory -- --nocapture
cargo test --locked -p rustcraft debug_ui
cargo test --locked -p rustcraft overlay_cpu_cost_and_retained_buffer_size -- --ignored --nocapture
```

Raw simulation output: [steady speeds](performance/slice19/steady-speeds.txt),
[courses](performance/slice19/courses.txt). More tuning details: [MOVEMENT](MOVEMENT.md).

## Resources and performance

One global retained F3 mesh/material and two entities (surface + isolated camera)
are reused across worlds. The camera is inactive when hidden. World cleanup hides
it immediately and clears toggle/text state. A/B/A and repeated-session tests
compare resource counts after cleanup;1000 toggle/update cycles do not grow assets.
The existing menu keeps one mesh/material; only its alpha policy changes.

Hidden F3 does no sampling, formatting, geometry rebuild or draw. Visible F3 does
constant-time frame smoothing; text, geometry, resize and diagnostics update at
most 5 Hz (plus immediate show). Rebuilds replace buffers under the same asset handle.
Counts use map lengths, without world/chunk scans. The text buffer is reused;
bounded temporary glyph vectors are allocated on refresh, not every render frame.
One extra small blended UI pass is active only while F3 is visible.

CPU-only dev measurements and graphics evidence are recorded below. They do not
establish native GPU frame cost. Confirm matched F3-off/on/off scenes on the
owner's GPU before acceptance; include resolution, radius, build, GPU/driver and
X11/Wayland in the record. The existing streaming P50/P95/P99 log samples occur
when terrain work settles, not continuously on every idle overlay toggle.

## Void Linux glibc / Fish acceptance

Use the prerequisites in [PLATFORMS](PLATFORMS.md) and supported normal drivers.
Keep one isolated data root in the same Fish session for restarts; never use your
only real saves for initial acceptance:

```fish
git fetch origin
git switch feat/slice-19-debug-ui-movement
git pull --ff-only
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace
python3 tools/generate_textures.py --check
set test_dir (mktemp -d /tmp/rustcraft-slice19.XXXXXX)
echo $test_dir
env RUSTCRAFT_DATA_DIR="$test_dir" RUST_LOG=info cargo run --locked -p rustcraft
# After normal close, same root and a distant start:
env RUSTCRAFT_DATA_DIR="$test_dir" RUSTCRAFT_START_CHUNK="1000,-1000" RUST_LOG=info cargo run --locked -p rustcraft
```

Create A/seed42 and B/seed43 through the UI. Also test a release build by adding
`--release`. Back up **whole closed world directories**, including regional/player/
journal files. Do not delete journals/locks or use real saves for error injection.

- [ ] F3 outdoors/dark cave: readable at 960×540,1280×720,1920×1080; save HUD/hotbar/crosshair clear.
- [ ] Press/hold/repress F3 while moving and looking with captured mouse; no flicker, pause or cursor change.
- [ ] Cross X/Z0 and±16; verify negative blocks, chunk/local boundaries and far world coordinates.
- [ ] Jump/walk off a ledge, open E while falling, observe landing with inventory still open; Shift-click/left/right remain usable.
- [ ] Hold movement/jump/mouse while opening/closing E; no phantom movement, jump, mine/place or camera motion; release/repress restores controls.
- [ ] Escape closes E first; next Escape pauses. Pause midair and enter Options: frozen player/world visible behind dark scrim.
- [ ] Options from Title stays opaque; buttons/navigation, settings capture, Apply/Back and display rollback retain behavior.
- [ ] Alt-Tab with E open and closed: no falling/drift until explicit resume; cursor recaptures correctly.
- [ ] Walk/Ctrl/double-forward/sneak pace, diagonal normalization, repeated jumps, ledges/collisions and smooth sprint FOV feel correct.
- [ ] F5/autosave/save-and-title, A/B/A and restart retain blocks/inventory; F3 resets and resource counts return to baseline.
- [ ] Compare matched F3-off/on/off frames and long idle/resize/toggle sessions for resource growth or flicker.
- [ ] Repeat on Void X11 and Wayland and Windows 10/11 with native drivers/DPI; do not treat CI or Mesa as GPU acceptance.

## Validation and known limitations

Software X11 captures use the production client and original RustCraft assets;
they are not mockups. Native Void/Windows GPU, Wayland/compositor capture, DPI and
long-duration driver/VRAM acceptance remain manual. One-tick interpolation means
F3's authoritative position can lead the displayed camera by up to a fixed tick.
Diagnostics/resize may lag200 ms. This is a kinematic RustCraft controller with
Java-like target ground speeds, not complete Minecraft movement parity; no step-up,
flight, torch/block lighting or time-of-day behavior was added.

### Recorded cloud results

All five requested local validation commands passed on Rust 1.99.1/Bevy 0.20.0:
164 client tests + 165 core tests; the one ignored CPU measurement was run explicitly
and passed. [Validation record](performance/slice19/validation.txt).

A CPU-only debug-build MinimalPlugins fixture (no renderer, 20 ms synthetic render
step) measured median whole-App update 0.328 ms hidden / 0.496 ms shown across five
batches of 500 updates. This includes ECS scheduling and 5 Hz refreshes and is not
an isolated system or GPU benchmark. Representative geometry is 14,972 vertices /
22,458 indices / 688,712 logical bytes, with one mesh/material in both cases.
[Raw CPU/resource output](performance/slice19/debug-cost.txt). Actual text length
changes geometry size within the bounded number of diagnostic lines.

Production-client Xvfb/Mesa 25.0.7 llvmpipe (LLVM 19.1.7), 1280×720 captures:

| Scene | Evidence |
| --- | --- |
| Outdoor F3, captured mouse, sky/fog/hand/hotbar | [Screenshot](images/slice19-f3-outdoor.png) |
| Dark-cave F3, negative block/chunk/local positions | [Screenshot](images/slice19-f3-cave.png) |
| Translucent Pause, frozen rendered world | [Screenshot](images/slice19-pause.png) |
| Translucent Options entered from Pause | [Screenshot](images/slice19-ingame-settings.png) |
| Opaque Options entered from Title | [Screenshot](images/slice19-title-settings.png) |

The software-rendered client was also exercised through title/world selection,
mouse capture, inventory, jump, save/normal quit and reload. Software FPS in these
images includes llvmpipe/cloud contention; it is not representative of physical
GPU performance and cannot establish a before/after GPU regression threshold.

The airborne-inventory regression is verified tick-for-tick by automation. A
synchronized screenshot of inventory *during* flight was not captured reliably at
the software renderer's low frame rate; the captured inventory appeared after
landing. That specific airborne screenshot remains a native acceptance item.
