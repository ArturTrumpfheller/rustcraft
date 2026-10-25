# Slice 20 — torches and dynamic voxel block light

Original RustCraft source/art: MIT OR Apache-2.0. Based on accepted Slice19
`9eec01272d463c6eaa350bcbf3345d24eba6b53f`. The owner accepted the original Slice20 and the precise-selection/wall-mount refinement on native Void Linux. Final support-collection and held-pose changes below await final native acceptance.

## Playing and support rules

A freshly initialized player starts with **32 torches in hotbar slot6**. Select6
(or use the wheel). Right click a solid block's top face for a standing torch;
a horizontal face mounts it to that wall. Underside placement is rejected. Left click collects the target torch. Full inventory rejects breaking;
failed placements/no-ops never consume items. Existing initialized inventories
are loaded exactly and never receive a refill.

Torches are centered 0.1875-block-wide, 0.6875-block-high models: wood shaft and
bright amber tip, twelve quads total. They allow movement and both light channels,
are selectable only on their shaft/tip geometry, and do not hide neighboring cube faces
or contribute AO. Placement cannot replace occupied cells. Ceiling mounts,
particles, animated flames, sound, crafting and held moving lights are deferred.

**Mining a floor or wall collects its dependent torches automatically.** The
single action collects the support plus zero to five standing/wall torches. All
items must fit in inventory; otherwise neither world nor inventory changes.
Dependent chunks/regions must be available and the whole action must fit the
dirty-region budget. Unknown frontier data can temporarily defer the operation.
There are no dropped-item entities or implicit item loss. Trusted malformed
imports remain preserved rather than automatically repaired.

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
light bands, model winding/tip, shape targeting/collision, inventory failure/no-op
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
all, and place/remove a complete wall. Break a support: it must collect the block and all attached torches together,
or reject everything if the complete set does not fit. Repeat at X/Z0,±16,±64 and Y0/16. Alternate edits rapidly, change
radius1↔3, travel far enough to evict and return. Verify F5,>30-second autosave,
Save and Quit, reopen, exact counts and restored light. Check capture/release,
focus/pause/inventory, F3, nearest pixels/AO/fog and absence of holes/stale seams.

Repeat with render distances1–3 on native Void X11 **and** Wayland and Windows
10/11 (PowerShell: `$env:RUSTCRAFT_DATA_DIR = Join-Path $env:TEMP ([guid]::NewGuid())`,
then normal Cargo run and create a world). Record driver/backend, resolution,
VSync, build mode, F3 frame times and worker diagnostics. Software llvmpipe images
and CPU timing do not establish i3-13100/RX9060XT performance or native acceptance.
Keep the new directory for restarts; back up entire closed world bundles.

## Slice20 refinement: precise selection, wall mounts and held profiles

The owner accepted this refinement on native Void. The earlier held-pose images
are retained as historical comparisons; the final presentation below supersedes
their raised ordinary block and fully visible torch.

Selection, ray intersection, outline and wall model placement share
`voxel_core::shape`: the shaft and tip are two exact boxes in an orthonormal
model frame. Wall torches tilt about22° out from their wall and stay entirely
inside their voxel cell. The ray transforms into this frame, intersects each
box and returns the closest hit within reach. Empty space beside/between the
parts continues through the DDA to farther blocks. Cube tie ordering, inside hits
and max reach remain unchanged. `RayHit.surface_normal` is the exact tilted
normal; `normal` remains the dominant integer grid face for `adjacent` placement.

The single selection entity draws both model boxes with a small local expansion
to avoid coplanar flicker. Its retained mesh changes only when the selected shape
changes; moving the target or standing still does not allocate new meshes.
World torches still join chunk geometry with48 vertices/72 indices each.
No per-torch entity/draw-call, PointLight or new texture is added. Block storage
is still one byte, inventory Torch remains ID6 and lighting cache size/radius/
propagation/worker publication are unchanged. An orientation-only change reuses
identical light samples and forces only the existing geometry/AO footprint.

| Stable block ID | Mount | Support offset from torch | Clicked support face |
| --- | --- | --- | --- |
| 6 | Standing | (0,-1,0) | +Y |
| 7 | East | (-1,0,0) | +X |
| 8 | West | (+1,0,0) | -X |
| 9 | South | (0,0,-1) | +Z |
| 10 | North | (0,0,+1) | -Z |

