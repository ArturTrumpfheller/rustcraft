# Quantitative geography

All thresholds below are transparent geometric diagnostics, not exact Minecraft landscape/biome frequencies. No seed-specific quotas were used.

## Domain, scales and categories

Development seeds0/42/2026/u64::MAX use spacing16 over central8192² and signed distant2048² windows at(−24000,20000) and(28000,−30000). Each arm samples1,179,648 points over301,989,888 blocks². Held-out seeds123/777/4099/900001 use spacing32 in the same windows (294,912 points/arm). Native reference geometry is measured at Y[-64,320), sea63; plots subtract61. Separate normalized statistics scale sea-relative height by190/257, the ratio of exclusive-ceiling-minus-sea spans. This unit conversion does not align the two foundations or model their water systems.

Contextual land is height>sea2 with a256-block border excluded. Gentle:128-span relief≤12 and256-span≤24. Rugged:128-span≥40, or256-span≥64 together with64-span≥20. Other contextual land is hills. Unlike the historical Slice38 analyzer, this shared reference comparison does not add a one-block slope gate to the gentle class, because Horizon does not export such slopes on this grid. RustCraft one-block slope distributions are retained separately. Centered relief includes both endpoints. Four-neighbor components measure sampled connected area, not traversable polygons.

All arms use grid-derived secant slopes and height differences at16/32/64/128/256/512 blocks where resolution permits. High-pass residuals compare a point with four opposing samples at each scale, separating local roughness from macro height variance. Valleys use opposing shoulders within128 blocks on fixed cardinal transects every fourth grid row; depth≥12 and width at half-depth. Narrow/deep means depth≥24 and width≤48. Spacing32 cannot resolve a≤48-wide minimum; those zero counts are not evidence that such valleys are absent. Transects include ocean beds; these are surface-geometry minima, not a count of dry/buildable valleys. Off-axis valleys and features below the sample spacing can be missed.

## Development seeds: matched16-grid surfaces

| Seed | Gentle v16→v17 | Hills v16→v17 | Rugged v16→v17 | Narrow/deep minima v16→v17 | Median valley width v16→v17 |
| --- | --- | --- | --- | --- | --- |
| 0 | 24.21%→40.53% | 39.21%→39.15% | 36.58%→20.32% | 129→22 | 112→128 |
| 42 | 24.11%→36.41% | 37.47%→41.33% | 38.42%→22.25% | 156→23 | 112→128 |
| 2026 | 22.14%→38.51% | 38.78%→37.57% | 39.08%→23.92% | 153→35 | 112→128 |
| 18446744073709551615 | 23.22%→39.41% | 39.98%→40.26% | 36.80%→20.33% | 165→30 | 112→128 |

## Independent seeds: same32-grid criteria

| Seed | Gentle v16→v17 | Hills v16→v17 | Rugged v16→v17 |
| --- | --- | --- | --- |
| 123 | 25.20%→35.65% | 41.68%→41.68% | 33.12%→22.68% |
| 777 | 24.99%→43.30% | 41.86%→36.43% | 33.15%→20.27% |
| 4099 | 24.16%→39.86% | 37.60%→41.73% | 38.24%→18.41% |
| 900001 | 21.76%→39.98% | 40.44%→40.16% | 37.80%→19.86% |

## Height and local slope

These are full central-grid samples including water-bed/ocean elevations. Surface Y is the highest uncaved terrain-density solid before Hydrology v1. Heights are actual integer Y; one-block slopes are maximum cardinal height steps.

| Version / seed | Height median / P95 / P99 / max | One-block slope P95 / P99 / max |
| --- | --- | --- |
| v16 / 0 | 18.0 / 64.0 / 112.0 / 161.0 | 1.0 / 3.0 / 15.0 |
| v16 / 42 | 20.0 / 69.0 / 118.0 / 160.0 | 2.0 / 3.0 / 13.0 |
| v16 / 2026 | 19.0 / 67.0 / 111.0 / 159.0 | 2.0 / 3.0 / 18.0 |
| v16 / 18446744073709551615 | 18.0 / 65.0 / 109.0 / 159.0 | 1.0 / 3.0 / 14.0 |
| v17 / 0 | 13.0 / 42.0 / 93.0 / 157.0 | 1.0 / 2.0 / 12.0 |
| v17 / 42 | 14.0 / 46.0 / 96.0 / 156.0 | 1.0 / 2.0 / 12.0 |
| v17 / 2026 | 13.0 / 51.0 / 110.0 / 158.0 | 1.0 / 2.0 / 13.0 |
| v17 / 18446744073709551615 | 14.0 / 46.0 / 100.0 / 160.0 | 1.0 / 2.0 / 11.0 |

## Multi-scale continuity (seed42, all other seeds/regions in JSON)

