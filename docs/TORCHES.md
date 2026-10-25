# Slice 20 — torches and dynamic voxel block light

Original RustCraft source/art: MIT OR Apache-2.0. Based on accepted Slice19
`9eec01272d463c6eaa350bcbf3345d24eba6b53f`. Native owner acceptance remains open.

## Playing and support rules

A freshly initialized player starts with **32 torches in hotbar slot6**. Select6
(or use the wheel), aim at a solid floor and right click. A torch can also be
placed against a side when its destination has an appropriate loaded solid block
underneath. Left click collects the target torch. Full inventory rejects breaking;
failed placements/no-ops never consume items. Existing initialized inventories
are loaded exactly and never receive a refill.

Torches are centered 0.1875-block-wide, 0.6875-block-high models: wood shaft and
bright amber tip, twelve quads total. They allow movement and both light channels,
remain targetable throughout their cell, and do not hide neighboring cube faces
or contribute AO. Placement cannot replace occupied cells. Wall/ceiling mounts,
particles, animated flames, sound, crafting and held moving lights are deferred.

**Break the torch before mining its floor.** Support removal is refused while a
torch is above it; when the upper cell is pending, retry once it loads. Support
must be within world bounds and loaded; y=-16 cannot host a floor torch. This
bounded single-cell policy avoids automatic multi-block removals, item drops and
new journal transactions. Trusted imports/cache installs do not validate support;
malformed authored worlds may contain floating torches, but their light is still
derived deterministically. There is no automatic support repair/drop system.

## Light, meshes and persistence

Torch emission is14. Six-connected propagation loses1 per step, stops at0 and
opaque blocks, and combines multiple sources by maximum. Air and torch cells
transmit light. Pending cells stop it; unloaded edits are materialized by the
existing captured regional/generator halo on the worker. Removing a source or
changing a wall recomputes the bounded field, so other sources remain correct.

Skylight retains its original independent rules. Terrain uses
`max(skylight, blocklight)` before the existing direction and AO modulation;
placing a torch cannot darken sunny terrain. Neutral lighting is intentional;
warmth comes from the original flame art. The tip has full vertex brightness.
This is voxel light, with no GPU point lights or shadow-map work.

The single terrain worker reuses the original full-height, 15-block horizontal
halo, dirty footprints, edit coalescing, exact cache comparisons and local
revision tokens. Cached bytes pack sky in the low nibble and block light in the
high nibble. Stale batches cannot publish samples/meshes or clear newer work;
old complete meshes remain until the replacement publishes atomically. Greedy
keys require equivalent combined brightness, tile and AO. Models join chunk
meshes using the same atlas/material and preserve floating-origin/fog behavior.
There are no world scans on idle frames or synchronous propagation in input.

