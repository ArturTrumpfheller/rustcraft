# Genuine Bevy v16/v17 comparisons

Thirty-four captures form17 matched pairs across seeds0/42/2026. Every image is actual generated RustCraft rendered with production meshes, lighting, original textures, sky/fog and materials. No terrain blocks, cave lights, decorations, generated heights or density are edited for presentation. PPM frames are encoded as PNG without compositing or appearance changes.

Normal views use supported radius4, production fog and the normal supported spawn/eye-height calculation at each requested location. Camera Y can differ because terrain height changes; resolved poses are listed below. Input seed/X/Z/yaw are identical per pair. Landscape names refer to v17 selection; v16 can be a different terrain category at the same coordinates. A separate shared-lowland control selects Plains with low relief in both versions. These bounded selected views are illustrations, not independent landscape-frequency measurements.

Development-only panoramas use the existing gallery radius10 and wide inspection fog. They have explicit identical world camera poses per pair, with no change to the game's radius1–4 limit. Their extra visible geography is a development inspection aid, not normal gameplay draw distance.

Captures use1280×720 Xvfb and Mesa llvmpipe Vulkan; this is neither the owner's RX9060 XT nor a native GPU/FPS benchmark. Every command, camera log and image/executable SHA256 is retained in[captures.json](captures/captures.json). The terrain-only selection criteria/coordinates are in[plan.json](captures/plan.json).

## Supported gameplay-distance views (radius4)

### broad plains

Seed0; requested X/Z=(-2624,-3584); radius4. World eye v16=(-2623.5, 48.62, -3583.5), v17=(-2623.5, 15.62, -3583.5).

| v16 | v17 |
| --- | --- |
| ![v16 broad-plains](captures/broad-plains-v16.png) | ![v17 broad-plains](captures/broad-plains-v17.png) |

### rolling hills

Seed42; requested X/Z=(-2496,-3264); radius4. World eye v16=(-2495.5, 12.62, -3263.5), v17=(-2495.5, 28.62, -3263.5).

| v16 | v17 |
| --- | --- |
| ![v16 rolling-hills](captures/rolling-hills-v16.png) | ![v17 rolling-hills](captures/rolling-hills-v17.png) |

### forested lowlands

Seed0; requested X/Z=(1664,-3584); radius4. World eye v16=(1664.5, -15.38, -3583.5), v17=(1664.5, 17.62, -3583.5).

| v16 | v17 |
| --- | --- |
| ![v16 forested-lowlands](captures/forested-lowlands-v16.png) | ![v17 forested-lowlands](captures/forested-lowlands-v17.png) |

### foothills

Seed2026; requested X/Z=(-1920,-3520); radius4. World eye v16=(-1919.5, 23.62, -3519.5), v17=(-1919.5, 40.62, -3519.5).

| v16 | v17 |
| --- | --- |
| ![v16 foothills](captures/foothills-v16.png) | ![v17 foothills](captures/foothills-v17.png) |

### connected range

Seed42; requested X/Z=(-1984,-2944); radius4. World eye v16=(-1983.5, 34.62, -2943.5), v17=(-1983.5, 94.62, -2943.5).

| v16 | v17 |
| --- | --- |
| ![v16 connected-range](captures/connected-range-v16.png) | ![v17 connected-range](captures/connected-range-v17.png) |

### mountain valley

Seed0; requested X/Z=(-1024,-3008); radius4. World eye v16=(-1023.5, 28.62, -3007.5), v17=(-1023.5, 31.62, -3007.5).

| v16 | v17 |
| --- | --- |
| ![v16 mountain-valley](captures/mountain-valley-v16.png) | ![v17 mountain-valley](captures/mountain-valley-v17.png) |

### rocky cliff

Seed2026; requested X/Z=(-2368,-3584); radius4. World eye v16=(-2367.5, 36.62, -3583.5), v17=(-2367.5, 88.62, -3583.5).

| v16 | v17 |
| --- | --- |
| ![v16 rocky-cliff](captures/rocky-cliff-v16.png) | ![v17 rocky-cliff](captures/rocky-cliff-v17.png) |

### snowy peaks

Seed42; requested X/Z=(-1920,-2880); radius4. World eye v16=(-1919.5, 30.62, -2879.5), v17=(-1919.5, 115.62, -2879.5).

| v16 | v17 |
| --- | --- |
| ![v16 snowy-peaks](captures/snowy-peaks-v16.png) | ![v17 snowy-peaks](captures/snowy-peaks-v17.png) |

### coastal lowlands

Seed0; requested X/Z=(64,-3456); radius4. World eye v16=(64.5, 18.62, -3455.5), v17=(64.5, 6.62, -3455.5).

