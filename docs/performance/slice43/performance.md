# Performance and resource measurements

All executables were built before measurement. The final locked correctness suite and render/reference workloads finished first; no builds, tests, rendering or compression ran alongside these measurements. Both execution orders are retained. These are cloud CPU/headless Bevy measurements, not native RX9060 XT frame rates or VRAM.

## Core protocol

`benchmark_terrain_geography.py` copies the current core into an isolated workspace and adds timers/allocation counters only there. One verified release executable selects frozen v16 or new v17; historical compatibility fixtures separately pin v16 behavior. Thirteen fixed seed/coordinate scenes use4 warmups and32 retained samples per arm/order, with orders16→17 and17→16. Source/instrumented-source/executable/harness fingerprints are retained.

The raw phase generates all16 chunks of one full-height column. `raw_cpu_us` is thread CPU time; `raw_us` is elapsed time. The20² surface-profile timer is a bounded stage, not every point height query used by caves/water. A separate prepared/cached-source worker phase includes lighting/meshing; it is not a cold end-to-end streaming latency. Allocation high-water counters track requested heap bytes, excluding allocator bookkeeping, stacks and GPU. All raw samples/outliers remain.

| Metric | Median of per-scene median changes, forward | Reverse | Largest adverse per-scene median change (forward / reverse) |
| --- | --- | --- | --- |
| 20² terrain profiles | +3.07% | -5.49% | +43.29% (beach) / +58.00% (beach) |
| Full-column CPU | -25.05% | -22.71% | +22.80% (beach) / +23.38% (beach) |
| Full-column elapsed | -25.06% | -22.70% | +22.80% (beach) / +23.39% (beach) |
| Prepared-source worker/light/mesh | -21.79% | -12.75% | +29.48% (beach) / +37.37% (beach) |
| Additional raw-generation requested heap | -9.11% | -9.11% | +0.00% (peaks) / +0.00% (peaks) |
| Worker requested heap | -21.05% | -21.05% | +14.64% (beach) / +14.64% (beach) |
| Mesh payload bytes | -21.87% | -21.87% | +14.17% (beach) / +14.17% (beach) |
| Vertices | -19.09% | -19.09% | +12.51% (beach) / +12.51% (beach) |

A median of scene ratios is not a pooled whole-world speedup. Terrain/ecology/cave cover intentionally change at the same coordinates. Less short steep relief reduces work in many scenes; coastal/forested cases can cost more. Quality is the purpose of the new composition. The profile-stage coastal regression is retained rather than hidden behind faster whole-column cases.

| Fixed scene / seed / chunk X,Z | Full-column median ms, v16→v17 forward | Reverse | Raw heap bytes v16→v17 | Mesh bytes v16→v17 |
| --- | --- | --- | --- | --- |
| plains / 0 / (-220,-252) | 5.325→3.693 | 5.255→3.993 | 74463→64109 | 587264→235848 |
| forest / 0 / (128,-252) | 6.855→4.712 | 6.085→4.704 | 75551→73378 | 531960→278504 |
| birch / 0 / (-120,-252) | 6.271→5.104 | 5.276→4.775 | 68706→62445 | 190464→50592 |
| taiga / 0 / (100,-252) | 4.138→2.729 | 4.123→3.066 | 65261→58964 | 118544→94232 |
| meadow / 0 / (208,-116) | 4.365→3.488 | 4.413→3.698 | 68614→62445 | 294872→230384 |
| grove / 2026 / (220,108) | 6.715→3.426 | 6.841→3.303 | 92210→72388 | 666376→363312 |
| grove-dense / 18446744073709551615 / (-101,-252) | 4.501→4.832 | 4.358→4.956 | 66950→66438 | 106144→99200 |
| snow / 0 / (-60,156) | 8.951→4.596 | 8.788→4.771 | 115253→80262 | 938432→797568 |
| peaks / 42 / (48,-56) | 4.435→4.445 | 4.239→4.318 | 72582→72582 | 335048→323888 |
| desert / 0 / (-168,-200) | 3.692→4.054 | 3.963→3.907 | 67654→67654 | 81840→82336 |
| beach / 42 / (208,-204) | 4.158→5.106 | 4.077→5.030 | 118500→118420 | 555016→633640 |
| river / 18446744073709551615 / (-252,132) | 5.485→4.111 | 5.759→4.322 | 80568→70406 | 211296→87048 |
| distant / 2026 / (-1464,1272) | 5.327→2.732 | 5.448→2.691 | 75423→58964 | 457808→71168 |

## Radius2–4 streaming and actual CPU publication