This extends the same fail-closed world registry policy in v2/v3/v4 records;
standing ID6 data is unchanged. All five variants collect the one Torch item.
Player item IDs7–10 remain invalid. Unknown world IDs11+ remain invalid. No
new envelope, migration, journal or derived save data is introduced. Tests cover
all mounts, signed region support seams, eviction/reload and old standing saves.

`client::viewmodel::presentation` supplies position, scale, rotation, model-origin
correction and swing pivot for Arm, Block and Slender categories. Torches reuse
the standing chunk model directly, centered through their profile. Ordinary blocks
use the original Slice20 block pose; torches receive only a small lift and left
lean, with their lower shaft intentionally off screen. The original camera X/Y
FOV correction is preserved without additional aspect correction. The original
arm profile, gait cadence/amplitude, swing timing/travel and pivot curves are kept.
Profile selection is constant-time without frame allocations or mesh recreation.

Reproduce the new selection/mount gallery using the production terrain and
selection adapter (no save I/O):

```sh
cargo build --locked -p rustcraft --example lighting_gallery
python3 tools/capture_lighting.py --output /tmp/rustcraft-torch-refinement --scenes standing-selection east west south north
```

Native retest: use the existing isolated Slice20 world or a fresh directory from
the Fish commands above. Aim beside a standing torch at a block behind it; the
outline must follow only the visible shaft/tip. Place on each of four wall faces
and on top, reject undersides, collect each variant and test support protection.
Repeat across signed chunk/region seams and with F5/autosave/Save and Quit/reopen/
eviction. Check the full held torch and normal block at FOV70/78/90/100 with hotbar,
walk/sprint/crouch/swing and empty-arm selection. New native tests remain open.

### Refinement images and CPU comparison

| Fixture | Screenshot |
| --- | --- |
| Standing torch and exact box outline, dark room | [Standing](images/slice20-refinement/slice20-standing-selection.png) |
| +X wall mount | [East](images/slice20-refinement/slice20-east.png) |
| -X wall mount | [West](images/slice20-refinement/slice20-west.png) |
| +Z wall mount | [South](images/slice20-refinement/slice20-south.png) |
| -Z wall mount | [North](images/slice20-refinement/slice20-north.png) |
| Complete held torch, FOV78 and hotbar | [Torch](images/slice20-refinement/held-torch-fov78.png) |
| Matched camera/FOV78, ordinary held block | [Block](images/slice20-refinement/held-block-fov78.png) |

Selection/mount images use the production terrain/selection adapters at960×640.
Held images use the actual application at1280×720, the same seed42 world/camera
and saved FOV78 settings, changing only the selected slot. All captures use Xvfb
X11/Mesa llvmpipe software Vulkan. They do not establish native AMD performance.

An isolated release harness comparison against the previous PR head
`d5c9c171327f611d4e5a634cad4015682390d4b0` uses three interleaved process runs
after warmup, with no renderer/test suite running. The new mounts fixture gives
both versions identical floor/side supports; the old version uses standing
torches at the same emitter cells, so lighting/opacity and quad counts match.
[Raw per-run CSVs and environment](performance/slice20-refinement).

| Scene | Light+mesh µs before → after | Change |
| --- | ---: | ---: |
| No torches | 1296.53 → 1324.44 | +2.2% |
| One | 1241.23 → 1244.28 | +0.2% |
| Several | 1254.69 → 1399.67 | +11.6% |
| Many | 1331.06 → 1350.21 | +1.4% |
| Repeated edits | 1282.26 → 1187.51 | -7.4% |
| Signed seam | 1213.13 → 1192.22 | -1.7% |
| Four wall mounts plus one standing source | 1268.05 → 1309.14 | +3.2% |

Small cloud timings vary; the several-source regression is retained explicitly.
No general speedup or physical frame-time claim is made. The new orientations
keep one-byte block storage, identical resident light buffers and exactly48
vertices/72 indices per torch. A deterministic worker-core regression confirms
that changing only mount orientation preserves all cached light samples and
remeshes just the one affected interior geometry chunk. The selected outline
uses one retained asset (16 vertices/48 line indices for two boxes), not per-torch
assets or work on idle frames.

```sh
python3 tools/benchmark_lighting.py --baseline d5c9c171327f611d4e5a634cad4015682390d4b0 --output /tmp/rustcraft-refinement-bench
```