| v16 | v17 |
| --- | --- |
| ![v16 coastal-lowlands](captures/coastal-lowlands-v16.png) | ![v17 coastal-lowlands](captures/coastal-lowlands-v17.png) |

### terrain transition

Seed2026; requested X/Z=(-2112,-3456); radius4. World eye v16=(-2111.5, 19.62, -3455.5), v17=(-2111.5, 43.62, -3455.5).

| v16 | v17 |
| --- | --- |
| ![v16 terrain-transition](captures/terrain-transition-v16.png) | ![v17 terrain-transition](captures/terrain-transition-v17.png) |

### shared lowland control

Seed0; requested X/Z=(2944,-3584); radius4. World eye v16=(2944.5, 28.62, -3583.5), v17=(2944.5, 21.62, -3583.5).

| v16 | v17 |
| --- | --- |
| ![v16 shared-lowland-control](captures/shared-lowland-control-v16.png) | ![v17 shared-lowland-control](captures/shared-lowland-control-v17.png) |

## Development-only wide panoramas (radius10)

### broad plains panorama

Seed0; requested X/Z=(-2624,-3584); radius10. World eye v16=(-2720.0, 80.0, -3472.0), v17=(-2720.0, 80.0, -3472.0).

| v16 | v17 |
| --- | --- |
| ![v16 broad-plains-panorama](captures/broad-plains-panorama-v16.png) | ![v17 broad-plains-panorama](captures/broad-plains-panorama-v17.png) |

### rolling hills panorama

Seed42; requested X/Z=(-2496,-3264); radius10. World eye v16=(-2592.0, 80.0, -3152.0), v17=(-2592.0, 80.0, -3152.0).

| v16 | v17 |
| --- | --- |
| ![v16 rolling-hills-panorama](captures/rolling-hills-panorama-v16.png) | ![v17 rolling-hills-panorama](captures/rolling-hills-panorama-v17.png) |

### connected range panorama

Seed42; requested X/Z=(-1984,-2944); radius10. World eye v16=(-2080.0, 142.0, -2832.0), v17=(-2080.0, 142.0, -2832.0).

| v16 | v17 |
| --- | --- |
| ![v16 connected-range-panorama](captures/connected-range-panorama-v16.png) | ![v17 connected-range-panorama](captures/connected-range-panorama-v17.png) |

### mountain valley panorama

Seed0; requested X/Z=(-1024,-3008); radius10. World eye v16=(-1120.0, 121.0, -2896.0), v17=(-1120.0, 121.0, -2896.0).

| v16 | v17 |
| --- | --- |
| ![v16 mountain-valley-panorama](captures/mountain-valley-panorama-v16.png) | ![v17 mountain-valley-panorama](captures/mountain-valley-panorama-v17.png) |

### rocky cliff panorama

Seed2026; requested X/Z=(-2368,-3584); radius10. World eye v16=(-2464.0, 136.0, -3472.0), v17=(-2464.0, 136.0, -3472.0).

| v16 | v17 |
| --- | --- |
| ![v16 rocky-cliff-panorama](captures/rocky-cliff-panorama-v16.png) | ![v17 rocky-cliff-panorama](captures/rocky-cliff-panorama-v17.png) |

### terrain transition panorama

Seed2026; requested X/Z=(-2112,-3456); radius10. World eye v16=(-2208.0, 116.0, -3344.0), v17=(-2208.0, 116.0, -3344.0).

| v16 | v17 |
| --- | --- |
| ![v16 terrain-transition-panorama](captures/terrain-transition-panorama-v16.png) | ![v17 terrain-transition-panorama](captures/terrain-transition-panorama-v17.png) |

## Visual observations and limits

The new plains views retain small steps/depressions and sparse vegetation. Hills/forested land show low rolling changes instead of the old short ridge budget everywhere. The range panorama exposes connected snowy ridges, saddles, side cliffs and a broad vegetated approach. The valley/transition panoramas show high relief concentrated around selected ranges with walkable lower ground. Ordinary views deliberately keep production fog, so they cannot display entire multi-hundred-block ranges; use the explicitly labeled panoramas/maps for that scale.

These are selected local examples, not a proof that every summit/valley has a natural silhouette or that every biome transition is ideal. Some snow/soil layering and block terraces remain visually strong; rare overhangs/cliffs can still be dramatic. Quantitative surveys and fixed cross-sections cover a much larger domain. No Minecraft screenshots, reference-game assets, AI-generated images or heightmap renderings are presented as Bevy gameplay.

```sh
# With an actual X11/Wayland display, or an explicitly configured software/Xvfb development environment:
cargo build --release --locked -p rustcraft --example terrain_gallery
python tools/capture_terrain_geography.py --plan docs/performance/slice43/captures/plan.json --output work/captures --executable target/release/examples/terrain_gallery
```