The explicit [save extension policy](SAVE_FORMAT.md#slice-20-additive-material-registry-extension)
adds stable block/item ID6 without reassigning IDs0–5 or changing envelopes,
generators, identities or journal layout. v1 stays frozen; v2/v3/v4 region/player
records accept6. Old readers reject unknown IDs. Loading old worlds never rewrites
or migrates them. F5/autosave/final save retain the paired redo/backup guarantees;
lighting never enters authoritative save bytes. Never open the same initialized
world directory with an older client; use complete closed-directory copies.

## Matched renderer fixture

The shared authored room has a sealed roof, wall partition variant and multiple
sources. Its wooden floor line marks X=0, a signed chunk boundary. All six images
use the production mesh/atlas/material and **the same 960×640 camera** at (9,4,6),
looking toward(-2,2,-3), Reinhard tonemapping and disabled fog for indoor comparison.
Captured on Xvfb X11, Mesa llvmpipe Vulkan, not a physical AMD GPU.

| State | Screenshot |
| --- | --- |
| Before lighting | [Dark room](images/slice20/slice20-dark.png) |
| One torch | [One source](images/slice20/slice20-one.png) |
| Three torches | [Overlapping sources](images/slice20/slice20-several.png) |
| Last torch placed then removed | [Dark again](images/slice20/slice20-removed.png) |
| Full opaque partition at X=0 | [Wall blocks light](images/slice20/slice20-wall.png) |
| Source at X=-1, light crosses X=0 | [Chunk seam](images/slice20/slice20-seam.png) |

Reproduce in a working desktop session (no saves are opened or changed):

```sh
cargo build --locked -p rustcraft --example lighting_gallery
cargo run --locked -p rustcraft --example lighting_gallery -- one
python3 tools/capture_lighting.py --output /tmp/rustcraft-slice20-images
```

The optional gallery output argument writes a raw PPM screenshot; the Python
helper losslessly encodes PNGs using the standard library, refusing overwrites.
The `dark` and `removed` captures have identical pixels.

## CPU and memory measurements

Cloud Linux x86-64, Intel Xeon Platinum8573C, Rust1.99.1, optimized core harness.
No renderer/test suite ran during the final comparison. The helper reconstructs
Slice19 core sources in a disposable package, uses cached dependencies offline,
warms each scene, and reports the median of three interleaved process runs. Each
run takes the upper median of six samples,20 batches/sample. Both versions use the
same eight-chunk room shell; baseline cannot contain torches and replaces them
with Air. **Only the no-source row is a matched old/new capability comparison.**
Other rows measure the cost of the new feature, not equivalent baseline lighting.

| Scene | Light µs | Mesh µs | Quads | Channel payload bytes |
| --- | ---: | ---: | ---: | ---: |
| Slice19, no torches | 787.81 | 249.20 | 231 | 279744 |
| Slice20, no torches | 916.73 | 238.04 | 231 | 279744 |
| One torch | 889.38 | 293.87 | 779 | 559488 |
| Three overlapping torches | 845.27 | 318.70 | 1304 | 559488 |
| Many nearby torches (67 unique cells) | 885.06 | 347.93 | 2099 | 559488 |
| Repeated placement/removal | 831.98 | 260.65 | 779* | 559488* |
| Signed chunk seam | 844.62 | 282.64 | 779 | 559488 |

*Edit timings average alternating dark/lit states; payload/quads describe the
last lit iteration. No-source light CPU rises16.4%; combined light+mesh rises11.4%
(1.037→1.155ms). This overhead is recorded, not a performance improvement claim.
Scene/order noise explains small nonmonotonic differences; raw per-run values are
retained in [performance/slice20](performance/slice20). Geometry deliberately
splits at light bands. More dense-room meshing/incremental propagation is follow-up
work; no shader/RGB architecture was added to mask the cost.

The separate debug-build worker harness exercises radii1–3, empty/one/several/many
sources,80 rapid toggles and signed seam edits through actual worker publication.
[Raw worker results](performance/slice20/worker.csv): batches remesh2–4 chunks,
coalesce into one job, and dirty-target peaks stay12–48. Largest batch wall time
is68.9ms, max recorded dirty latency65.4ms and max main-thread update1.01ms.
These are CPU-only ECS timings without winit/rendering, not display frame times.
The workload adds sources cumulatively (the many-source case has55 torches);
no-source work changes one opaque roof material to trigger the normal mesh path.

Exact resident samples remain5832 bytes/chunk:157464/437400/857304 bytes at
radii1/2/3. Worker channels use one byte/cell without emitters, two with emitters;
maximum967872 cells gives1935744 channel bytes plus opacity, queue, source list,
source world and mesh scratch. No extra artificial buffer is allocated without
sources. Models cost2976 logical terrain bytes/torch (12×248), before allocator/
driver overhead; atlas size and vertex stride remain unchanged. Native frame-time,
GPU/VRAM and resident process RSS comparisons remain unmeasured.

```sh
python3 tools/benchmark_lighting.py --output /tmp/rustcraft-light-bench
cargo test --locked -p rustcraft torch_worker_measurements -- --ignored --nocapture
```

## Validation and native acceptance

Deterministic tests cover emission/properties/IDs, exact attenuation, overlap and
removal, opaque partition opening/closing, signed X/Y/Z seams, world-height limits,
pending cells, independent skylight, deterministic insertion/batch order, greedy
light bands, model winding/tip, cell targeting/collision, inventory failure/no-op
atomicity, real slot6 clicks, paired save/reopen without refill, v2/v3 round trips,
regional signed-seam eviction/reload and rapid edits against a blocked stale worker.
Historical save/generator goldens, corruption/recovery and all existing worker,
viewmodel, AO, fog, physics and inventory regressions remain in the workspace suite.
Automated commands and final counts are recorded in performance/slice20/validation.txt.

On **Void glibc / Fish**, in a normal X11 or Wayland desktop with native drivers:

```fish
git fetch origin
git switch feat/slice-20-torches-blocklight
cargo build --locked --workspace
set -l torch_dir (mktemp -d /tmp/rustcraft-slice20.XXXXXX)
echo $torch_dir
env RUSTCRAFT_DATA_DIR="$torch_dir" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=2 cargo run --locked -p rustcraft
# Create a NEW world in Spielen; after normal Save and Quit, same directory:
env RUSTCRAFT_DATA_DIR="$torch_dir" RUSTCRAFT_LOAD_RADIUS=2 cargo run --locked -p rustcraft
```

Select6, inspect icon/hand, walk through placed torches and test full/empty slots.
Make a sealed room or enter a cave, place overlapping sources, remove one then
all, and place/remove a complete wall. Break a support: it must refuse until the
torch is removed. Repeat at X/Z0,±16,±64 and Y0/16. Alternate edits rapidly, change
radius1↔3, travel far enough to evict and return. Verify F5,>30-second autosave,
Save and Quit, reopen, exact counts and restored light. Check capture/release,
focus/pause/inventory, F3, nearest pixels/AO/fog and absence of holes/stale seams.

Repeat with render distances1–3 on native Void X11 **and** Wayland and Windows
10/11 (PowerShell: `$env:RUSTCRAFT_DATA_DIR = Join-Path $env:TEMP ([guid]::NewGuid())`,
then normal Cargo run and create a world). Record driver/backend, resolution,
VSync, build mode, F3 frame times and worker diagnostics. Software llvmpipe images
and CPU timing do not establish i3-13100/RX9060XT performance or native acceptance.
Keep the new directory for restarts; back up entire closed world bundles.
