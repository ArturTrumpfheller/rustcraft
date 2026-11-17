# Slice43 validation

All five required local checks passed on the final implementation, using the
locked workspace and Rust 1.99.1. The workspace suite ran serially without
concurrent rendering, reference generation or benchmarks. No test timeout or
production streaming budget was increased.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --locked --workspace -- --test-threads=1` | 618 passed, 7 existing ignored, 0 failed |
| `cargo test --release --locked -p voxel_core` | 396 passed, 1 existing ignored, 0 failed |
| `cargo build --release --locked --workspace` | Pass |

[Command exit codes and durations](raw/validation/results.json), with complete
compressed stdout/stderr logs in [raw/validation/](raw/validation/). The ignored
tests remain the existing opt-in expensive surveys and profiling checks; they
were not newly disabled to obtain a passing suite. Linux and Windows CI retain
the locked workspace checks and independent numerical oracles. Remote CI results
must be checked on the actual PR head; these local results do not assert a
Windows execution.

## Numerical and compatibility checks

- Historical generator1–16 fixtures are unchanged. Historical full-column,
  terrain, biome, cave, save identity and lifecycle checks pass alongside v17.
- An independent Python integer implementation verifies 102 new v17
  continentalness/erosion/weirdness, nested-curve offset, factor, jaggedness,
  density-envelope and exact surface-bit fixtures. A separate file adds 24 new
  full-height column/biome CRCs; no historical expectations were regenerated.
- The independent v10, v11, v13 and climate numerical checks also pass. Fresh
  executed Horizon noise matches all 720 archived samples byte-for-byte; the
  independent numerical noise evaluator agrees within its stated `2e-6`
  tolerance.
- The existing Python discovery suite passes all 14 tests. The original atlas
  generation check passes; no texture asset, dependency, lockfile, block ID or
  save format changes.

Reproduce the lightweight checks from the repository root:

```bash
python -m unittest discover -s tools -p 'test_*.py'
python tools/generate_textures.py --check
python tools/terrain_v3_reference.py --check
python tools/terrain_v4_reference.py --check
python tools/terrain_v5_reference.py --check
python tools/terrain_v6_reference.py --check
python tools/biome_v2_reference.py --check
python tools/cave_v3_reference.py --check
```

## Terrain and integration coverage

The added v17 tests cover signed/distant coordinates, exact shared density
profiles, point/chunk/halo parity, opposite generation orders, cross-thread
determinism, foundation and vertical limits, protected spawn, and occasional
overhangs. The cave regression explicitly initializes the same terrain-height
cache through the point path first and through the bulk lattice first. Both
paths must use v17 heights; this catches the stale-v16 lazy-initialization bug
found during integration.

Actual generated geometry is audited at sites in all fifteen biomes, including
tree root support, surface materials and water containment. Extended cave and
lifecycle matrices exercise walking/collision, mining/placement, torches,
authoritative saved Air and construction edits, eviction, reopen, journal
recovery, streaming cancellation and session/world switching. Generator,
world-bounds, region and persistence validation accept17 and reject18+.

Two persistence tests needed v17-specific vegetation sites because the new
geography changes which old coordinates contain a plant or forest. Historical
versions retain their original test locations. The v17 forest fixture still
crosses a regional boundary. The assertions were retained. One earlier heavy
session test timed out while builds/rendering competed for the two-CPU quota;
its unchanged quiet targeted run and the subsequent complete serial suite pass.

The 120 paired cave-exposure windows, cave-disabled geological controls and
sixteen independent deep windows are described in [integration.md](integration.md).
All 120 historical v16 normal volumes and all 120 historical controls reproduce
the archived hashes. The baseline metrics reuse is explicit; v17 metrics were
analyzed afresh. These geometry measurements supplement the executable tests.

## Executed external reference

The pinned Horizon density suite passes 49 focused upstream tests. The final
adapter executes 216 router probes including actual final density. After the
adapter's last changes, fresh spacing128 runs match 18,432 frozen surface heights
each for normal final density, the declared cave-disabled control and independent
validation seeds. Eight complete cave-stage exports reproduce the archived
reference volume hashes. See
[final reference verification](raw/reference-final-verification.json).

Separately, 122,708 integer-Y scalar surface searches match the volume API's
highest-positive-density result exactly. This cross-check validates those two
executed reference paths, not Minecraft JVM parity. The graph-only terrain
reference, cave-disabled ablation and complete cave-stage evidence remain
distinct. External source/registry data and executables are excluded from the PR;
source/input hashes, commands and compressed numerical output are retained.

## Artifact verification and remaining acceptance

All 34 real Bevy images have verified hashes and matched camera metadata in
[captures/captures.json](captures/captures.json). The compressed raw-data manifest
records uncompressed sizes and SHA-256 hashes. The final artifact manifest covers
the evidence files, and provenance records source and reference fingerprints.
The screenshots use Mesa llvmpipe Vulkan at 1280×720 through Xvfb. Both execution
orders of the CPU and radius2–4 streaming measurements are reported, including
regressions and adverse tails, in [performance.md](performance.md).

Native Gentoo/Void X11 and Wayland, Windows 10/11 GUI, physical RX9060 XT frame
times/FPS/VRAM and owner visual/gameplay acceptance remain pending. Headless
streaming timings and software-rendered images cannot substitute for these.
No automatic merge, release, save migration or later hydrology/cave slice is
performed.
