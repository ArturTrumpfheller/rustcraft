# Slice 17 — Voxel ambient occlusion

Based on merged Slice 16, `ed0c3e48f2f8fcddf7e500d6b4062f8356d4c32e`.
AO is derived presentation data in `voxel_core::mesh`, independent of Bevy.
No dependency, block, generator, world/player format, input setting or texture
artwork changes. The existing shared terrain material consumes vertex COLOR;
there are no additional passes, per-block draws or material instances.

## Sampling and brightness

For each visible unit face of a solid block at `p`, start at `p + normal`.
At each vertex, sample the two tangent side cells toward that vertex and their
diagonal, all in that outside layer. All current solid blocks, including Leaves,
are opaque. If both sides are solid, occlusion is 3 regardless of the diagonal;
otherwise it is the number of solid samples (0–3). Same-layer neighboring floor
blocks do not darken an unobstructed flat top or outline every block.

A temporary 18³ occupancy halo includes all 26 neighboring chunks. It queries
each chunk once and copies only the required cells, avoiding world-map queries
per vertex. Sampling offsets are precomputed for the six orientations. Scratch
is fixed: 5,832 occupancy bytes, 288 offset bytes, plus Slice 16's fixed
6×16×256 merge mask with four additional AO bytes per key. There is no AO cache
per resident chunk and no per-frame recomputation for unchanged terrain.

| Occlusion | Open-sky multiplier | Level-0 multiplier |
| --- | ---: | ---: |
| 0 | 1.000 | 1.000 |
| 1 | 0.920 | 0.972 |
| 2 | 0.840 | 0.944 |
| 3 | 0.760 | 0.916 |

The factor is `1 - occlusion * 0.08 * (0.35 + 0.65 * skylight / 15)`.
It multiplies the existing linear RGB `face_color`; alpha stays 1. The light
solver, exact 0–15 gameplay samples, directional face factors and cave floor
are unchanged. AO strength fades in darkness to preserve texture legibility.
Skylight remains flat per unit face as in Slice 16; this slice does not replace
it with smooth lighting. Existing skylight transitions can still be visible.

## Greedy compatibility and triangulation

The merge key is `(atlas tile, exact skylight, four AO counts)`, with orientation
and slice implicit in the plane. Only faces with **constant AO at all four
corners** may grow, and all keys in the rectangle must match. Equal nonuniform
tuples alone are insufficient: repeating a gradient across unit faces is not
the same field as stretching one gradient over a large rectangle.

Nonuniform faces remain unit quads. This conservative rule preserves every
corner and internal block boundary exactly; constant shading interpolates
identically at every point on a merged rectangle. Uniform occluded strips can
still merge, as can large open surfaces. Some valid affine gradients could be
merged by a more complex algorithm; deliberately retaining those unit quads is
the main geometry limitation. The measured reductions versus individual faces
remain substantial (see below).

Unit quads connect the darker opposite vertex pair: flip from diagonal 0–2 to
1–3 when `ao[0] + ao[2] < ao[1] + ao[3]`; ties keep Slice 16's diagonal.
Both index patterns are outward CCW. Normals, visibility, face corner order,
repeat UVs and constant atlas origins retain Slice 16's contracts. The shader,
nearest LOD0 sampling and original padded 16×16 artwork are unchanged. Vertex
stride remains 56 bytes; four vertices and six u32 indices cost 248 bytes/quad.

## Chunk seams, edits and workers

`BlockChange::affected_chunks` now covers the one-cell Chebyshev neighborhood
of an edited block. An interior edit forces one geometry/AO chunk; a face, edge
or corner edit forces at most two, four or eight, respectively. This is not an
unconditional 27-chunk rebuild. Euclidean coordinate splitting and checked
offsets handle signed seams. The existing full-height skylight candidate set
is unchanged and still skips meshes whose exact samples are unchanged.

The AO footprint feeds both `DirtyChunks` geometry forcing and streamed local
revision tokens. This is essential: changing dark Air to solid can leave every
cached light sample identical while changing a diagonal chunk's AO. Those
chunks must still remesh. Existing one-worker coalescing, connected atomic
publication, stale-result rejection, retained handles and eviction rules remain.
Legacy workers retain their existing complete-world revision check.

The existing worker snapshot/light-source halo already includes a full square
of neighboring columns, full height and diagonal region overlays. It therefore
supplies all AO dependencies without expanding region I/O or generation bounds.
Absent legacy data is Air; pending in-volume streamed data is opaque. Production
workers resolve missing sources from generation plus captured authoritative
edits before meshing; failed regional reads retain old visuals/safe barriers.
Outside the finite world is known Air under the existing contract. Unrepresentable
coordinate offsets are ignored. Streaming/reloading computes AO from current
authority, never from an old mesh or a pristine generator in place of edits.

## Performance against Slice 16

Measured on 2026-10-24, Debian 13 x86-64, AMD EPYC 9V74 (three exposed CPUs),
Rust 1.99.1, release optimization. No renderer or other test suite ran during
this final mesh-only comparison. Both versions use the same existing fixture and
timing harness; the baseline is reconstructed from the exact merged commit in
an isolated temporary Cargo package. Raw [before](performance/slice17/before.csv),
[after](performance/slice17/after.csv) and [base revision](performance/slice17/baseline.txt).

