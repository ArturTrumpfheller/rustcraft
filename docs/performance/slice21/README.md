# Slice21 performance comparison

Accepted baseline: `ab36403bd5742438499c645d1bdb6e19cd4a1317` (Slice20).
Linux X11/Xvfb, 960×640, Mesa25.0.7 llvmpipe LLVM19.1.7 Vulkan CPU, Rust1.99.1.
No physical Radeon/Windows GPU result is implied. Builds, tests and captures ran
outside the paired renderer measurements. Renderer: three processes/version/radius,
alternating version order; each has at least5 seconds/120 warmup frames and300
measured frames. Quantiles below are medians of the three per-process quantiles,
not pooled quantiles. Shared cloud CPU scheduling makes tails noisy.

## Renderer

Same flat terrain, camera, radius, atlas and three torches in both versions.
The baseline uses its original daylight shaders and an inert clock adapter;
Slice21 starts at midnight and advances at normal speed. Geometry remains fixed.
This comparison includes the new procedural sky shader and its uniform updates.

| Radius | p50 ms before → after | p95 ms before → after | p99 ms before → after | p50 change |
| --- | --- | --- | --- | --- |
| 1 | 73.25 → 83.40 | 139.46 → 222.44 | 160.58 → 441.39 | +13.9% |
| 2 | 72.65 → 83.42 | 101.49 → 113.59 | 189.91 → 198.04 | +14.8% |
| 3 | 76.06 → 88.87 | 196.17 → 145.88 | 246.98 → 274.59 | +16.8% |

The software renderer has a measurable frame cost; these data do not establish
native playability or a native-GPU budget. No repeated growth over time or
time-driven terrain jobs were observed. The fixture control-system medians are
roughly6–7µs before and11–12µs after, including ECS resource bookkeeping.
A separate CPU-only retained-asset helper benchmark measured500–504ns per clock
tick, atmosphere calculation and two uniform writes in a debug build;
see [raw helper results](clock-uniform.txt). It excludes renderer uploads and
does not represent the whole frame cost.

| Radius | Terrain+sky mesh assets (both) | Quads before → after | Images (both) | Terrain / sky materials (both) |
| --- | --- | --- | --- | --- |
| 1 | 23 | 77 → 652 | 6 | 1 / 1 |
| 2 | 55 | 109 → 684 | 6 | 1 / 1 |
| 3 | 103 | 157 → 732 | 6 | 1 / 1 |

Both fixtures retain Bevy’s default images plus the one original atlas. The
additional575 flat-fixture quads preserve previously hidden torch gradients:
142,600 extra vertex/index bytes, allocated only when meshing. They do not
increase as the clock advances. Main-client tests across radii1–3 cover more
than three cycles with zero added jobs/remeshes, unchanged handles/cache bytes
and unchanged asset counts, then travel/eviction without changing time.

## Terrain jobs (release CPU benchmark)

Generator-v3 seed2026, full-height targets at radii1–3 with complete light halos.
The three-torch variant uses the same emitter cells on actual generated solid
surfaces. Each process warms two jobs, then takes a median of13 jobs; values are
medians of three processes. Natural/no-torch results and all individual runs
are retained in the CSV files.

| Radius / three torches | Light µs before → after | Mesh µs before → after | Quads before → after | Mesh bytes before → after |
| --- | --- | --- | --- | --- |
| 1 | 2349.58 → 1587.65 | 3039.66 → 2400.83 | 5377 → 5692 | 1333496 → 1411616 |
| 2 | 4746.47 → 3482.90 | 7635.93 → 6906.85 | 13734 → 14049 | 3406032 → 3484152 |
| 3 | 7745.74 → 6573.72 | 14285.49 → 15250.60 | 26431 → 26746 | 6554888 → 6633008 |

The lighting algorithm is unchanged; lower timings are cloud variation, not a
lighting optimization claim. Meshing now retains two channels and can do more
work. The generated fixture adds315 quads /78,120 bytes with torches at each
radius; natural terrain has exactly the same quads/payload as Slice20. The
56-byte vertex and248-byte per-quad vertex/index payload remain unchanged.
All of these costs occur on existing terrain jobs, rather than on clock ticks.

## Memory and bounds

| Radius | Light volume bytes with torches (both) | Exact resident light cache bytes (both) | Median sampled peak RSS MiB before → after |
| --- | --- | --- | --- |
| 1 | 584064 | 157464 | 332.9 → 344.2 |
| 2 | 1161600 | 437400 | 344.4 → 334.3 |
| 3 | 1935744 | 857304 | 333.6 → 326.5 |

RSS is sampled every10ms through Linux /proc and includes Bevy, software Vulkan,
pipeline/driver allocations and startup. Variability overlaps and it is not a
precise attribution of feature memory. Logical additions are two uniforms growing
48→80 bytes (+64 bytes total), a16-byte padded clock in each world/snapshot and
saved-time tracking, plus16 bytes in the player file/journal. The greedy mask
retains one extra byte per key (24KiB across6×16×256 cells with this toolchain),
bounded scratch on the existing worker. No second sky mesh, texture or material
is created per tick. Persistent mesh growth around emitters is reported above.

## Reproduce

From the repository root on a working Linux desktop, after native dependencies:

```sh
python3 tools/benchmark_day_night.py --output work/day-night-measurements --samples 3
cargo test --locked -p rustcraft --bin rustcraft clock_uniform_cpu_cost -- --ignored --nocapture
cargo test --locked -p rustcraft --bin rustcraft clock_progression_at_all_radii -- --nocapture
```

The harness reconstructs baseline core/client sources offline from Git, builds
the same fixture against each version and retains raw data. It compiles the
baseline renderer against the cached Bevy rlib and an isolated baseline core,
without replacing current Cargo artifacts. Linux /proc is used only by this
development script; the game has no platform-specific timing/profiling dependency.
See [DAY_NIGHT](../../DAY_NIGHT.md) for screenshots, equations, persistence and
the outstanding Void X11/Wayland and Windows physical-GPU acceptance checklist.
