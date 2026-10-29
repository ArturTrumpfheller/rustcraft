# Slice 26 measurements and validation

This is cloud CPU/software-rendering evidence, not a measurement on the owner's
Intel i3-13100 / RX9060 XT /32 GB / Void glibc /1920×1080 hardware. Native Void
X11/Wayland and Windows gameplay/driver acceptance remains open.

## Method and reproduction

Baseline: merged Slice25 commit `99137e3df86e71d6499a8d94f8c01c639249005b`.
Current checkout measures both unchanged48-block generator5 worlds and new
256-block generator6 worlds. `vertical_bench` uses the same release harness and
allocation meter on both revisions; the baseline adapter changes only three
measurement expressions for the older storage API. No baseline production code
is modified. Raw output is retained in [raw](raw).

```fish
git worktree add --detach work/slice25-baseline 99137e3df86e71d6499a8d94f8c01c639249005b
python3 tools/benchmark_vertical.py --baseline work/slice25-baseline --output work/vertical-results
```

The script refuses to overwrite existing results or an unrelated baseline
harness. Default is three complete repetitions. Each scene discards three warmup
samples and records30 jobs, separate thread CPU time on Linux, lighting costs,
installation time and requested-heap high-water. The64-step exploration probe
prunes its cache after every move. The save probe performs20 coordinated saves,
reopens and verifies high/deep block states plus inventory. Percentiles use the
existing nearest-rank-style floor index; with30 samples P99 is not a robust tail
estimate. Table values are medians across the three runs' reported statistics.

The resident fixture covers3×3 horizontal columns, with one center column mesh
job and its generated halo. `voxel_bytes` is allocated block-vector payload;
ready descriptors, maps, allocator metadata, GPU buffers and thread stacks are
separate. `peak_job_heap` is additional requested heap above the pre-job live
baseline, including first cold/cache construction and transient source/light/mesh
payloads. It is not OS RSS or VRAM. Output cache occupancy is a conservative
retained-column observation, not a claim of worst-case occupancy.

Scenes: normal generated surface; a separate completely empty upper target;
a4×4 mined shaft; a hollow tower; a narrow full-height falling water shaft;
and a checkerboard-edited center column. Only the surface/empty targets are
matched content comparisons. The v6 mine, tower, water and heavy edit fixtures
use the additional height and contain more edited volume; they must not be
interpreted as same-workload regressions against48-block fixtures.

## Matched surface results

Three-run medians,3×3 resident horizontal columns and one center-column job.
All times below are microseconds. Payloads are bytes.

| Metric | Slice25 baseline / gen5 | Slice26 / gen5 | Slice26 / gen6 |
| --- | ---: | ---: | ---: |
| Ready descriptors | 27 | 27 | 144 |
| Nonempty resident chunks | 20 | 20 | 47 |
| Allocated voxels | 110,592 | 57,344 | 57,344 |
| Worker cache columns | 9 | 9 | 9 |
| Cache voxel bytes | 110,592 | 57,344 | 57,344 |
| Mesh bytes | 24,056 | 24,056 | 24,056 |
| Dense light cells | 101,568 | 101,568 | 133,308 |
| Lighting µs | 353.40 | 420.64 | 679.34 |
| Job P50 µs | 527.52 | 625.75 | 1,019.14 |
| Job P95 µs | 658.94 | 781.16 | 1,194.85 |
| Job P99 µs | 690.51 | 786.56 | 1,244.97 |
| Worker CPU P50 µs | 527.65 | 625.85 | 1,019.58 |
| Install P50 µs | 1.42 | 1.27 | 8.99 |
| Temporary peak requested heap | 446,384 | 341,272 | 421,024 |
| Spawn prepare µs | 361.32 | 384.81 | 386.71 |

The surface payload is unchanged at24,056 mesh bytes. Compact
voxel vectors use57,344 bytes versus110,592
in the original baseline; extended v6 and compact historical v5 use the same
voxel bytes. Dense light work is1.31× the
original cell count, not256/48×. Ordinary v6 jobs cost1.93×
the baseline P50 (1.019 ms absolute); this is an explicit
CPU regression, with lower voxel/temporary memory and no mesh growth. Native
acceptance remains necessary.

