# Slice37 — actual Bevy visual review

All images below are genuine RustCraft Bevy renders with original project assets,
production meshes, lighting, cutout/water shaders and atmosphere. Mesa llvmpipe
Vulkan under Xvfb renders the cloud captures. These are qualitative Minecraft
1.18+ ecosystem comparisons, not matched Minecraft images or native FPS evidence.
Exact seeds, cameras, executable/image SHA256 and adapter logs accompany
[captures.json](../../images/slice37/captures.json).

## Supported-distance ground cameras

These use radius4, normal runtime fog, FOV78 and an ordinary player eye. The
standalone gallery builds a fixed scene rather than exercising streaming. Meadow
and Stony Peaks have explicit look directions to show flowers/rock rather than
looking away from a summit into fog. The geometry and render-distance cap are
unchanged. MAX means18446744073709551615.

| Ecosystem | Seed; absolute X,Z | Actual v12 render |
| --- | --- | --- |
| Plains |0; -3520,-4032|![Plains](../../images/slice37/plains-v12.png)|
| Forest |0; 2048,-4032|![Forest](../../images/slice37/forest-v12.png)|
| Birch Forest |0; -1920,-4032|![Birch Forest](../../images/slice37/birch-forest-v12.png)|
| Taiga |0; 1600,-4032|![Taiga](../../images/slice37/taiga-v12.png)|
| Meadow |MAX; -1840,-3808|![Meadow](../../images/slice37/meadow-v12.png)|
| Grove |MAX; -1616,-4032|![Grove](../../images/slice37/grove-v12.png)|
| Snowy Slopes |0; -960,2496|![Snowy Slopes](../../images/slice37/snowy-slopes-v12.png)|
| Stony Peaks |42; 688,-1328|![Stony Peaks](../../images/slice37/stony-peaks-v12.png)|
| Desert |0; -2688,-3200|![Desert](../../images/slice37/desert-v12.png)|
| Beach |42; 3328,-3264|![Beach](../../images/slice37/beach-v12.png)|
| Snowy Plains |0; 384,-2752|![Snowy Plains](../../images/slice37/snowy-plains-v12.png)|
| Actual River |MAX; -4032,2112|![River](../../images/slice37/river-v12.png)|

Forest has mixed darker Oaks with occasional white Birch trunks; Birch Forest is
visibly white-trunk dominated. Taiga/Grove use Spruce and ferns. Meadows have
flower patches and sparse trees. Snowy upper slopes are treeless with exposed
Stone on steep steps; warm peaks retain rock. Beaches meet actual marine water.
The existing compressed world height produces much shorter ecological zones than
Minecraft. Ground fog intentionally limits a supported-distance view.

## Matched v11/v12 cameras and development panoramas

The Birch comparison uses ordinary radius4. All panorama rows use **development
radius10 with wide survey fog**, only in the standalone gallery; they are not
available runtime settings and do not demonstrate radius4 visibility/performance.
The two versions use identical seeds, centers, eye/direction, meshes/shaders and
lighting settings. Only intended materials/vegetation change.

| Site | v11 | v12 |
| --- | --- | --- |
| Birch woodland, seed0, -1920,-4032, radius4 |![v11 woodland](../../images/slice37/birch-forest-v11.png)|![v12 Birch](../../images/slice37/birch-forest-v12.png)|
| Cold mountain, seed2026, center3520,1728; **development panorama** |![v11 cold mountain](../../images/slice37/grove-panorama-v11.png)|![v12 cold mountain](../../images/slice37/grove-panorama-v12.png)|
| Warm range, seed42, center768,-896; **development panorama** |![v11 warm mountain](../../images/slice37/warm-peaks-panorama-v11.png)|![v12 warm mountain](../../images/slice37/warm-peaks-panorama-v12.png)|
| Coast, seed42, center3328,-3264; **development panorama** |![v11 coast](../../images/slice37/coast-panorama-v11.png)|![v12 coast](../../images/slice37/coast-panorama-v12.png)|

The cold mountain reveals lower woodland, grassy flower-bearing shoulders, wooded
cold slopes and exposed snowy upper terrain. The sequence varies across faces;
steep ledges remain bare. The warm range has rocky peaks with colder snow above
and greener lower slopes. Neither is a prescribed repeated biome ladder.

An additional **development radius10 panorama**, seedMAX, center-1760,-3920,
shows Meadow shoulders neighboring ordinary woodland. The anchor itself is Forest;
this is a regional transition view, not a claim that every visible block is Meadow.

![Meadow/forest transition development panorama](../../images/slice37/meadow-panorama-v12.png)

## Actual game, streaming and F3

These captures run the ordinary release client, create isolated new worlds through
the real menus, wait for convergence, walk, pause/resume, toggle F3 and exit through
normal saved quit. Radius4 Birch has81/81 ready columns and1296 loaded descriptors;
radius2 Grove has25/25 and400. Both report GENERATOR12, correct biome, no pending
loads/dirty work and no failed jobs. These checks supplement automated cancellation,
session switching and persistence tests; they are not a full native GUI acceptance.
The old DEV SLICE34 banner remains a historical UI label.

| Actual client view | F3 after walking |
| --- | --- |
|![Birch gameplay radius4](../../images/slice37/gameplay-birch-r4.png)|![Birch F3 radius4](../../images/slice37/gameplay-birch-r4-f3.png)|
|![Grove gameplay radius2](../../images/slice37/gameplay-grove-r2.png)|![Grove F3 radius2](../../images/slice37/gameplay-grove-r2-f3.png)|

[Game capture provenance](../../images/slice37/gameplay-captures.json) includes
normal exit status, complete window-title streaming observations and image hashes.
Its logs preserve the slow software-rendered FPS. Neither the cloud FPS nor the
owner's previous Radeon v11 observations is a universal acceptance threshold.
