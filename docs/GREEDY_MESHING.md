# Slice 16 — Greedy terrain meshing

The engine-neutral mesher first collects visible opaque faces in storage order,
then visits the six orientations, slices and rows in a fixed order. Each mask
cell has a merge key (atlas tile, exact integer skylight sample). Grow width,
then height while keys match, consume that rectangle and emit four vertices and
two CCW triangles. Orientation is implicit in the plane; all present blocks share
the opaque material policy. Grass-bottom and Dirt may merge because their render
properties are identical. Future AO, block-light or render flags belong in the
merge key, including all corner values if light ever becomes nonuniform.

Visibility and lighting still query authoritative blocks and the same adjacent
Air sample. No lighting propagation, generator, save format, physics, inventory,
worker scheduling, dependency tokens or floating-origin arithmetic changed.
Changes in light may split/merge rectangles even when unit-face coverage stays
the same. Worker results retain the existing batch publication and invalidation
rules. Existing mesh handles remain in use; empty streamed meshes are removed.

There are fixed 6×16×256 mask entries on the worker stack and a 16-bit active-slice
mask per orientation; there are no heap allocations for search scratch. Empty
slices are skipped. Output vectors retain geometric growth. Output capacity is
measured below; neither allocator metadata nor total process RSS is included.

## Repeating the existing atlas

`ChunkMesh.uvs` now holds block-repeat coordinates. `atlas_uvs` is the constant
normalized tile origin, replicated at all four vertices. Walls and horizontal
faces retain their original CCW corner order, U/V directions, grass face choice
and bark/end-grain mapping. Every rectangle starts on the integer block grid,
so fractional coordinates agree across rectangles and signed chunk seams.

The terrain-only `ExtendedMaterial<StandardMaterial, AtlasRepeat>` uses the
embedded `client/src/terrain.wesl` shader through Bevy 0.20's MaterialPlugin.
It computes `tile_origin + fract(repeat_uv) * (16/128, 16/64)`. Explicit
`textureSampleLevel(..., 0)` avoids fract derivative discontinuities. This is
compatible with the existing one-mip, nearest, no-anisotropy, padded sRGB atlas.
No mip chain is synthesized; distant minification shimmer remains intentional.
Eight extruded edge texels still protect sampling from neighboring tiles.

The shader multiplies sampled linear color by the unchanged directional/skylight
vertex RGBA and uses Bevy's existing post-lighting processing (including camera
tonemapping). Terrain remains opaque, depth-tested and back-face culled. There
is one shared terrain material/image and one mesh per nonempty rendered chunk.
The shader is embedded; running the binary does not require a shader directory.
UI, inventory and held blocks keep StandardMaterial and their original atlas UVs.
Session cleanup tracks the new material asset type explicitly.

An uncompressed vertex grows from 48 to 56 bytes; a quad including u32 indices
therefore grows from 216 to 248 bytes. Reduction depends on actual merging. A
checkerboard with incompatible properties can cost more memory than before.

## Reproducible measurements

Measured 2026-10-24 on Debian 13 x86-64, Intel Xeon Platinum 8573C, five exposed
CPUs, Rust 1.99.1, release optimization. No renderer or other test suite ran during
the final comparison. Initial baseline measurements were recorded before the
mesher change. The final matched run reconstructs the unchanged core sources at
`535c6364ac30eca641ca54a2496656fd2382d8be` in a temporary standalone Cargo package;
it does not create a Git checkout or switch branches. Both run the same harness,
with only 48/56-byte buffer accounting adapted for the old/new API.

```sh
python3 tools/benchmark_meshing.py --baseline 535c636 --output /tmp/rustcraft-mesh-results
# Or measure only the current implementation:
cargo run --locked --release -p voxel_core --example meshing_bench
```

Use a new output directory. The comparison needs cached core dependencies and
runs Cargo offline. CPU values are the median of five samples, each averaging
100 batches including mesh creation/destruction. World generation and lighting
are excluded. Synthetic solid/flat/mixed/cave/tree fixtures have real skylight;
mixed uses stepped grass/dirt/stone. Edits starts from generator-v3 seed 2026,
applies alternating edits, then repeatedly changes a cell and recomputes light
outside each timed mesh pass. Geometry columns describe its initial edited state,
not an average over the edit cycle. Stream meshes all 27 chunks of a 3×3×3 seeded
neighborhood per batch; it measures the worker's mesh phase, not I/O, generation,
lighting or end-to-end streaming latency. Existing streaming regressions cover
actual async lifecycle and repeated seam updates, without claiming a new
end-to-end timing benchmark.

Raw [before](performance/slice16/before.csv) and [after](performance/slice16/after.csv)
results include triangle counts (two per quad) and vector capacities. Percentages
are `(after / before - 1) * 100`; negative means a reduction.

