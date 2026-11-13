# Matched CPU, resources and streaming

Baseline is exact merged Slice37 `cec8f593d20c012725ed17308bb9c7d44b470e08`,
generator12; candidate uses generator13. Both executables build before measurement.
Core source hashes match final core source. The streaming snapshot predates only
two test-only changes (occupancy-count assertions and the separately executed
v12/v13 cancellation loop); production evaluators are identical. Original hashes,
embedded runtime IDs and executable hashes are retained.

All four measurement passes ran sequentially without competing builds, tests or
renderers on this2-CPU Linux cloud executor. Forward order is v12→v13, reverse
order v13→v12. No samples/outliers are discarded after the declared warmups.
This is CPU-stage/resource evidence, not native FPS, physical GPU or VRAM data.

## Method and retained data

- Core:13 matched baseline ecosystem sites,128 measured columns plus8 warmups
  per site/build/order;24 real removal/save/reload/regeneration trials per build/order.
- `raw_us` generates a complete compact16-chunk column, including planning.
  `column_plan_us` isolates its sampler plan; `density_plan_us` includes shared
  geography/profile planning; `sparse_density_us` measures bounded profile
  construction including memo lookups, not just gradient arithmetic.
- Core worker measurements use the warmed source cache and include real lighting
  and opaque/water/plant meshing. They do not include a native GPU upload.
- Streaming:12 scripted scenarios at radii2/3/4,64 CPU observation frames,1 repeat
  per build/order. Real one-worker requests, queues, CPU mesh construction, Bevy
  assets/visibility/publication and resource cleanup are used without RenderApp/GPU.
- Names such as “plains”, “peaks” and “river” describe the **v12 baseline sites**.
  New geography intentionally changes their height/biome/mesh workload. These
  sites are not world-area-weighted and cannot establish a global speedup.
- Percentiles use rank `floor((n−1)*p/100)`. Each row retains its count; a single
  resource snapshot does not provide statistical tail confidence. Requested heap
  excludes allocator metadata, stacks and GPU; process RSS includes test machinery.

| Pass | All P50/P95/P99/max | Raw observations and provenance |
| --- | --- | --- |
|core-forward|[percentiles](benchmarks/core-forward/percentiles.csv)|[v12 columns](benchmarks/core-forward/slice37.csv) / [v13 columns](benchmarks/core-forward/slice38.csv); [method](benchmarks/core-forward/method.json), [source fingerprints](benchmarks/core-forward/sources.json)|
|core-reverse|[percentiles](benchmarks/core-reverse/percentiles.csv)|[v12 columns](benchmarks/core-reverse/slice37.csv) / [v13 columns](benchmarks/core-reverse/slice38.csv); [method](benchmarks/core-reverse/method.json), [source fingerprints](benchmarks/core-reverse/sources.json)|
|stream-forward|[percentiles](benchmarks/stream-forward/percentiles.csv)|[all event samples, gzip](benchmarks/stream-forward/samples.csv.gz); [method](benchmarks/stream-forward/method.json), [source fingerprints](benchmarks/stream-forward/sources.json)|
|stream-reverse|[percentiles](benchmarks/stream-reverse/percentiles.csv)|[all event samples, gzip](benchmarks/stream-reverse/samples.csv.gz); [method](benchmarks/stream-reverse/method.json), [source fingerprints](benchmarks/stream-reverse/sources.json)|

Streaming folders also retain lossless compressed original stdout, comparisons,
final states and executable fingerprints. Core folders include every lifecycle
trial. [Checksums](benchmarks/checksums.json) and [environment/scope](benchmarks/environment.json)
make the artifacts independently reviewable. The game needs none of this tooling.

## Core stage comparison

Forward-order site P50; times in milliseconds except sparse density in microseconds.

