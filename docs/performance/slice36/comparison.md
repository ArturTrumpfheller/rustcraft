# Actual terrain comparison — Slice 36

All RustCraft images below are genuine Bevy production-material renders of the
original assets. Ground-eye gallery views use supported radius4 and normal fog;
explicit panoramas use development-only radius10 and survey fog160–320. Runtime
render-distance limits are unchanged. Panoramas have identical absolute camera
poses; ground views select an open direction for each terrain version.
Exact commands/poses, renderer metadata and hashes: [manifest](raw/captures.json).

## Public modern Minecraft visual reference

[Mojang's 1.18 Java release article](https://www.minecraft.net/en-us/article/caves---cliffs--part-ii-out-today-java)
contains meadow/grove and mountain photographs. Its ordinary grass/forested
foothill views guide the plains/hills/forest categories; its mountain/coast/canyon
views guide range and transition structure. [Mojang's developer Q&A photograph](https://www.minecraft.net/en-us/article/caves---cliffs-update--part-ii-dev-q-a)
shows rock, irregular peaks and a wooded lower landscape. The linked
[Wiki terrain-feature documentation](https://minecraft.wiki/w/Terrain_features)
adds hill/cliff/shore context, but includes historical examples and is not
uniformly a modern1.18 reference. No spectacular seed collection is used as the
sole target. No Minecraft executable, matched captures, known reference camera
poses or pixel measurements were available; these are qualitative references.
Reference images remain on their sources and are not imported as project assets.

## Matched development panorama: range

| Generator10 | Generator11 |
| --- | --- |
| ![Development panorama v10 rounded contour walls](../../../images/slice36/range-panorama-v10.png) | ![Development panorama v11 irregular peaks and saddles](../../../images/slice36/range-panorama-v11.png) |

At this site v10's nearly uniform rounded shoulders and concentric terrace pattern
become a succession of separate summits, asymmetric flanks and saddles in v11.
The old large ring pattern is absent in this matched view. Occasional curved or
rounded geographical forms remain elsewhere; the survey's radial proxies do not
prove worldwide elimination. This is a visible model change at the same place,
not a height-range or texture change. The model was revised after an earlier
candidate still showed a broad dome and a repeated diagonal grain.

## Supported-radius ground-eye comparisons

Category names describe terrain at a representative site, not a new biome ID.
Geography/water may differ between versions at the same X/Z. In particular the
new plain site is marine in v10; use the separate v10-plains-site row for an
ordinary v10 plain reference. Ground yaw can differ; this is not a matched-pixel
experiment. The cold foothill row includes a real cliff rather than only gentle
foothills. Normal fog intentionally limits how much distant geography is visible.

| Site / seed / X,Z | Generator10 | Generator11 |
| --- | --- | --- |
| v10-plains-site · 42 · -144,752 | ![v10 v10-plains-site](../../../images/slice36/v10-plains-site-v10.png) | ![v11 v10-plains-site](../../../images/slice36/v10-plains-site-v11.png) |
| plains · 2026 · -2560,-4096 | ![v10 plains](../../../images/slice36/plains-v10.png) | ![v11 plains](../../../images/slice36/plains-v11.png) |
| rolling-hills · 42 · -2880,720 | ![v10 rolling-hills](../../../images/slice36/rolling-hills-v10.png) | ![v11 rolling-hills](../../../images/slice36/rolling-hills-v11.png) |
| forest · 18446744073709551615 · -3984,-3152 | ![v10 forest](../../../images/slice36/forest-v10.png) | ![v11 forest](../../../images/slice36/forest-v11.png) |
| foothills · 42 · -992,-3488 | ![v10 foothills](../../../images/slice36/foothills-v10.png) | ![v11 foothills](../../../images/slice36/foothills-v11.png) |
| rocky-peaks · 0 · -3984,-3152 | ![v10 rocky-peaks](../../../images/slice36/rocky-peaks-v10.png) | ![v11 rocky-peaks](../../../images/slice36/rocky-peaks-v11.png) |
| valley · 0 · -512,-3200 | ![v10 valley](../../../images/slice36/valley-v10.png) | ![v11 valley](../../../images/slice36/valley-v11.png) |
| coast-lowlands · 0 · -2176,-4096 | ![v10 coast-lowlands](../../../images/slice36/coast-lowlands-v10.png) | ![v11 coast-lowlands](../../../images/slice36/coast-lowlands-v11.png) |

## Additional development panoramas

| View | Generator10 | Generator11 |
| --- | --- | --- |
| Development-only peaks-panorama · seed 0 · -3360,-2576 | ![Development v10 peaks-panorama](../../../images/slice36/peaks-panorama-v10.png) | ![Development v11 peaks-panorama](../../../images/slice36/peaks-panorama-v11.png) |
| Development-only valley-panorama · seed 0 · -512,-3200 | ![Development v10 valley-panorama](../../../images/slice36/valley-panorama-v10.png) | ![Development v11 valley-panorama](../../../images/slice36/valley-panorama-v11.png) |
| Development-only transition-panorama · seed 42 · -992,-3488 | ![Development v10 transition-panorama](../../../images/slice36/transition-panorama-v10.png) | ![Development v11 transition-panorama](../../../images/slice36/transition-panorama-v11.png) |

## Actual production-client gameplay

These additional images use the unmodified production client, real streaming,
physics, world catalogs, inventory/hand and normal fog. The range capture reloads
an isolated world after one earlier Stone removal in a different column. The
plain is a separate fresh, unedited seed2026 world. Neither changes terrain to
compose the picture. The F3 header still carries the project's existing DEV
SLICE34 label; its **generator line is11**, with body ready and zero load/dirty
work after convergence. F3's biome name remains the old five-ID climate label;
terrain/plain describes geography, not a new biome. Software-Mesa FPS values
are not measurements of native Radeon or 240Hz performance.

| Radius2 ordinary plain | Radius4 high mountain flank with normal fog |
| --- | --- |
| ![Real client plain radius2](../../../images/slice36/gameplay-plains-v11.png) | ![Real client range radius4](../../../images/slice36/gameplay-range-v11.png) |
| ![Plain F3 generator11 and400 descriptors](../../../images/slice36/gameplay-plains-f3-v11.png) | ![Range F3 generator11 and1296 descriptors](../../../images/slice36/gameplay-range-f3-v11.png) |

The mountain flank demonstrates why the development panoramas are needed: the
supported gameplay fog hides distant summits. [Gameplay capture metadata](raw/gameplay-captures.json).
Prepare a new isolated catalog with
`cargo run --release --locked -p voxel_core --example terrain_capture_world -- ABSOLUTE_EMPTY_DIRECTORY SEED`,
then launch the normal client with that directory, recorded start chunk/radius,
choose Singleplayer → world → Play, and use F3 for the recorded pose. The normal
client's saved default is11; do not change a historical world's manifest.

## Assessment and remaining differences

V11 preserves usable quiet plains and rolling wooded terrain while making the
range and valley geometry more varied and asymmetric. It is visibly closer to
the ordinary modern-Overworld geographical character at these representative
sites. Higher complexity alone was not the acceptance test: the broad-dome and
single-grain candidates were rejected after rendered/map review. All heights,
slopes, material decisions, footprint checks and image poses are reproducible.

It is still RustCraft: original textures/tree silhouettes, only five climate
biomes, a256-block vertical volume with seaY2, smaller bounded density detail,
legacy caves and simple non-watershed water. Some cliffs are sharp; old cave
mouths can be buried; small inherited ponds can retain rounded outlines. No
Minecraft seed/block/biome parity, new aquifer system or native owner acceptance
is claimed. Explore the supplied locations on native hardware before merging.
