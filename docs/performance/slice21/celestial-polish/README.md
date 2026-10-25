# Sun/moon visual-polish comparison

Baseline: Slice21 commit `6d07e85`, before this follow-up. The before fixture
uses the baseline atmosphere Rust/WESL and production terrain/sky shaders with
the same updated diagnostic gallery camera controls as the after fixture.
`voxel_core`, Bevy and the original atlas are identical. No current Cargo
artifacts were replaced by the baseline build.

X11/Xvfb, Mesa25.0.7 llvmpipe LLVM19.1.7 Vulkan CPU. Debug builds,
1280×720, FOV 78°, radius2, `plains`, time18000 fixed, `--focus=moon`.
This deliberately keeps the visible full moon in view at a normal gameplay FOV.
Each of three processes/version warms at least 120 frames/5 seconds and records
120 frames; order alternates before/after and after/before. No builds, other
renderers or tests ran concurrently. Quantiles below are medians of the three
per-process quantiles. Cloud scheduling causes substantial tail variation.

| Version | p50 ms | p95 ms | p99 ms |
| --- | --- | --- | --- |
| before | 92.27 | 141.31 | 159.72 |
| after | 91.53 | 116.92 | 134.79 |

This comparison shows no measurable added median frame cost; the small lower
median is within cloud variation, not evidence of a performance optimization.
It does not replace a native RX 9060 XT retest of the new art. The owner reports
approximately 240 FPS with VSync disabled for the preceding Slice21 baseline.

Both versions retain 684 terrain quads /169,632 vertex+index bytes, 55 total
terrain+sky meshes, 6 images including Bevy defaults and the original atlas,
1 terrain material and 1 sky material. Fixed time gives zero material writes in
both runs. Clock progression, CPU allocations, the 80-byte atmosphere uniform,
terrain work and material/image counts are unchanged by the polish.

Only visible moon fragments perform the new packed 16×16 point lookup. The
art stores 64 bytes of row indices plus four constant palette colors in WESL;
it adds no sampled image, sampler, bind group, loop or new transcendental noise
function. Body UVs reuse the existing projected coordinates. The small horizon
fade is bounded arithmetic using the same body center/half-size.

Reproduce each version with the gallery command below, using the baseline
rendering modules at 6d07e85 for before. Build into separate target directories
if using separate worktrees; the camera/flags must be identical for both:

```sh
cargo run --locked -p rustcraft --example day_night_gallery -- plains 2 --time=18000 --focus=moon --size=1280x720 --frames=120
```

The focus/size flags are diagnostic gallery additions from this follow-up;
use the updated gallery adapter with the original baseline atmosphere/shaders
for before. Raw per-process output and `frames.csv` are retained alongside this
report. Physical-GPU visual acceptance of the polish remains open in PR#38.
