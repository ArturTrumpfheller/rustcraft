# Cave, biome and hydrology integration

V17 retains v16 cave-family functions, named noise inputs, carver paths, refined composition and exposure arithmetic. The only cave integration change is a single versioned terrain-height/slope cache initializer shared by point carvers and bulk density. A direct regression initializes those paths in opposite orders at seed42(−128,−128), then compares cached inputs and density samples. Full signed chunk/point/halo/ecology audits pass that case.

## Slice42 surface-exposure regression

Use the original primary80 and independent40 X/Z windows. Each version gets its own cave-disabled geological control, retaining actual terrain density, ecology and water. The existing physical-mouth/standing-visibility/penetration/supported-route analyzer is unchanged. Qualifying means a real two-block standing aperture or penetration≥3 into geological cover within the declared local search. Deep connection requires≥128 cavity cells reaching24 blocks of cover; supported exterior-to-deep access is a separate floor graph. Aperture patches touching the window boundary are censored, with the same8-block centroid margin and112² effective area. See[Slice42 definitions](../slice42/entrances/README.md).

All120 v16 normal volumes and120 geological controls reproduce the archived SHA256s exactly. Twenty-nine freshly reanalyzed baseline windows match the retained headline metrics. Thus the complete archived v16 metric set is reused with hash-verification provenance; the final corrected v17 arm is re-exported/reanalyzed for all120 windows. This is not a claim that historical v16 measurements were newly recomputed in full. Source tags in the archived mouths are not used for the terrain comparison.

| Cohort | Visible qualifying v16→v17 | All qualifying v16→v17 | Deep-connected qualifying v16→v17 | Visible deep v16→v17 | Supported exterior/deep v16→v17 |
| --- | --- | --- | --- | --- | --- |
| primary | 239→67 | 268→72 | 27→27 | 23→24 | 13→11 |
| independent | 165→44 | 186→44 | 27→21 | 18→21 | 11→9 |

| Unconditional-only visible qualifying rate / km² | v16 | v17 |
| --- | --- | --- |
| primary | 118.33 | 77.23 |
| independent | 124.56 | 94.67 |

Terrain changes invalidate the old terrain-conditional labels as v17 category assignments: these are matched X/Z windows, not newly balanced geographic quotas. The unconditional64/16 subsets provide the cleaner area-rate comparison. No cave parameter was adjusted to obtain these reductions. Fewer short sharp terrain features reduce the old adapter/surface intersections, while v16 exposure remains active. Deep access can still change: supported counts fall13→11 and11→9. Equal primary deep-mouth counts do not imply the same mouth coordinates or unchanged access routes. No universal entrance-frequency or Minecraft parity claim is made.

Actual v17 qualifying mouths include mountains (3 primary,2 independent), hills, plains, exposed cliffs and coasts. Classification uses measured geometry; a steep mountain face can be labeled cliff first. Counts are mouth counts, not terrain-area estimates. The old Horizon cave-stage report remains a separate pinned reference; its much greater deep-mouth prevalence is not solved by a terrain slice.

## Covered underground geometry

Sixteen independent unconditional windows use separate pre-cave geology for each version. The underground domain requires geological solid, cover≥8 and foundation exclusion. Cavity components use6-connectivity; supported floor graphs need two-Air clearance and permit only one-block jumps/drops. Their geodesic values are double-sweep lower bounds and all components/routes are window-clipped.

| Metric, summed over16 independent windows | v16 | v17 |
| --- | --- | --- |
| Eligible geological cells | 17738072 | 18025271 |
| Cavity cells | 2003879 | 1902634 |
| Components≥32768 cells | 13 | 14 |
| Supported floor nodes | 245404 | 250216 |
| Largest cavity component/window, median | 66114.5 | 57520.0 |
| Largest standing component/window, median | 4351.0 | 3932.0 |
| Supported route lower bound/window, median | 239.5 | 239.0 |

A joint mask requires solid geology in both controls, cover≥32 below both surfaces, Y≤−16, foundation exclusion and a24-block horizontal margin. Of3,060,113 cells,4,124 change cavity occupancy (0.1348%). This uses vertical cover, not a guarantee of Euclidean distance from every cliff. The result demonstrates retained underground composition with limited terrain coupling in this domain; it does not promise identical v17 deep voxels, unchanged whole-world connectivity or a fixed access percentage.

## Fifteen-biome ecology

The final73,728-point64-grid water/ecology survey covers four development seeds and all three signed windows. It runs actual versioned hydrology and point biome classification, not terrain-label proxies.

| Biome ID / identity | v16 samples | v17 samples |
| --- | --- | --- |
| 0 / Plains | 17876 | 19886 |
| 1 / Forest | 6362 | 6105 |
| 2 / Desert | 2495 | 2823 |
| 3 / Snowy Plains | 4885 | 5597 |
| 4 / Rocky Highlands | 3137 | 1398 |
| 5 / Meadow | 2460 | 1041 |
| 6 / Birch Forest | 4050 | 4267 |
| 7 / Taiga | 8942 | 10176 |
| 8 / Grove | 918 | 410 |
| 9 / Snowy Slopes | 4744 | 2131 |
| 10 / Stony Peaks | 286 | 187 |
| 11 / Beach | 371 | 364 |
| 12 / Ocean | 16989 | 18890 |
| 13 / River | 39 | 87 |
| 14 / Lake | 174 | 366 |

These are equally spaced sample counts, including both land and water; they are not exact world-wide biome fractions. Temperature/humidity, interval families, species/crowns, snowline/treeline rules and materials are unchanged. Different terrain/elevation/erosion changes placement. Fixed-site audits cover all fifteen actual ecosystems, supported roots/crowns/plants, density/cave intersections and point/cache/halo parity. The plant-save regression uses a recorded v17 grassland column, while historical versions retain their original sites.

## Hydrology v1

Water admission, ocean/river/lake evaluation, sea2, wet-bank seals and the fluid solver remain unchanged. The existing versioned facade supplies v17 height/envelope/rim samples consistently. Integration tests check source bottoms and lateral containment, roots not displacing water, protected spawn and admission-cache bounds. Existing water gameplay/mining/buckets/journal tests remain in the workspace suite. No river network, watershed simulation, new aquifer or Hydrology v2 is introduced. Candidate-footprint limitations and flat sea-level channels remain.

## Evidence and reproduction

`tools/prepare_terrain_controls.py work/uncaved` copies a disposable core and disables cave queries only there; production contains no cave-disable switch. Export both normal versions with `cave_volume`, and controls from that scratch executable. Use the unchanged Slice42 exposure plans/analyzer. The16-window underground report is produced by `tools/analyze_terrain_cave_integration.py`; raw records, hashes, plans, controls/source fingerprints and compressed metric records are under[raw/](raw/).