| Baseline site | Full column ms v12→v13 | Plan ms | Sparse profile µs | Warm light/mesh worker ms | Mesh bytes |
| --- | --- | --- | --- | --- | --- |
|plains|0.471→0.709|0.405→0.555|8.1→135.0|0.604→1.100|46872→170376|
|forest|0.718→0.923|0.455→0.656|8.3→179.8|1.264→1.408|383160→400520|
|birch|0.531→0.561|0.428→0.454|8.0→8.0|0.675→0.691|67208→128216|
|taiga|0.515→0.561|0.432→0.455|8.2→8.1|0.829→0.715|81344→80104|
|meadow|1.009→0.493|0.855→0.399|429.8→7.9|1.171→0.543|211792→50344|
|grove|1.293→0.786|1.105→0.683|653.7→218.9|0.813→0.637|184264→172112|
|grove-dense|1.612→0.523|1.423→0.439|864.0→8.2|1.386→0.718|259160→108128|
|snow|1.164→1.146|0.899→0.877|454.6→450.1|1.646→1.621|318432→295616|
|peaks|1.259→0.586|0.976→0.413|538.9→7.9|1.581→0.921|246264→189224|
|desert|0.604→0.589|0.415→0.418|8.1→8.0|0.730→0.724|112096→119040|
|beach|1.378→0.472|1.276→0.393|809.2→7.9|0.803→0.718|89728→91008|
|river|0.519→0.905|0.418→0.741|7.9→314.8|0.661→0.806|91944→174344|
|distant|0.548→0.784|0.429→0.600|8.0→133.2|0.722→0.871|159960→225680|

Both directions retain important adverse cases. The old “plains” site becomes
higher terrain: column P50 rises0.471→0.709ms (reverse0.515→0.707), and its
warm worker rises0.604→1.100ms (reverse0.659→1.083). Its mesh payload grows
46,872→170,376 bytes. The old river site likewise becomes higher/rugged; its
raw P50 rises0.519→0.905ms. These are real workload increases, not hidden outliers.

Conversely the old peaks site becomes lower: raw P50 falls1.259→0.586ms, while
Grove planning falls1.105→0.683ms. The retained snowy mountain stays expensive:
raw P50 is1.164→1.146ms forward and1.148→1.258ms reverse. Its density ceiling
and final elevation are not removed to improve the benchmark. The reverse Birch
raw maximum worsens1.897→5.831ms even though the median remains approximately
0.57ms; that unfavorable observation remains in the raw data and max columns.

## Streaming latency and publication

Ratio below is the median of per-scenario candidate/baseline P50 ratios across
the12 equally weighted scripts. It is descriptive, not a world-frequency estimate.

| Radius | Request-ready ratio forward / reverse | Worker-wall ratio forward / reverse | Main-install ratio forward / reverse | Worker-cache column maximum, both versions |
| --- | --- | --- | --- | --- |
|2|0.898 / 0.928|0.835 / 0.924|1.009 / 1.032|49|
|3|0.985 / 0.933|0.860 / 0.882|1.070 / 1.085|81|
|4|0.978 / 0.924|0.941 / 0.916|1.055 / 1.080|121|

Representative radius4 request-ready distributions below are **P50/P95/P99/max
milliseconds**. All other scenarios/radii remain in the linked CSVs.

| Order / site | v12 ms | v13 ms |
| --- | --- | --- |
|forward / forest|145.25 / 338.76 / 348.33 / 351.18|128.72 / 312.69 / 325.29 / 328.08|
|forward / mountain|193.66 / 370.17 / 386.07 / 389.06|142.46 / 289.22 / 299.42 / 302.70|
|forward / snow|198.16 / 378.99 / 394.96 / 400.13|193.82 / 377.22 / 392.19 / 399.00|
|forward / desert|105.24 / 201.52 / 210.95 / 213.61|122.45 / 223.94 / 234.62 / 237.87|
|forward / coast|148.05 / 298.12 / 309.17 / 312.06|117.49 / 270.45 / 288.94 / 292.45|
|forward / gated_teleport|167.62 / 326.41 / 340.28 / 346.43|157.49 / 312.44 / 333.11 / 344.24|
|reverse / forest|122.68 / 276.09 / 289.73 / 295.20|129.44 / 274.09 / 287.23 / 289.95|
|reverse / mountain|181.81 / 343.29 / 360.62 / 363.30|117.52 / 239.21 / 251.33 / 254.13|
|reverse / snow|188.76 / 384.15 / 400.17 / 405.25|195.00 / 373.94 / 385.52 / 388.44|
|reverse / desert|106.49 / 198.93 / 209.12 / 212.00|108.59 / 225.52 / 236.49 / 238.90|
|reverse / coast|142.62 / 298.54 / 308.94 / 312.05|114.39 / 247.46 / 259.42 / 261.59|
|reverse / gated_teleport|176.08 / 321.45 / 337.04 / 342.59|155.45 / 307.75 / 333.23 / 346.62|