```sh
python3 tools/benchmark_meshing.py --baseline ed0c3e4 --output /tmp/rustcraft-ao-results
cargo run --locked --release -p voxel_core --example meshing_bench
```

Use a new output directory. CPU is the median of five samples, each averaging
100 batches, including buffer construction/destruction. Generation and shared
skylight solving are excluded. Edits repeatedly changes a block and recomputes
light outside the timed mesh pass; its geometry counts describe the initial
edited state. Stream meshes 27 seeded chunks per batch, not disk I/O, generation,
light propagation or end-to-end streaming latency. These are CPU measurements,
not GPU timestamps or FPS. Payload includes all vertex attributes and u32 indices;
capacity also includes vector spare capacity, but not allocator overhead, retained
Bevy copies, driver allocations or process RSS.

| Scene | Quads 16 → 17 | Vertices 16 → 17 | Payload bytes 16 → 17 | Capacity bytes 16 → 17 |
| --- | ---: | ---: | ---: | ---: |
| solid | 34 → 34 | 136 → 136 | 8432 → 8432 | 15872 → 15872 |
| flat | 1 → 1 | 4 → 4 | 248 → 248 | 248 → 248 |
| mixed | 148 → 456 | 592 → 1824 | 36704 → 113088 | 63488 → 126976 |
| cave | 13 → 155 | 52 → 620 | 3224 → 38440 | 3968 → 63488 |
| trees | 68 → 116 | 272 → 464 | 16864 → 28768 | 31744 → 31744 |
| edits | 271 → 421 | 1084 → 1684 | 67208 → 104408 | 126976 → 126976 |
| stream | 4176 → 6236 | 16704 → 24944 | 1035648 → 1546528 | 1406408 → 2233736 |

| Scene | CPU µs 16 → 17 | CPU change |
| --- | ---: | ---: |
| solid | 91.89 → 147.04 | +60.0% |
| flat | 27.23 → 42.89 | +57.5% |
| mixed | 41.92 → 89.37 | +113.2% |
| cave | 44.96 → 78.64 | +74.9% |
| trees | 20.69 → 39.58 | +91.3% |
| edits | 68.37 → 117.91 | +72.5% |
| stream | 1662.11 → 2628.73 | +58.2% |

AO increases CPU time in every measured scene. The 27-chunk mesh phase grows
by about 0.97 ms (+58.2%); payload grows by 49.3%. Cave geometry has the largest
relative regression: 13 → 155 quads. The same fixture has 842 individual visible
faces, so it still reduces those by 81.6%. Trees retain an 80.0% reduction from
580 unit faces; seeded streaming retains a 56.3% reduction from 14,266 faces;
flat remains one quad for 256 faces. The stepped mixed fixture falls from a
72.0% reduction to only 13.6% (456 quads for 528 faces). The conservative merge
rule intentionally spends that geometry to preserve shading. Uniform solid/flat
geometry counts remain unchanged.

The bounded halo, precomputed face offsets and one base skylight/color calculation
per emitted quad avoid redundant map lookups and four repeated skylight evaluations.
No unchanged terrain is reprocessed per frame. The new fixed mask/halo scratch
and larger output remain radius-bounded. These costs are acceptable for this
foundation's correctness, but native frame/driver acceptance is still required;
no FPS improvement or real-hardware performance guarantee is claimed.


A separate headless debug streaming test (`repeated_seam_mining_workload`, one
test thread, no renderer) performed 32 coherent edit batches across a seam:
1.184 → 1.392 seconds wall clock (+17.6%), with 32 installs, zero discarded jobs,
22 final meshes and 27 resident chunks in each version. Its assertions verify
128 changed-chunk mesh installs, no cached-column regeneration and restored
initial asset/buffer counts. These are single-run observations, not statistical
latency estimates. [Raw observations](performance/slice17/edit-worker.txt).

## Automated and graphical validation

On 2026-10-24, Rust 1.99.1 / Bevy 0.20.0, Debian 13 x86-64:

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — passed.
- `cargo test --locked --workspace` — passed: 148 client + 162 core tests;
  zero doctests. Example texture-module tests are also checked by all-target Clippy.
- `cargo build --locked --workspace` — passed.
- `python3 tools/generate_textures.py --check` — passed; artwork unchanged.
- `cargo build --locked -p rustcraft --example ao_gallery` — passed.
- Matched release meshing benchmarks — completed; raw results linked above.

The rectangle oracle compares exact unit-face coverage, no duplicates, tile
origins, repeat lengths, normals and both triangle windings against separately
queried world occupancy. It evaluates emitted triangle interpolation at every
unit corner, including internal rectangle boundaries. All merged rectangles
must be constant, proving their entire field matches the unit-face reference.
Fixtures include isolated/flat/solid/hollow/irregular shapes, mixed materials,
inside/outside corners, all eight occupancy combinations, six orientations,
signed X/Y/Z seams, finite boundaries, pending neighbors, diagonal dependencies,
compatible uniform and incompatible repeated gradient patterns, both diagonals,
and AO/skylight attenuation. Placement/removal cycles restore identical meshes.