## Content-scaled extended scenes

These are **v6-only** results. Tall/deep fixtures contain more volume than their
historical counterparts; complete baseline/current raw rows remain available.

| v6 scene | Voxel bytes | Mesh bytes | Job P50/P95/P99 µs | Light µs | Install µs | Peak heap bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| deep_mined | 73,728 | 99,944 | 1,827.80 / 2,102.93 / 2,169.37 | 1,231.13 | 10.69 | 875,452 |
| tower | 102,400 | 64,480 | 3,081.62 / 3,480.39 / 3,507.59 | 2,264.34 | 12.54 | 1,079,720 |
| vertical_water | 118,784 | 89,216 | 4,008.21 / 4,379.55 / 4,523.21 | 2,729.96 | 843.13 | 1,299,240 |
| heavy_column | 118,784 | 48,164,824 | 36,068.36 / 38,402.78 / 38,498.62 | 2,543.05 | 977.13 | 65,850,280 |

The full-height narrow flow settled with queue0 in382.88 µs of fluid ticks.

## Empty sky, exploration and persistence

Upper probe is a single resident snapshot remesh per run, not a30-job percentile.

| Probe | Slice25 / gen5 | Slice26 / gen5 | Slice26 / gen6 |
| --- | ---: | ---: | ---: |
| Empty upper probe µs | 362.28 | 480.26 | 8.80 |
| Upper dense light cells | 101,568 | 101,568 | 0 |
| Travel P50/P95/P99 µs | 1,695.14 / 2,673.58 / 3,527.25 | 1,868.50 / 3,030.31 / 3,557.94 | 2,177.87 / 3,342.60 / 4,041.09 |
| Peak retained travel cache columns | 15 | 15 | 15 |
| Final travel cache voxel bytes | 184,320 | 143,360 | 143,360 |
| Save P50/P95/P99 µs | 65.18 / 109.13 / 109.13 | 57.85 / 73.87 / 73.87 | 63.58 / 78.32 / 78.32 |
| Reload P50/P95/P99 µs | 444.44 / 621.25 / 621.25 | 406.77 / 485.56 / 485.56 | 459.50 / 542.27 / 542.27 |

V6's empty upper probe has zero mesh geometry, zero retained light arrays and
zero dense light cells. Travel prunes on every step and stays within the existing
81-column policy. Save/reopen validates both extreme edits and inventory. The
core harness runs jobs serially and does not model a Bevy queue; actual client
queue/cache/install observations follow. See [summary CSV](summary.csv) for all
reported scene statistics and [environment](environment.txt).


## Actual client and images

The release client was launched and reopened through the real title/create/catalog/play flow
with a new isolated data root and seed42. Catalog and F3 report generator6.
Radius1 loaded144 ready descriptors,47 nonempty chunks,57,344 voxel bytes and23
opaque meshes with507,408 terrain buffer bytes. Cache held24 halo columns /
143,360 voxel bytes;32 exact chunk-light arrays were retained. No dirty regions
or discarded jobs remained. Queue peak was24, with four cached regions.

The startup sample logged nine load/remesh jobs:52.535 ms worker total,
15.972 ms maximum job,103.471 µs maximum main-thread installation and421.554 µs
maximum streaming update. P50/P95/P99 worker observations were3.946/13.696/13.696
ms. These tiny startup samples ran under a heavily contended software renderer
and include effects absent from the isolated core probe.

Xvfb1280×720, llvmpipe LLVM19.1.7 / Mesa25.0.7 Vulkan software rendering: F3
showed about4 FPS /244 ms smoothed frame time in the first loaded run and
8 FPS /126 ms in the reopened screenshot. The initial10-frame
sample included shader/pipeline/startup stalls: frameP50/P95/P99 was159/3761/3761
ms, mean1.6 FPS. These observations **cannot establish native Radeon performance**.
The initial external Xvfb window-destruction command caused swapchain-Lost messages;
that tooling path was stopped after successful autosave. The subsequent reopen
completed normal Pause → Quit Game with a successful final save and clean process
exit. Native window-manager
close/input/compositor behavior remains a manual acceptance item.