Main-thread installation is mildly slower at many sites, with substantial local
exceptions. Radius4 coast install P50 rises0.280→0.435ms forward and0.252→0.458ms
reverse; the reverse radius-change install maximum rises3.297→5.102ms. Worst
stream-update observations remain33.127ms baseline and29.042ms candidate across
the two orders. Forward median update ratios rise, while reverse ratios fall;
this is not sufficient evidence for a uniform update-speed improvement.

CPU-visible latency follows ready latency closely; actual Bevy mesh construction,
command application, visibility and publication are measured separately. Worst
radius4 worker-wall observations include25.441→30.236ms forward,28.457→17.440ms
reverse. No unfavorable maxima are removed.

## Allocation, retained resources and lifecycle

The same one-worker/cache/radius budgets remain. Radius4 peaks across these
scripted scenarios are489→494 retained Mesh assets and26,028,344→24,628,880 mesh
payload bytes. This is not uniformly lower: the forest site grows401→494 assets
and16,877,872→19,921,824 payload bytes. Its cached voxel payload grows1,507,328→
1,765,376 bytes; the mountain site shrinks2,310,144→1,585,152. Uniform descriptors
remain compact and region/cache limits do not grow with travel.

Radius4 process high-water RSS maxima are74,748→71,848KiB forward and75,752→
72,852KiB reverse. Worker-window peak requested heap maxima are approximately
1.72→1.61MB. These headless process figures cannot be compared directly to the
owner’s full graphical-client RAM or physical GPU allocation.

Real lifecycle P50 forward: save0.122→0.113ms, reload3.526→2.727ms, regeneration
3.515→2.636ms. Reverse save is slower0.099→0.114ms; forward regeneration max is
also worse4.358→5.198ms. Every one of the24 trials removes eight actual generated
plants and verifies their saved Air after regeneration/reload; it is not empty IO.

The two isolated runs show comparable or lower request latency and bounded
retention overall, with real location-dependent work/resource regressions.
They do not establish native400FPS, physical GPU behavior or universal speedup.
No unrelated performance/renderer/streaming rewrite was introduced.

## Reproduction

Build first, then run with no concurrent build/test/renderer. Preserve each output
directory before the reverse run (including its verified executables/manifest).

```sh
python tools/benchmark_terrain_balance.py --output work/s38-core --build-only
python tools/benchmark_terrain_balance.py --output work/s38-core --reuse --samples 128 --warmups 8
# Copy that directory to work/s38-core-reverse, then:
python tools/benchmark_terrain_balance.py --output work/s38-core-reverse --reuse --reverse --samples 128 --warmups 8
python tools/benchmark_streaming.py --output work/s38-stream --baseline cec8f593d20c012725ed17308bb9c7d44b470e08 --labels slice37 slice38 --terrain-balance --distance-expansion --rendering-install --build-only
python tools/benchmark_streaming.py --output work/s38-stream --baseline cec8f593d20c012725ed17308bb9c7d44b470e08 --labels slice37 slice38 --terrain-balance --distance-expansion --rendering-install --reuse --repeats 1 --frames 64
# Copy that directory to work/s38-stream-reverse, then repeat with --reverse.
```

Use the native procedure in the Slice38 README for Gentoo/Void X11/Wayland and
Windows GUI/frame/GPU acceptance; the headless runner is not a substitute.