A real client streaming regression creates a sealed pocket crossing X/Y/Z=0.
A diagonal occupancy edit changes vertex colors while the target's entire exact
light cache remains equal. It verifies forced remeshing, rapid repeated edits,
stale worker rejection, no regeneration of cached columns, eviction and reload
against fresh authoritative meshes. Existing save, inventory, physics, worldgen,
radius/resource-lifetime and skylight suites continue passing.

### Matched screenshots

`client/examples/ao_gallery.rs` uses the actual production terrain material,
embedded shader, atlas, skylight and mesh conversion with fixed cameras. It
never opens saves or changes gameplay. The same fixture source was built once
against exact Slice 16 and once against this implementation. Captures are
960×640, Xvfb X11 / Mesa 25.0.7 llvmpipe (LLVM 19.1.7), Vulkan software rendering.
They demonstrate shader execution and the original art, **not native GPU
compatibility or GPU performance**. Camera, geometry and lighting inputs match.

| Scene | Slice 16 | Slice 17 |
| --- | --- | --- |
| A: Flat grass and placed blocks | [Before](images/slice17-flat-before.png) | [After](images/slice17-flat-after.png) |
| B: Stone interior corners/overhang | [Before](images/slice17-structure-before.png) | [After](images/slice17-structure-after.png) |
| C: Enclosed room and skylight aperture | [Before](images/slice17-room-before.png) | [After](images/slice17-room-after.png) |
| D: Trees meeting terrain | [Before](images/slice17-trees-before.png) | [After](images/slice17-trees-after.png) |

Inspected all four pairs: contact corners and tree bases darken subtly; the room
retains readable original stone pixels. Open flat areas keep uniform shading and
per-block texture repetition. No persistent signed X/Z seam or oversized-quad
gradient was seen in these fixed scenes. This is a bounded visual observation,
not exhaustive native rasterization coverage. Edits/stale results were validated
through real client schedules in automated tests, not an interactive GPU stress
run. Each captured frame was nonblack and stable across successive screenshots;
all four before/after pairs differ.

Reproduce a current fixture in a desktop session:

```sh
cargo run --locked -p rustcraft --example ao_gallery -- structure
# Other arguments: flat, room, trees.
```

## Native Void Linux acceptance (pending)

Use Void glibc x86-64 with normal supported drivers, separately on X11 and
Wayland. Cloud software rendering does not replace these checks. In Fish:

```fish
git fetch origin
git switch feat/slice-17-ambient-occlusion
set ao_test_dir (mktemp -d /tmp/rustcraft-slice17.XXXXXX)
env RUSTCRAFT_DATA_DIR="$ao_test_dir" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=2 cargo run --locked -p rustcraft
# Keep the same isolated directory for save/restart checks.
```

- [ ] Inspect flat grass, placed blocks, stone wall/floor intersections, roof
  undersides, cave corners and tree bases; textures remain sharp and caves readable.
- [ ] Inspect all six face orientations at X/Z 0, ±16, ±64 and Y 0/16. Place and
  remove blocks at chunk edges/corners; diagonally neighboring shading must update.
- [ ] Rapidly alternate edits while crossing seams; no holes, duplicate faces,
  stretched gradients, stale AO or persistent seams after workers settle.
- [ ] Travel out of radius and back, change radius 1–3, then inspect distant signed
  coordinates; resources remain bounded and edited terrain returns correctly.
- [ ] Check mouse capture, inventory counts, normal movement/collision, menus,
  F5/autosave, normal close and restart using the same isolated directory.
- [ ] Compare matched scenes on physical GPU; record driver/backend, resolution,
  build mode, radius, worker latency, memory and frame percentiles. No FPS gain
  is claimed by the CPU meshing measurements.

Use complete closed-directory copies for historical-save testing; never test
recovery on the only real save. See [PLATFORMS](PLATFORMS.md) for prerequisites.

## Windows acceptance considerations (pending)

Windows 10/11 x86-64 uses the same engine-neutral integer occupancy/coordinates,
embedded shader and assets, and the existing Bevy/wgpu backend selection. No
Vulkan-only requirement, path change or native dependency was introduced.
Compile/test through the existing Windows CI and repeat the visual/edit/streaming
checks on a supported native GPU; CI itself is not graphical acceptance.

```powershell
$env:RUSTCRAFT_DATA_DIR = Join-Path $env:TEMP ("rustcraft-slice17-" + [guid]::NewGuid())
$env:RUSTCRAFT_WORLD_SEED = "42"
cargo run --locked -p rustcraft
# Retain RUSTCRAFT_DATA_DIR to check save/restart.
```

Native Void GPU, Wayland, Windows rendering/input/DPI and long-session VRAM
endurance remain unverified here. The algorithm intentionally retains flat
skylight discontinuities and greedy T-junctions from Slice 16; it does not add
smooth skylight, directional shadows, emitted light or a day/night cycle.