The established streaming harness exports exact merged baseline1f60f01 and current sources, injects identical scratch-only probes, and checks executable/runtime source fingerprints. It executes supported radii2/3/4 with40 retained scripted frames per scene, one repeat per order, plus cancellation/session lifecycle tests and real Bevy CPU mesh/visibility publication. Labels are inherited fixed cave/biome locations; they do not guarantee the same landscape category in v17. No worker, cache-capacity, publication or gameplay-radius policy changes were made.

The240 Hz deadlines are a headless workload schedule, not rendered240 FPS. Request-ready, request-to-CPU-visible and publication/update durations measure actual CPU events. RSS is process-wide; worker-window requested-heap peaks can include concurrent main-thread allocations. GPU upload/presentation/compositor costs are absent. Short trace tails are noisy; do not extrapolate native Radeon frame stability from them.

| Metric, matched scene/radius P99 ratios | Forward median change | Reverse median change | Worst adverse forward | Worst adverse reverse |
| --- | --- | --- | --- | --- |
| Request ready | -6.13% | -6.86% | +18.08%: stationary r2, 214000.3→252685.2 | +16.56%: coast r2, 263294.9→306896.4 |
| Request to CPU visible | -6.13% | -6.86% | +18.08%: stationary r2, 214013.4→252701.6 | +16.56%: coast r2, 263313.9→306912.3 |
| CPU publication/install | +0.92% | -2.63% | +950.68%: birch r4, 103.3→1085.3 | +731.78%: forest r2, 66.3→551.5 |
| Streaming update | +5.93% | -3.37% | +190.56%: forest r3, 277.1→805.1 | +184.67%: forest r4, 374.8→1067.1 |
| Headless ECS update | -21.89% | -8.61% | +385.61%: meadow r3, 305.2→1482.0 | +677.92%: grove r3, 294.9→2294.3 |
| Worker-window requested heap | -9.47% | -9.69% | +16.61%: desert r2, 1236084.0→1441348.0 | +16.62%: desert r2, 1236164.0→1441572.0 |
| Worker cached voxel bytes | -9.18% | -9.18% | +25.83%: stationary r3, 1110016.0→1396736.0 | +25.83%: stationary r3, 1110016.0→1396736.0 |

Time values above are microseconds; memory rows are bytes. In particular, large relative publication regressions can start from tens of microseconds. They are real retained observations, not proof of a general multi-fold slowdown. All maxima, including unfavorable frame/update outliers, are preserved below and in the raw data.

| Observed maximum over these workloads | v16 forward / reverse | v17 forward / reverse |
| --- | --- | --- |
| Worker cached columns | 121 / 121 | 121 / 121 |
| Worker cached voxel bytes | 4136960 / 4136960 | 2560000 / 2560000 |
| Additional worker-window requested heap bytes | 2906244 / 2904956 | 2005004 / 2001340 |
| Process high-water RSS KiB | 115304 / 115236 | 80260 / 79264 |
| Resident triangles | 404104 / 404104 | 233234 / 233234 |
| Resident mesh payload bytes | 50108896 / 50108896 | 28921016 / 28921016 |
| CPU publication maximum µs | 2938 / 4746 | 5673 / 3630 |
| Headless ECS update maximum µs | 26003 / 25825 | 20564 / 32592 |

No worker failures occur. Per-arm final states are identical across execution orders. Cache-column maxima remain121. Some forest/cavern-labeled cases gain triangles/payload even though the largest overall scene becomes smaller; a lower global maximum is not a per-scene improvement guarantee. Session drain, cancellation, saved edits and radius transitions run through the normal policies.

## Reproduction and practical limits

```sh
python tools/benchmark_terrain_geography.py --output work/core-perf --samples 32 --warmups 4
# Streaming builds first; retain each order in a separate output copy before the next run:
python tools/benchmark_streaming.py --baseline 1f60f01 --labels v16 v17 --caves-v3 --baseline-generator 16 --current-generator 17 --distance-expansion --frames 40 --repeats 1 --output work/stream --build-only
python tools/benchmark_streaming.py --baseline 1f60f01 --labels v16 v17 --caves-v3 --baseline-generator 16 --current-generator 17 --distance-expansion --frames 40 --repeats 1 --output work/stream --reuse
# Copy the results, then repeat with --reuse --reverse. Run without other workloads.
```

The source/executable provenance, all retained samples, percentiles, resource/state traces and both method files are under [raw/performance/](raw/performance/). Host/capture details are in [environment.json](raw/environment.json). Timing variation/order effects and changed scene geometry remain material limits. Native Gentoo/Void X11/Wayland, Windows GUI/input and the owner's i3-13100/RX9060 XT FPS/VRAM acceptance remain pending. No native GPU speedup or zero-frame-spike guarantee is claimed.