Known limits remain deliberate: no ceiling mounts or dropped-item entities,
RGB/moving hand light or multiplayer. Removing support can be deferred at an
unknown regional frontier; it never guesses away an unloaded dependent torch.
Imported malformed worlds are preserved, not automatically repaired. The tilted
surface normal is exposed separately from the grid normal used for adjacent-cell
placement. Held action animations can lower/sweep the model as before; equipped
poses and camera/UI clearance are tested. Native Void/Windows refinement testing
is still required even though the original Slice20 was accepted on Void.

## Final support-removal transaction and held presentation

One mining action preflights a fixed, deterministic batch of at most six changes:
the hit support, then its standing,+X,-X,+Z,-Z attachments. A dependent torch is
identified by its saved support offset; unrelated adjacent torches stay intact.
Every changed cell must be loaded/in bounds, all required regional authority must
be available, and the number of unique newly dirty regions plus existing dirty
regions must stay at or below8. This aggregate check prevents a primary-region
commit followed by failure at a neighboring region.

`edit_inventory` stages the complete support/Torch item set in a small inventory
clone, rejects any overflow, revalidates the plan and commits all world cells and
regional overlays without independent `set_block` calls. The batch has one world
revision; every touched region uses that revision. Geometry/light dependency
footprints are unioned, and the client marks every returned `EditBatch` change
before scheduling background work. Adjacent changes coalesce into the existing
worker path; stale results cannot publish a partial source removal. The inventory
is installed only after the infallible validated world commit. No whole-world
clone, recursive cascade, synchronous lighting or per-action disk journal is used.

The low-level single-cell `set_block`/`interaction::edit` API stays conservative
and rejects changes that would orphan torches. Gameplay mining must use the
inventory batch API and invalidate **all** returned changes. Saves capture the
coherent result through existing F5/autosave/final-save snapshots and paired redo
journals, with no ID/format change. Tests inject interruption after journal, world
and player stages across positive/negative regional seams, then replay all Air
edits and exact inventory counts after eviction/restart.

The BLOCK profile exactly restores accepted commit`d5c9c17`: position/pivot
(0.56,-0.52,-0.72), rotation(0,45,0), scale0.40. The extra aspect correction from
the prior refinement is removed, restoring original FOV/aspect behavior. ARM is
unchanged. SLENDER/Torch uses position/pivot(0.56,-0.46,-0.72), scale0.40 and
rotation(0,35,12), with the original model-origin correction(0.5,0.35,0.5). The
0.06 upward adjustment exposes some shaft; the bottom intentionally remains off
screen. The **projected** tip is left of the lower shaft; this is verified with
rendered images and tests, not inferred from an angle's sign. The model/textures,
Slice19 bob coefficients and shared swing timing/travel remain unchanged.

Native final check: mine a support with one standing torch, one wall torch and
all five attachments, including signed chunk/region seams. A nearly full inventory
with space for the support but too few torches must reject everything. Free enough
space and retry: collect exactly1 support +N torches, preserve unrelated light,
save/reopen and leave/return across eviction. Rapid edits must settle without
stale lighting or holes. Compare ordinary blocks to the original equipped pose;
inspect the modest, left-leaning, partly off-screen torch at FOV70/78/90/100.

### Final matched FOV78 comparison

| Item | Original Slice20 (`d5c9c17`) | Final |
| --- | --- | --- |
| Grass block | [Original](images/slice20-final/original-grass-fov78.png) | [Restored](images/slice20-final/updated-grass-fov78.png) |
| Torch | [Original](images/slice20-final/original-torch-fov78.png) | [Modest lift / left lean](images/slice20-final/updated-torch-fov78.png) |

Captured from the actual original binary built in a clean detached`d5c9c17`
worktree and the updated application, using two identical disposable copies of
the same seed42 world/settings,1280×720,FOV78, radius1, matching camera and hotbar.
The original and restored grass screenshots have **identical pixels over the
entire image**. The updated torch shows some wooden shaft while its bottom remains
off screen; visual inspection confirms the tip leans left relative to the shaft.
FOV70/78/90/100 and16:9,3:2,4:3 projection tests preserve the exact original block
lens/pose and check the torch's visible shaft, partial crop and leftward screen
lean. Captures use Xvfb X11/Mesa llvmpipe Vulkan, not a native AMD benchmark.

Local validation:173 client +192 core tests passed; format, all-target Clippy,
locked workspace build and texture artifact checks passed. Existing wall mounting,
precise selection, lighting, greedy meshing, scene/worker safeguards and save
fixtures remain covered. No geometry, texture, ID, format, generator, lighting
buffer, bob coefficient or shared swing curve changed in this final adjustment.
[Validation evidence](performance/slice20-final/validation.txt).