| Distance/span | Median height difference v16→v17 | P95 difference v16→v17 | Median contextual relief v16→v17 | Median high-pass residual v16→v17 |
| --- | --- | --- | --- | --- |
| 16 | 1.00→1.00 | 15.00→8.00 | 3.00→2.00 | 0.50→0.25 |
| 32 | 2.00→1.00 | 25.00→14.00 | 6.00→3.00 | 1.00→0.50 |
| 64 | 4.00→3.00 | 38.00→24.00 | 11.00→7.00 | 2.00→1.50 |
| 128 | 6.00→5.00 | 51.00→34.00 | 23.00→14.00 | 4.25→3.25 |
| 256 | 10.00→8.00 | 63.00→45.00 | 55.00→31.00 | 8.00→5.75 |
| 512 | 17.00→12.00 | 74.00→56.00 | 121.00→70.00 | 12.50→8.75 |

## Regional coherence and unfavorable regions

- v16: largest rugged fraction across retained windows is52.79% at seed18446744073709551615, region2. The central result is not a universal regional balance.

- v17: largest rugged fraction across retained windows is43.76% at seed18446744073709551615, region1. The central result is not a universal regional balance.

- v17-validation: largest rugged fraction across retained windows is41.68% at seed4099, region1. The central result is not a universal regional balance.

| Version / seed42 central | Largest gentle component (blocks²) | Largest rugged component (blocks²) | High components≥8 samples | Enclosed low area at height≥62 contour (blocks²) |
| --- | --- | --- | --- | --- |
| v16 | 2279424 | 3036672 | 20 retained (largest20) | 98560 |
| v17 | 4694016 | 1815040 | 20 retained (largest20) | 27392 |

High-elevation component bounding-box axis ratios, perimeter compactness and enclosed low pockets are screening diagnostics at the declared grid spacing/height contour. They do not prove the absence of all circles, domes, rings or repeated silhouettes. Open contours, sampling, erosion boundaries and legitimate basins affect these metrics. Inspect maps, fixed cross-sections and genuine panoramas together.

## Executed Horizon: actual final graph and declared terrain control

Each row below uses the normalized above-sea span for class/height-difference comparisons. Normal final-density output includes cave branches and can expose deep floor cells in surface pits; it excludes later carvers/aquifers/materials. The terrain control removes cave graph branches while retaining the rest of the executed graph. Neither is a locally rerun Minecraft world. The coarse surface-search proxy is preserved as separate raw data and never substituted for final height/slope measurements.

| Reference / seed | Gentle / hills / rugged (normalized contextual land) | Native sea-offset height P50 / P95 / P99 | Normalized32-step difference P50 / P95 |
| --- | --- | --- | --- |
| horizon-final / 0 | 1.02% / 61.57% / 37.42% | 0.0 / 39.0 / 64.0 | 2.22 / 14.79 |
| horizon-final / 42 | 0.49% / 51.51% / 48.00% | 1.0 / 48.0 / 66.0 | 2.96 / 17.00 |
| horizon-final / 2026 | 0.83% / 54.38% / 44.79% | 1.0 / 42.0 / 72.0 | 2.22 / 16.26 |
| horizon-final / 18446744073709551615 | 0.93% / 54.56% / 44.51% | 2.0 / 48.0 / 72.0 | 2.96 / 17.00 |
| horizon-terrain-control / 0 | 5.24% / 70.54% / 24.22% | 1.0 / 39.0 / 64.0 | 2.22 / 12.57 |
| horizon-terrain-control / 42 | 3.22% / 61.26% / 35.52% | 1.0 / 48.0 / 67.0 | 2.22 / 14.79 |
| horizon-terrain-control / 2026 | 4.10% / 63.55% / 32.35% | 1.0 / 42.0 / 73.0 | 2.22 / 14.05 |
| horizon-terrain-control / 18446744073709551615 | 4.38% / 64.56% / 31.06% | 2.0 / 48.0 / 72.0 | 2.22 / 14.79 |

The adapted RustCraft terrain is substantially quieter than these executed reference grids under this strict gentle definition. Horizon also has a much larger ocean fraction in these windows. Matching these arbitrary category percentages is therefore not a parity objective. The technical influence is nested spline composition, broad structural fields, terrain-dependent factor/jaggedness and cached/interpolated density; RustCraft deliberately retains its smaller bounds, common lowlands and independent geography. The observed difference remains a limitation, not a claim of statistical Minecraft parity.

## Reproduction and evidence

```sh
cargo run --release --locked -p voxel_core --example terrain_balance_survey -- --generator=16 --step=16 > work/v16.csv
cargo run --release --locked -p voxel_core --example terrain_balance_survey -- --generator=17 --step=16 > work/v17.csv
python tools/analyze_terrain_geography.py work/v16.csv --label v16 --step 16 --output work/maps
python tools/analyze_terrain_geography.py work/v17.csv --label v17 --step 16 --output work/maps
# Repeat with --validation --step=32, and with --final --step=64 for ecology.
# Pinned Horizon runner preparation/commands: tools/horizon_reference/README.md.
```

Raw scalar grids are compressed under[raw/](raw/); all per-seed, per-region native/normalized distributions are in[diagnostics/](diagnostics/). Maps show measured geometry; they are not gameplay screenshots. CLI source and input/source/executable fingerprints make the sampling and thresholds reviewable.