| Scene | Quads before → after | Vertices before → after | Payload bytes before → after | Payload change |
| --- | ---: | ---: | ---: | ---: |
| solid | 1536 → 34 | 6144 → 136 | 331776 → 8432 | -97.5% |
| flat | 256 → 1 | 1024 → 4 | 55296 → 248 | -99.6% |
| mixed | 528 → 148 | 2112 → 592 | 114048 → 36704 | -67.8% |
| cave | 842 → 13 | 3368 → 52 | 181872 → 3224 | -98.2% |
| trees | 580 → 68 | 2320 → 272 | 125280 → 16864 | -86.5% |
| edits | 535 → 271 | 2140 → 1084 | 115560 → 67208 | -41.8% |
| stream | 14266 → 4176 | 57064 → 16704 | 3081456 → 1035648 | -66.4% |

| Scene | CPU µs before → after | CPU change | Allocated vector bytes before → after |
| --- | ---: | ---: | ---: |
| solid | 171.74 → 100.45 | -41.5% | 442368 → 15872 |
| flat | 30.20 → 29.66 | -1.8% | 55296 → 248 |
| mixed | 46.98 → 46.00 | -2.1% | 221184 → 63488 |
| cave | 61.91 → 48.15 | -22.2% | 221184 → 3968 |
| trees | 32.37 → 23.18 | -28.4% | 221184 → 31744 |
| edits | 75.53 → 80.78 | +6.9% | 221184 → 126976 |
| stream | 1857.57 → 1887.27 | +1.6% | 4371840 → 1406408 |

A standalone solid chunk produces 34 rectangles under real, nonuniform skylight,
compared with 1,536 original faces. The uniform-light regression fixture produces
exactly six. Lighting discontinuities deliberately prevent further merges.

CPU is not uniformly improved: the repeated edit workload regresses and the
27-chunk batch is roughly unchanged/slightly slower. Large geometry reductions
are verified, but no GPU hardware FPS/frame-time improvement is claimed. Software
rendering is functional acceptance, not a comparable GPU performance benchmark.

## Validation and visual evidence

Passed locally: locked workspace build; formatting; Clippy all targets with
warnings denied; 147 client + 155 core tests (zero doctests separately); original
texture artifact check; release benchmark and baseline comparison tool.
The new coverage oracle expands rectangles into unit faces and checks exact
visibility, no overlaps, tile and light values, normals, winding and repeat UV
lengths. Fixtures cover empty/solid/planes/prisms/irregular mixed materials/hollow
rooms and tunnels, signed coordinates, finite X/Z edges and all six chunk seams.
Existing suites cover roofs opening/closing, stale async results, repeated edits,
radius changes, floating origin, save/restart and inventory. Session asset tests
now include the terrain material type.

The unmodified application was run through Xvfb X11 + Mesa llvmpipe Vulkan.
Generated-world screenshots use the same saved seed and spawn view:
[before](images/slice16-forest-before.png), [after](images/slice16-forest-after.png).
Grass, trees, bark/end-grain, inventory icons and the held block were inspected.
A disposable authored snapshot tests a large stone wall crossing a signed X seam,
flat grass, roof and a dark walk-in room:
[before](images/slice16-gallery-before.png), [after](images/slice16-gallery-after.png),
[inside](images/slice16-chamber.png). Stone repeats per block rather than stretching;
light bands and wall/floor alignment remain visible. The selected held item differs
between the gallery overview captures. Normal menu exit and restart succeeded.
No shader errors remained after correcting WESL binding-constant syntax.

Create this fixture with the repeatable helper (which refuses existing output):

```sh
cargo run --locked -p voxel_core --example meshing_gallery -- /tmp/rustcraft-gallery
RUSTCRAFT_DATA_DIR=/tmp/rustcraft-gallery cargo run --locked -p rustcraft
```

Select Singleplayer → legacy entry → Inspect → Import a copy → confirm → Play.
These are disposable fixture saves, never instructions to replace a real world.

Not verified here: Windows/Ubuntu remote CI outcomes, native Void GPU/Wayland,
physical-driver T-junction rasterization, representative mountain screenshots,
interactive long edit/flicker stress or measured GPU FPS. Exact pixel equality is
not claimed: changed primitive tessellation and exact tile-edge sampling can
alter boundary pixels. Native play-testing remains required before acceptance.
No new dependencies, art assets or shader framework were introduced.

## Native Void Linux acceptance (Fish)

```fish
cargo build --locked --workspace
set -l scratch (mktemp -d)
cargo run --locked -p voxel_core --example meshing_gallery -- "$scratch/gallery"
set -lx RUSTCRAFT_DATA_DIR "$scratch/gallery"
cargo run --locked -p rustcraft
```

Use native driver/backend discovery; do not set the cloud software-adapter
variables. Import the fixture copy, inspect wall/roof/floor repetitions and the
signed X seam from both sides, then walk into the room. Mine and replace the
roof near its aperture: light boundaries must update without holes or flashes.
Place a large stone surface and check all six directions, grass and log faces.
Save/close/restart and confirm edits and inventory. In a separate seeded world,
walk through trees and cave entrances and switch radius 1↔3 through Options.
Repeat on X11 and Wayland and record GPU/driver/backend and frame times.