Actual captures, using original RustCraft textures and production shaders:

- [New v6 world, real client and F3](../../images/slice26/client-v6-surface.png).
- [Tower above the old ceiling, gallery eye Y124](../../images/slice26/tower-y124.png).
- [Torch-lit sealed room, gallery eye Y-49.8](../../images/slice26/torches-y50-below-zero.png).

Tower/room captures are explicitly authored development fixtures in the existing
terrain gallery, not naturally generated features or proof of native gameplay.
Reproduce with the normal desktop environment:

```fish
cargo build --locked -p rustcraft --example terrain_gallery
./target/debug/examples/terrain_gallery 42 0 0 0 --generator=6 --vertical --capture=/tmp/slice26-tower.ppm
./target/debug/examples/terrain_gallery 42 0 0 0 --generator=6 --deep --capture=/tmp/slice26-deep.ppm
```

## Automated validation

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`:476 passed;4 pre-existing CPU measurement
  tests remain ignored. Client193 passed, core283 passed, no failed tests.
- `cargo build --release --locked -p rustcraft`
- `python3 tools/generate_textures.py --check`
- Gallery build, real Xvfb software captures, actual new-world creation/play.
- `python3 tools/benchmark_vertical.py`: matched release harness across revisions.

Linux/Windows GitHub CI uses the existing matrix; remote results are reported
in the PR. [Command/result summary](validation.txt).

New regression tests (historical golden constants remain unchanged):

- `vertical_tests::identities_define_exclusive_bounds_and_old_worlds_never_expand`
- `vertical_tests::v6_matches_frozen_v5_surface_and_point_chunk_sampling_in_any_order`
- `vertical_tests::v6_independent_column_goldens`
- `vertical_tests::ready_air_materializes_and_last_removal_stays_ready_across_eviction`
- `vertical_tests::sparse_sky_and_deep_slabs_do_not_multiply_surface_work_or_cache_bytes`
- `vertical_tests::deep_and_high_torches_occlusion_and_vertical_seams_relight_exactly`
- `vertical_tests::expanded_mesh_culls_vertical_faces_and_invalidates_neighbor_revisions`
- `vertical_tests::water_crosses_all_vertical_seams_stops_at_floor_and_recedes_after_removal`
- `vertical_tests::collision_jump_targeting_and_pending_barriers_work_at_extended_heights`
- `vertical_tests::sparse_light_matches_full_256_plane_reference_including_shadows_and_pending`
- `vertical_tests::deferred_vertical_flow_restarts_when_next_pending_slab_arrives`
- `vertical_tests::swimming_and_torch_washing_work_across_high_and_deep_chunk_boundaries`
- `persistence::regional::tests::v6_high_deep_edits_inventory_binding_and_every_journal_boundary_recover`
- `persistence::regional::tests::region_v1_validates_height_by_identity_and_retains_its_bounded_record_budget`
- `persistence::player::tests::extended_height_keeps_eight_maximum_regions_inside_existing_journal_budget`
- Client `streaming::tests::v6_upper_air_mesh_materialization_stale_rejection_eviction_and_reload`

## Remaining limits and native acceptance

A very tall edited structure/shaft may require a full256-plane light span. The
checkerboard stress fixture is deliberately expensive: much larger mesh output
and temporary heap are driven by actual edited content, not empty sky. A single
worker remains adequate for measured ordinary surface jobs; cooperative worker
cancellation, segmented solvers and dense-region redesign are future work if
native profiling justifies them.

Each region retains its196,608-record total overlay budget. Edits beyond that
fail before mutation; saving does not free that total budget. This preserves the
historical file and32 MiB journal limits. Materializing fluids in a previously
uncached upper chunk can require a full initial lighting job. The bounded fluid
queue/route caches allocate on demand; full-region backpressure can defer flow.

No native GPU, Windows GUI or Wayland play-test was available in this cloud.
No new caves, mountain retuning, natural water, structures or movement abilities
are included. [Version contracts and Void/Fish checklist](../../VERTICAL_WORLD.md).
