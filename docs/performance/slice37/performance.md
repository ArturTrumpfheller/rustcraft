# Slice37 — exact Slice36 performance comparison

Baseline is3d0f13c7e603ed060d53a722cf73c51619ade308, generator11. Candidate is
the final generator12 implementation. Both builds finish before measurement.
Forward and reverse execution orders run with no concurrent compiler, renderer or
test suite. Source/executable fingerprints accompany every raw run. Linux cloud:
Rust1.99.1, two CPU quota,8GiB cgroup. This is CPU/software infrastructure evidence,
not the owner's Radeon FPS or a native hardware acceptance.

There are256 observations per core scene (128 in each order,8 excluded warmups),
24 lifecycle trials per order/build,64 paced streaming frames per scene/radius/order,
real Bevy CPU mesh/visibility/install probes, and Linux wait4 process CPU/RSS.
P50/P95/P99 use floor((n-1)*p/100); max is retained. No outlier is discarded.

Raw `raw_us` is sampler planning plus all16 vertical chunks with reusable bounded
generator admission caches. `worker_us` repeats cached raw-column worker output,
including lighting/meshing; it is not a second cold generation measurement.
Streaming separately measures real asynchronous request-to-ready/visible latency.
Clock/allocation probes are only in disposable exports. Requested heap excludes
allocator metadata, stacks, libraries and GPU; process RSS includes those CPU
process costs. Different ecosystems intentionally produce different payloads.

## Results and remaining costs

The optimized coastal cold-column P50 is1.421ms versus v11's0.693ms (+105%):
exact finite marine proximity proof still costs extra work. It improves from the
initial v12's3.281ms. The final coastal classification P50 falls from2.484ms to
0.652ms; its18.654ms whole-column max remains in the table. The protected starting
shore uses full final-depth evaluation, and the tile-local shore memo has at most
756 exterior entries. Intermediate optimization runs are also retained under
`raw/after-first-optimization-*`.

At radius4 the coast's request-ready P95 is291ms versus270ms. Ordinary Forest
has the larger remaining ecosystem cost: its converged mesh payload grows from
11.56MB to16.88MB, and request-ready P95 grows from258ms to475ms. The current
forest contains substantially more foliage at these same coordinates. Its
stream-update P50 stays about2.6us and P95 is345us versus338us; this does not
measure GPU FPS. Birch/Taiga cached workers also increase about25.5%/23.2% at
P50. These regressions are reported, not treated as zero-cost biome labels.

Whole streaming-process high-water RSS is73.70/74.02MiB for v11 and73.94/74.10MiB
for v12 in forward/reverse order. Forest's per-scene resident RSS still rises
52.73→59.86MiB; the high-water peak across every workload cannot substitute for
that scene-specific result. These CPU benchmark processes omit a graphical
client/GPU. Caches remain bounded and are reclaimed by existing lifecycle tests.
Native hardware must assess the foliage/payload tradeoff before owner acceptance.

## Core column and worker tails

Every tuple is **P50/P95/P99/max in microseconds**. All13 matched coordinates,
including steep and denser Groves and a distant region, are retained.

| Scene | v11 raw column | v12 raw column | v11 cached worker | v12 cached worker |
| --- | --- | --- | --- | --- |
| plains | 455.6/825.8/1374.9/4017.0 | 460.2/696.8/1184.9/2169.7 | 651.3/1129.2/3001.9/10739.2 | 611.6/1013.7/1543.5/2040.7 |
| forest | 639.1/1052.2/2099.2/112706.1 | 689.0/1015.3/1279.0/1702.9 | 1101.1/1770.5/3159.2/4074.4 | 1243.1/1981.9/2546.2/4782.1 |
| birch | 481.2/812.4/1102.2/1420.6 | 551.0/1015.2/1757.8/10352.3 | 569.7/1164.7/2404.6/12219.8 | 715.2/2044.0/6432.4/10664.9 |
| taiga | 524.7/760.6/939.4/1126.1 | 534.3/1065.3/2658.3/5012.0 | 701.0/997.6/1178.4/1451.1 | 863.4/2080.3/3847.3/70530.3 |
| meadow | 951.7/1403.6/1791.0/4258.9 | 1012.8/1631.8/2577.4/99883.9 | 1112.8/2306.0/3254.7/10098.5 | 1136.9/1863.3/2985.2/6727.4 |
| grove | 1127.0/1588.1/1669.1/1860.3 | 1276.1/1839.9/2685.9/28832.5 | 791.2/1087.3/1514.5/1779.6 | 795.0/1202.8/2998.6/18526.3 |
| grove-dense | 1303.3/1967.9/3746.6/8758.3 | 1513.9/2149.3/3785.3/5296.4 | 1177.7/2710.0/5155.7/12284.9 | 1350.4/2344.1/2974.5/10216.4 |
| snow | 1111.0/1475.1/2627.6/3885.3 | 1141.6/1591.7/1940.2/3603.4 | 1597.9/2168.5/2944.2/5758.5 | 1614.5/2483.8/3257.8/7088.7 |
| peaks | 1123.8/1527.2/1778.2/3179.2 | 1260.7/1773.6/2200.3/3990.8 | 1621.5/2284.9/3435.9/13640.9 | 1606.8/2259.7/2967.6/7307.6 |
| desert | 662.2/1005.0/1396.3/1613.3 | 568.9/883.4/1361.4/1519.5 | 842.9/1236.8/1815.4/2532.9 | 729.7/1271.5/2816.9/37287.8 |
| beach | 692.6/1071.6/1958.8/5097.2 | 1421.1/2432.7/5338.3/18654.0 | 770.0/1298.0/1753.7/8200.0 | 806.3/1721.6/3613.6/17982.6 |
| river | 481.8/769.2/1140.3/3347.8 | 500.7/779.5/1083.7/1661.1 | 566.3/905.7/1049.3/1156.6 | 676.5/1036.7/1437.5/4777.8 |
| distant | 517.3/820.8/1147.1/3356.2 | 579.6/954.8/1501.3/1637.7 | 679.0/1166.5/2068.4/4929.1 | 728.3/1279.0/1871.7/2998.0 |

## Stage P50 costs

Microseconds; stages overlap where an outer operation contains another. Climate
includes inherited old-climate water admission plus new v12 T/H; shared terrain
C/E/W belongs to density/profile planning. V11 classification is the historical
threshold only; v12 classification also includes materials and shoreline proof.

| Scene/build | Climate | Classification/material | Trees | Plants | Lighting | Opaque mesh | Water mesh | Plant mesh |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| plains/slice36 | 50.3 | 6.2 | 2.4 | 29.0 | 322.4 | 175.8 | 0.6 | 2.9 |
| plains/slice37 | 46.7 | 22.6 | 33.4 | 12.6 | 333.7 | 153.1 | 0.7 | 0.0 |
| forest/slice36 | 50.5 | 18.5 | 1.9 | 30.0 | 543.9 | 394.7 | 0.7 | 4.1 |
| forest/slice37 | 47.6 | 24.7 | 55.0 | 19.1 | 611.6 | 487.6 | 1.1 | 4.5 |
| birch/slice36 | 50.6 | 18.5 | 2.2 | 27.7 | 334.2 | 113.8 | 0.5 | 0.0 |
| birch/slice37 | 47.8 | 25.7 | 37.6 | 14.6 | 413.0 | 162.9 | 0.8 | 1.1 |
| taiga/slice36 | 55.0 | 19.0 | 42.5 | 30.1 | 413.3 | 145.2 | 0.6 | 4.4 |
| taiga/slice37 | 46.8 | 19.2 | 40.2 | 22.1 | 510.9 | 183.8 | 1.0 | 9.9 |
| meadow/slice36 | 50.4 | 16.3 | 1.8 | 29.1 | 667.7 | 275.4 | 0.8 | 5.5 |
| meadow/slice37 | 47.3 | 24.2 | 54.2 | 28.2 | 662.4 | 273.6 | 1.1 | 15.0 |
| grove/slice36 | 52.0 | 14.4 | 82.7 | 31.6 | 380.0 | 265.4 | 0.7 | 6.9 |
| grove/slice37 | 46.7 | 27.1 | 223.2 | 14.8 | 380.0 | 273.1 | 0.8 | 4.5 |
| grove-dense/slice36 | 55.1 | 17.3 | 285.6 | 38.1 | 660.6 | 323.9 | 0.9 | 6.1 |
| grove-dense/slice37 | 46.8 | 25.7 | 440.5 | 28.5 | 789.1 | 389.3 | 1.3 | 8.5 |
| snow/slice36 | 49.8 | 6.2 | 2.1 | 27.3 | 961.5 | 454.6 | 0.9 | 0.0 |
| snow/slice37 | 45.8 | 23.0 | 19.0 | 11.4 | 1009.7 | 437.5 | 1.3 | 0.0 |
| peaks/slice36 | 51.0 | 6.3 | 2.1 | 30.0 | 1051.6 | 381.1 | 0.8 | 5.1 |
| peaks/slice37 | 48.6 | 25.0 | 108.6 | 11.4 | 1050.2 | 384.2 | 1.2 | 0.0 |
| desert/slice36 | 66.6 | 20.1 | 81.5 | 35.3 | 399.1 | 295.2 | 0.7 | 5.1 |
| desert/slice37 | 48.7 | 24.5 | 33.7 | 13.1 | 354.7 | 233.6 | 0.9 | 0.9 |
| beach/slice36 | 50.8 | 14.1 | 2.4 | 8.4 | 414.2 | 169.6 | 48.2 | 0.0 |
| beach/slice37 | 47.5 | 651.6 | 99.2 | 3.9 | 429.6 | 173.6 | 56.0 | 0.0 |
| river/slice36 | 49.6 | 14.9 | 1.8 | 17.2 | 268.4 | 159.3 | 39.9 | 0.0 |
| river/slice37 | 46.1 | 18.8 | 25.4 | 12.6 | 303.9 | 168.7 | 47.5 | 5.3 |
| distant/slice36 | 50.9 | 18.5 | 2.1 | 33.1 | 294.1 | 243.4 | 0.6 | 5.2 |
| distant/slice37 | 48.2 | 23.2 | 35.1 | 26.7 | 316.6 | 252.8 | 0.9 | 10.2 |

## Allocations, temporary heap and resident payload

P50 per observation, bytes unless allocation calls. Requested bytes are cumulative
allocation traffic, not live memory. Raw peak is the temporary planning/raster
high-water mark; worker peak includes lighting/mesh output. All percentile tails
for these metrics remain in each raw `percentiles.csv`.

| Scene/build | Raw allocation calls | Raw requested B | Raw peak B | Worker allocation calls | Worker peak B | Mesh payload B | Voxel cache B | Water cache B | Light cache B |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| plains/slice36 | 18 | 49,888 | 21,844 | 206 | 511,304 | 48,856 | 77,824 | 960 | 34,992 |
| plains/slice37 | 23 | 66,308 | 39,892 | 171 | 507,336 | 46,872 | 73,728 | 960 | 29,160 |
| forest/slice36 | 23 | 62,624 | 28,212 | 306 | 887,572 | 309,256 | 143,360 | 2,520 | 34,992 |
| forest/slice37 | 59 | 100,708 | 57,140 | 319 | 942,868 | 383,160 | 135,168 | 2,520 | 34,992 |
| birch/slice36 | 13 | 45,152 | 21,268 | 118 | 504,136 | 11,904 | 69,632 | 0 | 29,160 |
| birch/slice37 | 32 | 68,516 | 41,620 | 149 | 575,944 | 67,208 | 73,728 | 0 | 29,160 |
| taiga/slice36 | 30 | 49,056 | 21,732 | 198 | 600,008 | 23,560 | 98,304 | 0 | 34,992 |
| taiga/slice37 | 39 | 70,788 | 41,892 | 241 | 622,024 | 81,344 | 118,784 | 0 | 34,992 |
| meadow/slice36 | 17 | 52,208 | 21,812 | 248 | 831,304 | 183,024 | 126,976 | 0 | 40,824 |
| meadow/slice37 | 29 | 78,772 | 40,676 | 284 | 831,304 | 211,792 | 126,976 | 0 | 40,824 |
| grove/slice36 | 21 | 57,432 | 21,780 | 291 | 668,548 | 190,712 | 102,400 | 0 | 34,992 |
| grove/slice37 | 27 | 77,212 | 39,412 | 277 | 652,548 | 184,264 | 98,304 | 0 | 34,992 |
| grove-dense/slice36 | 35 | 66,044 | 24,788 | 297 | 844,616 | 179,552 | 139,264 | 9,792 | 46,656 |
| grove-dense/slice37 | 55 | 88,928 | 47,636 | 316 | 914,020 | 259,160 | 159,744 | 9,792 | 46,656 |
| snow/slice36 | 21 | 72,884 | 27,924 | 343 | 1,235,528 | 316,448 | 192,512 | 0 | 46,656 |
| snow/slice37 | 29 | 105,256 | 51,524 | 343 | 1,235,528 | 318,432 | 192,512 | 0 | 46,656 |
| peaks/slice36 | 23 | 66,872 | 24,852 | 407 | 1,200,472 | 253,952 | 225,280 | 7,008 | 40,824 |
| peaks/slice37 | 31 | 112,444 | 55,188 | 352 | 1,196,376 | 246,264 | 221,184 | 7,008 | 40,824 |
| desert/slice36 | 47 | 67,484 | 27,860 | 305 | 650,708 | 190,216 | 122,880 | 0 | 34,992 |
| desert/slice37 | 30 | 82,656 | 44,020 | 212 | 556,360 | 112,096 | 122,880 | 0 | 34,992 |
| beach/slice36 | 15 | 47,184 | 22,100 | 260 | 624,072 | 89,728 | 122,880 | 0 | 29,160 |
| beach/slice37 | 40 | 75,828 | 42,708 | 260 | 624,072 | 89,728 | 122,880 | 0 | 29,160 |
| river/slice36 | 16 | 45,856 | 21,684 | 216 | 451,784 | 81,528 | 86,016 | 0 | 29,160 |
| river/slice37 | 30 | 69,828 | 40,820 | 266 | 453,832 | 91,944 | 86,016 | 0 | 29,160 |
| distant/slice36 | 22 | 56,252 | 25,108 | 239 | 554,700 | 137,640 | 90,112 | 0 | 34,992 |
| distant/slice37 | 36 | 92,512 | 49,012 | 251 | 570,572 | 159,960 | 90,112 | 0 | 34,992 |

## Streaming and frame/update tails, radii2/3/4

Tuples are **P50/P95/P99/max microseconds** across both orders. Request-to-ready
includes queueing/worker/publication. It is sampled per completed column, not per
frame. Stream update is CPU work in the frame's streaming system. Tail latencies
can include cloud scheduling delays. Radius stress changes supported distances;
it is a transition workload rather than a stationary scene. Gated teleport tests
cancellation. Session probes additionally retain radius1 compatibility workloads.

| Scene/radius | v11 request ready | v12 request ready | v11 stream update | v12 stream update |
| --- | --- | --- | --- | --- |
| stationary/r2 | 36659.2/101415.8/114006.0/118032.3 | 48549.4/97154.9/102405.5/106212.0 | 2.2/1017.1/1372.3/1734.1 | 1.8/965.3/1163.1/1308.2 |
| stationary/r3 | 77054.4/173606.9/182719.8/188398.1 | 97191.1/195530.1/203351.0/206393.5 | 2.7/1229.6/1773.9/3495.5 | 1.9/1154.3/1595.9/2172.5 |
| stationary/r4 | 151849.4/387104.3/436290.3/443229.5 | 169515.5/377723.4/417635.9/425465.5 | 3.0/1921.6/4288.1/9109.4 | 2.9/1384.8/2669.8/6719.9 |
| forest/r2 | 36980.0/91727.3/97396.9/100113.6 | 40809.5/103126.0/107503.2/110719.2 | 2.3/254.7/311.9/1612.5 | 1.9/249.0/428.3/895.7 |
| forest/r3 | 69624.4/167244.7/175989.9/181703.7 | 62683.5/151220.4/158211.2/163782.0 | 2.3/286.2/622.2/1325.8 | 2.0/217.5/400.5/503.1 |
| forest/r4 | 121217.1/258087.3/278541.5/284760.4 | 149369.9/474966.7/501721.3/508225.7 | 2.7/338.0/626.8/1959.9 | 2.6/344.5/775.5/3731.6 |
| birch/r2 | 35204.8/80392.8/88519.4/91367.6 | 40059.7/82252.1/86350.0/89566.3 | 2.2/206.9/300.3/423.7 | 1.9/235.2/331.5/434.6 |
| birch/r3 | 68312.8/142692.6/147963.9/151764.2 | 65565.6/142411.5/151790.8/157097.6 | 2.4/241.3/373.0/1462.2 | 1.8/221.3/261.8/354.8 |
| birch/r4 | 117163.9/234390.9/244145.6/246852.0 | 177789.6/380736.9/411535.0/420944.8 | 2.2/259.4/393.6/664.3 | 2.5/354.1/1035.5/15851.6 |
| taiga/r2 | 33009.2/91255.4/99773.7/102369.9 | 45535.2/86575.8/90634.7/93902.7 | 2.4/267.0/528.1/2155.8 | 1.7/236.9/497.6/1267.9 |
| taiga/r3 | 64962.4/139612.5/144982.3/148215.4 | 71097.0/155146.8/161759.4/168281.8 | 2.3/252.3/342.0/473.6 | 2.1/249.8/372.0/849.6 |
| taiga/r4 | 103963.4/218067.1/238003.4/244589.0 | 134122.4/256451.8/267747.0/273506.9 | 2.4/353.3/484.8/9977.0 | 2.4/259.9/379.1/684.4 |
| meadow/r2 | 40937.0/92581.2/99038.0/102558.9 | 44741.6/96616.5/102397.9/106355.9 | 1.7/210.7/273.1/495.9 | 2.1/206.9/253.1/507.7 |
| meadow/r3 | 85706.2/172065.5/179300.4/184722.2 | 105722.7/194248.1/201014.6/205606.7 | 2.3/270.1/418.5/873.1 | 2.0/261.3/445.5/2576.9 |
| meadow/r4 | 146321.0/283114.5/306406.5/313413.7 | 170084.4/576472.0/615027.0/626454.0 | 2.0/298.5/422.8/636.1 | 2.2/301.9/650.1/10806.4 |
| grove/r2 | 46855.2/111392.6/121839.5/124455.5 | 47050.8/109658.9/121489.7/124100.9 | 2.0/170.2/311.0/401.6 | 1.7/203.3/302.6/958.5 |
| grove/r3 | 92174.4/184954.6/194093.6/202170.6 | 98533.0/188193.6/196036.5/203834.1 | 2.1/238.5/402.6/1955.5 | 1.6/223.6/284.8/598.2 |
| grove/r4 | 219791.6/409863.1/442063.2/451627.0 | 183438.3/381886.7/423473.2/434891.2 | 2.3/282.1/652.0/5696.2 | 2.1/324.3/529.9/3368.4 |
| snow/r2 | 55717.0/132164.9/141065.7/146250.2 | 60567.9/143361.5/156341.4/163530.2 | 1.8/224.4/344.5/679.2 | 1.4/186.9/298.5/1091.5 |
| snow/r3 | 111915.4/226241.2/234418.0/239928.5 | 116648.5/251313.5/265261.0/275060.9 | 1.9/226.6/308.2/523.0 | 1.7/257.9/410.9/2291.1 |
| snow/r4 | 216213.3/406386.2/437044.6/443727.7 | 220404.3/423727.0/464289.5/476681.3 | 2.3/284.9/504.1/1151.3 | 1.4/286.7/491.9/1166.3 |
| desert/r2 | 33859.9/82810.8/88878.1/91156.9 | 37581.3/70648.2/75984.9/77343.5 | 2.0/193.2/309.8/366.6 | 2.2/192.8/287.8/467.5 |
| desert/r3 | 77426.5/166866.6/176672.3/182147.6 | 63098.8/130538.7/134521.2/137283.7 | 2.3/226.6/317.6/1643.9 | 1.9/207.1/305.2/2012.5 |
| desert/r4 | 121835.5/239659.2/255235.0/261590.8 | 103967.3/193652.5/201680.4/204483.9 | 2.4/255.6/332.7/736.0 | 2.2/241.6/354.5/523.5 |
| coast/r2 | 33866.6/89928.5/94148.8/97190.9 | 35892.3/104406.6/116765.3/120474.1 | 2.4/624.6/933.7/1054.8 | 1.6/534.4/728.5/966.1 |
| coast/r3 | 69970.3/170012.0/186866.1/193569.7 | 84759.8/191259.9/199526.6/205675.1 | 2.2/819.7/1101.8/1807.9 | 2.1/702.8/864.1/1304.7 |
| coast/r4 | 130912.5/269888.6/281542.0/287651.9 | 137245.5/290907.6/305027.7/308340.2 | 2.5/894.5/1296.4/5323.0 | 2.1/772.5/1126.4/10282.8 |
| mountain/r2 | 48476.5/108690.3/117962.7/122067.7 | 56549.0/128446.6/136644.7/138785.0 | 2.0/205.0/310.1/371.1 | 1.7/229.1/331.0/417.5 |
| mountain/r3 | 97428.7/226480.4/243801.5/252606.7 | 134007.7/346831.3/370143.1/379307.7 | 1.8/257.7/383.0/1406.3 | 1.9/193.6/291.9/629.4 |
| mountain/r4 | 165479.0/354566.0/390391.3/398051.0 | 180305.2/349653.2/387254.3/398151.3 | 2.2/344.1/717.1/3702.8 | 1.8/264.0/438.6/7392.4 |
| gated_teleport/r2 | 58832.3/142490.4/171613.2/179921.2 | 58623.8/112178.9/122385.1/125076.9 | 2.2/1238.1/1919.7/7028.5 | 1.8/1017.0/1234.8/5973.5 |
| gated_teleport/r3 | 111398.3/236057.2/272853.6/284911.5 | 107225.6/205083.4/225924.0/242858.4 | 124.1/1530.7/3173.2/16735.7 | 2.8/1163.9/1717.5/12111.2 |
| gated_teleport/r4 | 178581.0/329603.1/364733.5/387435.9 | 167110.2/307634.2/331942.7/359626.3 | 164.5/1391.1/2270.8/28174.5 | 187.8/1290.2/1852.3/24673.4 |
| radius/r2 | 31580.0/83004.1/99187.2/107051.9 | 33960.2/82768.5/89346.7/92943.9 | 4.0/1343.2/3236.9/5415.4 | 2.8/1089.4/2765.6/5862.8 |
| radius/r3 | 47068.2/168379.7/178290.5/182824.8 | 47318.0/178716.0/198134.5/204669.6 | 95.0/1629.1/3623.1/14042.8 | 3.3/1307.4/3111.1/12457.1 |
| radius/r4 | 99475.8/260595.2/274578.1/283279.2 | 100788.7/284430.9/299038.7/308695.7 | 109.4/1544.9/2750.1/26472.3 | 168.6/1330.4/2796.6/24243.9 |

## Converged streaming memory and payload

P50 across the two observations (floor percentile picks the lower when n=2);
consult raw values for both orders. Per-frame/request tails and process high-water
observations remain separate. No persistent biome map/cache was introduced.

| Scene/radius/build | Mesh payload B | Voxel B | Worker cache B | Cached columns | Light cache B | Cached regions | Resident RSS MiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| forest/r2/slice36 | 5,265,536 | 389,120 | 491,520 | 31 | 868,968 | 6 | 37.94 |
| forest/r2/slice37 | 7,222,256 | 385,024 | 487,424 | 31 | 874,800 | 6 | 40.35 |
| forest/r3/slice36 | 8,188,688 | 659,456 | 712,704 | 48 | 1,685,448 | 9 | 45.25 |
| forest/r3/slice37 | 11,676,560 | 671,744 | 712,704 | 48 | 1,714,608 | 9 | 49.54 |
| forest/r4/slice36 | 11,558,768 | 987,136 | 995,328 | 73 | 2,700,216 | 12 | 52.73 |
| forest/r4/slice37 | 16,877,872 | 1,032,192 | 1,032,192 | 73 | 2,781,864 | 12 | 59.86 |
| birch/r2/slice36 | 3,725,456 | 286,720 | 290,816 | 24 | 833,976 | 4 | 37.02 |
| birch/r2/slice37 | 3,588,064 | 290,816 | 311,296 | 24 | 839,808 | 4 | 39.42 |
| birch/r3/slice36 | 8,524,144 | 630,784 | 548,864 | 44 | 1,673,784 | 9 | 44.44 |
| birch/r3/slice37 | 8,407,584 | 634,880 | 552,960 | 44 | 1,679,616 | 9 | 46.42 |
| birch/r4/slice36 | 13,850,680 | 1,101,824 | 901,120 | 68 | 2,805,192 | 16 | 53.86 |
| birch/r4/slice37 | 14,083,304 | 1,105,920 | 905,216 | 68 | 2,811,024 | 16 | 59.20 |
| taiga/r2/slice36 | 3,598,552 | 331,776 | 319,488 | 24 | 775,656 | 4 | 35.21 |
| taiga/r2/slice37 | 4,944,448 | 385,024 | 372,736 | 24 | 851,472 | 4 | 37.11 |
| taiga/r3/slice36 | 7,195,296 | 651,264 | 569,344 | 44 | 1,504,656 | 9 | 42.04 |
| taiga/r3/slice37 | 9,921,064 | 745,472 | 663,552 | 44 | 1,650,456 | 9 | 46.38 |
| taiga/r4/slice36 | 11,206,848 | 1,048,576 | 864,256 | 68 | 2,461,104 | 16 | 50.27 |
| taiga/r4/slice37 | 15,929,016 | 1,191,936 | 1,003,520 | 68 | 2,688,552 | 16 | 56.82 |
| grove/r2/slice36 | 4,043,640 | 286,720 | 274,432 | 24 | 816,480 | 4 | 35.79 |
| grove/r2/slice37 | 4,325,864 | 282,624 | 270,336 | 24 | 810,648 | 4 | 36.73 |
| grove/r3/slice36 | 9,115,488 | 643,072 | 552,960 | 44 | 1,831,248 | 9 | 45.48 |
| grove/r3/slice37 | 9,774,424 | 643,072 | 552,960 | 44 | 1,831,248 | 9 | 46.79 |
| grove/r4/slice36 | 17,925,192 | 1,179,648 | 1,007,616 | 71 | 3,242,592 | 16 | 60.47 |
| grove/r4/slice37 | 18,933,560 | 1,179,648 | 913,408 | 68 | 3,242,592 | 16 | 61.89 |
| snow/r2/slice36 | 8,452,584 | 520,192 | 495,616 | 24 | 1,329,696 | 4 | 43.62 |
| snow/r2/slice37 | 8,419,104 | 520,192 | 495,616 | 24 | 1,335,528 | 4 | 43.61 |
| snow/r3/slice36 | 16,430,000 | 1,077,248 | 958,464 | 44 | 2,618,568 | 9 | 57.13 |
| snow/r3/slice37 | 16,418,592 | 1,081,344 | 962,560 | 44 | 2,624,400 | 9 | 57.20 |
| snow/r4/slice36 | 26,079,680 | 1,712,128 | 1,478,656 | 68 | 4,111,560 | 16 | 73.94 |
| snow/r4/slice37 | 26,028,344 | 1,716,224 | 1,482,752 | 68 | 4,117,392 | 16 | 74.07 |
| coast/r2/slice36 | 4,190,488 | 323,584 | 425,984 | 31 | 729,000 | 6 | 37.21 |
| coast/r2/slice37 | 4,204,376 | 323,584 | 425,984 | 31 | 729,000 | 6 | 37.18 |
| coast/r3/slice36 | 7,980,392 | 610,304 | 655,360 | 48 | 1,423,008 | 9 | 45.26 |
| coast/r3/slice37 | 7,940,712 | 610,304 | 655,360 | 48 | 1,423,008 | 9 | 44.46 |
| coast/r4/slice36 | 12,879,744 | 999,424 | 954,368 | 73 | 2,350,296 | 12 | 53.42 |
| coast/r4/slice37 | 12,910,496 | 999,424 | 954,368 | 73 | 2,350,296 | 12 | 54.37 |
| desert/r2/slice36 | 3,761,664 | 319,488 | 311,296 | 24 | 833,976 | 4 | 41.82 |
| desert/r2/slice37 | 1,840,408 | 311,296 | 311,296 | 24 | 822,312 | 4 | 41.16 |
| desert/r3/slice36 | 7,579,376 | 630,784 | 573,440 | 44 | 1,627,128 | 9 | 57.13 |
| desert/r3/slice37 | 4,034,216 | 606,208 | 552,960 | 44 | 1,580,472 | 9 | 57.20 |
| desert/r4/slice36 | 13,429,448 | 1,052,672 | 884,736 | 68 | 2,676,888 | 16 | 69.93 |
| desert/r4/slice37 | 7,772,320 | 987,136 | 839,680 | 68 | 2,571,912 | 16 | 71.07 |
| mountain/r2/slice36 | 5,570,328 | 413,696 | 389,120 | 24 | 1,038,096 | 4 | 38.95 |
| mountain/r2/slice37 | 5,554,952 | 413,696 | 389,120 | 24 | 1,038,096 | 4 | 38.97 |
| mountain/r3/slice36 | 12,617,744 | 880,640 | 778,240 | 44 | 2,210,328 | 9 | 50.37 |
| mountain/r3/slice37 | 12,549,792 | 876,544 | 774,144 | 44 | 2,204,496 | 9 | 50.30 |
| mountain/r4/slice36 | 21,328,496 | 1,523,712 | 1,273,856 | 68 | 3,749,976 | 16 | 65.48 |
| mountain/r4/slice37 | 21,314,360 | 1,519,616 | 1,269,760 | 68 | 3,744,144 | 16 | 65.86 |

## Real Bevy CPU install/visibility probes

Tuples are **P50/P95/P99/max microseconds**. These exercise actual Bevy mesh
construction, ECS command application and visibility; they do not submit GPU
frames. The genuine rendered gallery/game captures supply separate visual evidence.
The retained-install fixture is identical constructed geometry in both versions,
not a generated biome workload. Its observed ECS visibility P50 at radius1 is
72.5→171.5us and radius3 P95 is1578→3595us; these adverse measurements remain
without attributing identical-fixture differences to biome density. No rendering
algorithm changed, and two execution orders do not establish a causal explanation.

| Scene/radius/metric | v11 | v12 |
| --- | --- | --- |
| birch/r2/bevy_mesh_construct_us | 0.4/1.0/1.1/1.3 | 0.4/0.9/1.1/150.8 |
| birch/r2/request_cpu_visible_us | 35219.5/80410.1/88535.5/91385.3 | 40076.8/82268.9/86368.9/89585.7 |
| birch/r3/bevy_mesh_construct_us | 0.4/0.9/1.1/1.4 | 0.4/0.8/1.0/72.8 |
| birch/r3/request_cpu_visible_us | 68332.0/142706.6/147980.1/151778.2 | 65582.1/142428.0/151892.0/157116.1 |
| birch/r4/bevy_mesh_construct_us | 0.4/0.9/1.1/6.4 | 0.4/1.1/6.3/21.5 |
| birch/r4/request_cpu_visible_us | 117183.5/234415.8/244173.5/246868.5 | 177845.5/380759.5/411608.9/420969.1 |
| coast/r2/bevy_mesh_construct_us | 0.5/0.9/1.2/13.9 | 0.5/0.9/1.1/128.6 |
| coast/r2/request_cpu_visible_us | 33894.6/89941.0/94161.6/97208.9 | 35905.3/104422.3/116779.4/120490.1 |
| coast/r3/bevy_mesh_construct_us | 0.4/0.9/1.1/1.6 | 0.4/0.9/1.1/14.6 |
| coast/r3/request_cpu_visible_us | 69982.9/170031.7/186888.3/193590.4 | 84777.4/191275.8/199547.3/205691.0 |
| coast/r4/bevy_mesh_construct_us | 0.5/1.0/1.2/23.5 | 0.4/0.9/1.1/81.5 |
| coast/r4/request_cpu_visible_us | 130926.7/269907.3/281559.3/287671.4 | 137263.9/290925.5/305043.8/308499.5 |
| desert/r2/bevy_mesh_construct_us | 0.4/0.9/1.2/2.6 | 0.4/0.9/1.1/1.9 |
| desert/r2/request_cpu_visible_us | 33886.3/82822.0/88892.8/91170.8 | 37704.6/70659.2/75996.6/77355.1 |
| desert/r3/bevy_mesh_construct_us | 0.4/0.9/1.0/2.7 | 0.4/0.9/1.1/18.3 |
| desert/r3/request_cpu_visible_us | 77436.3/166886.6/176689.2/182165.5 | 63114.1/130552.4/134535.3/137305.9 |
| desert/r4/bevy_mesh_construct_us | 0.4/0.9/1.2/86.5 | 0.4/0.9/1.0/1.2 |
| desert/r4/request_cpu_visible_us | 121851.5/239675.6/255366.7/261609.6 | 103983.2/193670.1/201704.7/204506.4 |
| forest/r2/bevy_mesh_construct_us | 0.4/0.9/1.1/16.2 | 0.4/0.9/1.2/47.8 |
| forest/r2/request_cpu_visible_us | 36994.2/91743.4/97414.3/100128.9 | 40825.6/103142.9/107520.5/110797.2 |
| forest/r3/bevy_mesh_construct_us | 0.4/0.9/1.1/1.6 | 0.4/0.9/1.1/1.5 |
| forest/r3/request_cpu_visible_us | 69638.3/167264.3/176007.7/181721.8 | 62700.7/151237.6/158228.4/163837.2 |
| forest/r4/bevy_mesh_construct_us | 0.4/1.0/1.4/303.5 | 0.4/0.9/1.4/17.0 |
| forest/r4/request_cpu_visible_us | 121234.3/258107.4/278561.0/284781.5 | 149386.7/474995.7/501758.8/508250.4 |
| gated_teleport/r2/bevy_mesh_construct_us | 0.5/1.0/2.1/133.7 | 0.4/0.9/1.1/1.6 |
| gated_teleport/r2/request_cpu_visible_us | 58850.6/142505.2/171629.4/179935.4 | 58639.8/112196.0/122399.3/125093.3 |
| gated_teleport/r3/bevy_mesh_construct_us | 0.5/1.1/1.5/177.2 | 0.4/0.9/1.1/377.9 |
| gated_teleport/r3/request_cpu_visible_us | 111412.7/236078.4/272873.9/284935.2 | 107247.8/205103.0/225945.6/242882.2 |
| gated_teleport/r4/bevy_mesh_construct_us | 0.4/1.0/1.2/21.5 | 0.4/0.9/1.1/2.5 |
| gated_teleport/r4/request_cpu_visible_us | 178598.5/329624.0/364756.1/387458.1 | 167129.0/307660.2/331965.4/359651.1 |
| grove/r2/bevy_mesh_construct_us | 0.4/0.9/1.1/1.5 | 0.4/1.0/1.2/83.5 |
| grove/r2/request_cpu_visible_us | 46869.5/111404.8/121854.6/124474.5 | 47063.8/109675.7/121506.9/124123.7 |
| grove/r3/bevy_mesh_construct_us | 0.4/0.9/1.1/2.8 | 0.4/0.9/1.0/1.1 |
| grove/r3/request_cpu_visible_us | 92189.3/184980.8/194113.8/202190.9 | 98548.2/188214.0/196096.2/203854.1 |
| grove/r4/bevy_mesh_construct_us | 0.4/1.0/1.3/23.9 | 0.4/1.0/1.3/37.1 |
| grove/r4/request_cpu_visible_us | 219811.2/409887.2/442089.8/451670.6 | 183450.8/381912.0/423494.2/434920.3 |
| meadow/r2/bevy_mesh_construct_us | 0.4/0.9/1.1/1.1 | 0.4/0.9/0.9/1.2 |
| meadow/r2/request_cpu_visible_us | 40949.6/92593.3/99052.1/102574.3 | 44760.8/96633.9/102415.4/106373.7 |
| meadow/r3/bevy_mesh_construct_us | 0.4/1.0/1.2/1.7 | 0.4/0.9/1.8/49.0 |
| meadow/r3/request_cpu_visible_us | 85721.1/172087.6/179316.7/184738.8 | 105739.6/194262.4/201036.3/205628.0 |
| meadow/r4/bevy_mesh_construct_us | 0.4/0.9/1.2/73.8 | 0.4/1.0/1.4/8.9 |
| meadow/r4/request_cpu_visible_us | 146335.3/283131.4/306424.2/313431.2 | 170105.0/576496.0/615071.1/626476.6 |
| mountain/r2/bevy_mesh_construct_us | 0.4/0.9/1.1/14.5 | 0.4/0.9/1.1/1.3 |
| mountain/r2/request_cpu_visible_us | 48492.1/108703.7/117979.2/122111.6 | 56563.2/128459.7/136662.5/138800.7 |
| mountain/r3/bevy_mesh_construct_us | 0.4/0.9/1.0/129.8 | 0.4/0.9/1.2/281.3 |
| mountain/r3/request_cpu_visible_us | 97442.9/226501.8/243819.1/252629.0 | 134026.0/346849.0/370162.8/379330.3 |
| mountain/r4/bevy_mesh_construct_us | 0.4/0.9/1.2/22.4 | 0.4/0.9/1.1/12.2 |
| mountain/r4/request_cpu_visible_us | 165493.1/354590.1/390414.6/398070.2 | 180324.6/349675.0/387283.5/398177.0 |
| radius/r2/bevy_mesh_construct_us | 0.5/1.0/1.2/1.8 | 0.5/0.9/1.0/1.3 |
| radius/r2/request_cpu_visible_us | 31593.1/83016.1/99203.8/107067.6 | 33973.8/82779.3/89360.2/92958.7 |
| radius/r3/bevy_mesh_construct_us | 0.5/1.0/1.2/107.2 | 0.5/1.0/2.4/27.0 |
| radius/r3/request_cpu_visible_us | 47084.5/168401.9/178307.8/182846.0 | 47334.8/178736.5/198148.5/204686.5 |
| radius/r4/bevy_mesh_construct_us | 0.5/1.0/1.2/21.0 | 0.5/1.0/1.2/13.1 |
| radius/r4/request_cpu_visible_us | 99488.8/260619.2/274595.3/283296.2 | 100804.6/284457.5/299057.8/308718.1 |
| retained-install/r1/bevy_mesh_construct_us | 0.2/0.2/0.5/12.8 | 0.3/0.5/0.6/16.8 |
| retained-install/r1/ecs_visibility_update_us | 72.5/166.9/269.2/449.4 | 171.5/384.2/2042.4/6311.0 |
| retained-install/r1/publication_total_us | 75.0/174.8/271.4/452.9 | 176.5/399.9/2047.8/6315.9 |
| retained-install/r2/bevy_mesh_construct_us | 0.2/0.4/0.6/260.4 | 0.3/0.5/0.6/294.4 |
| retained-install/r2/ecs_visibility_update_us | 306.6/584.4/898.6/3240.3 | 359.8/658.3/1214.2/1771.6 |
| retained-install/r2/publication_total_us | 346.8/635.7/941.3/3310.9 | 403.6/700.3/1255.2/1809.3 |
| retained-install/r3/bevy_mesh_construct_us | 0.2/0.3/0.5/340.0 | 0.2/0.3/0.5/293.6 |
| retained-install/r3/ecs_visibility_update_us | 995.7/1578.0/3383.7/6847.2 | 1077.8/3595.0/4550.4/42660.5 |
| retained-install/r3/publication_total_us | 1215.2/1931.7/3552.1/7179.3 | 1297.5/3767.0/4705.2/42843.3 |
| retained-install/r4/bevy_mesh_construct_us | 0.1/0.3/0.4/1356.0 | 0.2/0.3/0.4/585.2 |
| retained-install/r4/ecs_visibility_update_us | 1464.1/2150.6/3160.2/4396.1 | 1581.9/3963.3/14338.6/86594.0 |
| retained-install/r4/publication_total_us | 1779.7/2730.2/3823.3/4690.3 | 1908.1/4551.5/14599.2/101688.0 |
| session/r1/session_switch_us | 2934.1/4434.4/4914.2/14006.3 | 3389.6/9012.9/9651.9/11379.4 |
| session/r2/session_switch_us | 3062.6/5083.6/5256.7/11921.3 | 3360.2/4227.6/4633.5/9080.7 |
| session/r3/session_switch_us | 2394.6/7428.3/8473.0/12763.9 | 3540.9/4647.6/4653.0/5131.4 |
| session/r4/session_switch_us | 2574.8/3800.2/4203.2/4398.1 | 3904.3/5461.9/5987.9/7768.3 |
| snow/r2/bevy_mesh_construct_us | 0.4/0.9/1.1/20.6 | 0.4/0.9/1.1/13.0 |
| snow/r2/request_cpu_visible_us | 55740.1/132209.2/141080.5/146272.7 | 60586.1/143403.0/156359.6/163548.9 |
| snow/r3/bevy_mesh_construct_us | 0.4/0.9/1.0/6.6 | 0.4/0.9/1.1/68.2 |
| snow/r3/request_cpu_visible_us | 111931.1/226261.4/234436.9/239947.8 | 116669.9/251330.3/265304.9/275085.5 |
| snow/r4/bevy_mesh_construct_us | 0.4/1.0/1.2/33.5 | 0.4/0.9/1.2/557.5 |
| snow/r4/request_cpu_visible_us | 216235.1/406408.4/437182.3/443746.8 | 220421.3/423746.2/464307.8/476699.7 |
| stationary/r2/bevy_mesh_construct_us | 0.4/0.9/1.3/2.0 | 0.4/1.1/2.1/19.1 |
| stationary/r2/request_cpu_visible_us | 36702.3/101430.5/114024.0/118048.5 | 48562.0/97172.6/102420.0/106227.6 |
| stationary/r3/bevy_mesh_construct_us | 0.5/1.0/1.1/31.1 | 0.5/1.0/9.2/134.5 |
| stationary/r3/request_cpu_visible_us | 77067.4/173625.9/182740.6/188423.0 | 97205.6/195547.7/203368.2/206413.7 |
| stationary/r4/bevy_mesh_construct_us | 0.5/1.2/2.2/83.3 | 0.5/1.1/6.0/25.0 |
| stationary/r4/request_cpu_visible_us | 152085.2/387142.9/436321.2/443247.5 | 169531.7/377841.4/417672.0/425483.3 |
| taiga/r2/bevy_mesh_construct_us | 0.4/0.9/1.2/29.0 | 0.4/0.9/1.2/19.8 |
| taiga/r2/request_cpu_visible_us | 33319.7/91267.6/99788.5/102383.4 | 45550.0/86608.5/90700.9/93922.9 |
| taiga/r3/bevy_mesh_construct_us | 0.4/0.9/1.1/11.5 | 0.4/0.9/1.0/23.5 |
| taiga/r3/request_cpu_visible_us | 64980.6/139709.4/144998.0/148232.0 | 71114.5/155163.0/161788.3/168302.8 |
| taiga/r4/bevy_mesh_construct_us | 0.4/0.9/2.1/165.1 | 0.4/0.9/1.1/59.0 |
| taiga/r4/request_cpu_visible_us | 103976.1/218084.3/238022.0/244606.8 | 134138.8/256471.0/267767.9/273534.0 |

## Whole-process observations

Each executable invocation is observed with Linux wait4, including worker threads.
Streaming/session/install are separate process invocations; high-water RSS cannot
be added across them. Forward/reverse are shown separately rather than treating
two process runs as a large statistical sample. These do not include the compiler
or a graphical Bevy client/GPU.

| Process/order | Wall s | User+system CPU s | Max RSS MiB |
| --- | --- | --- | --- |
| core/forward/slice36-bench/core | 5.811 | 5.770 | 13.12 |
| core/forward/slice36-lifecycle/core | 0.216 | 0.213 | 13.24 |
| core/forward/slice37-bench/core | 6.368 | 6.339 | 13.24 |
| core/forward/slice37-lifecycle/core | 0.280 | 0.278 | 13.24 |
| core/reverse/slice37-bench/core | 6.989 | 6.954 | 12.50 |
| core/reverse/slice37-lifecycle/core | 0.315 | 0.308 | 12.61 |
| core/reverse/slice36-bench/core | 6.201 | 6.156 | 12.61 |
| core/reverse/slice36-lifecycle/core | 0.209 | 0.207 | 12.61 |
| stream/forward/slice36-bench/slice32_streaming_measurements | 20.694 | 12.218 | 73.70 |
| stream/forward/slice36-bench/slice32_session_measurements | 0.220 | 0.176 | 71.15 |
| stream/forward/slice36-bench/slice33_install_measurements | 1.111 | 0.995 | 71.15 |
| stream/forward/slice37-bench/slice32_streaming_measurements | 22.181 | 13.829 | 73.94 |
| stream/forward/slice37-bench/slice32_session_measurements | 0.303 | 0.256 | 71.18 |
| stream/forward/slice37-bench/slice33_install_measurements | 1.384 | 1.160 | 71.18 |
| stream/reverse/slice37-bench/slice32_streaming_measurements | 20.562 | 11.901 | 74.10 |
| stream/reverse/slice37-bench/slice32_session_measurements | 0.289 | 0.265 | 71.05 |
| stream/reverse/slice37-bench/slice33_install_measurements | 1.404 | 1.166 | 71.08 |
| stream/reverse/slice36-bench/slice32_streaming_measurements | 20.207 | 11.514 | 74.02 |
| stream/reverse/slice36-bench/slice32_session_measurements | 0.298 | 0.238 | 71.08 |
| stream/reverse/slice36-bench/slice33_install_measurements | 1.030 | 0.912 | 71.08 |

## Lifecycle persistence tails

P50/P95/P99/max microseconds,48 real trials per build across both orders. Each
trial removes8 actual generated plants, saves explicit Air, reloads and regenerates.

| Metric | v11 | v12 |
| --- | --- | --- |
| break_us | 1.4/3.0/14.1/16.7 | 1.4/2.9/6.1/15.4 |
| regeneration_us | 2445.1/2914.2/3944.0/4926.1 | 3444.8/4328.9/5319.6/6784.2 |
| reload_us | 2440.5/2925.1/3172.9/3661.2 | 3421.9/4253.9/7424.9/9418.9 |
| save_us | 108.4/304.3/367.0/595.7 | 118.1/253.5/292.3/520.3 |

## Regression investigation and retained adverse observations

The initial final-quality pass found redundant coast geography and tree-footprint
climate work. Its complete two-order evidence is retained under
`raw/before-optimization-*`. The optimized implementation uses an exact marine
predicate outside the protected spawn shore and dry-support-only crown checks;
full actual hydrology/geometry and sampled block/geometry/light CRCs verify output.
The following before/after candidate timings pool the two execution orders.

| Candidate scene/metric | Before P50/P95/P99/max | Final P50/P95/P99/max |
| --- | --- | --- |
| beach/raw_us | 3281.1/4639.8/5872.4/8998.7 | 1421.1/2432.7/5338.3/18654.0 |
| beach/classification_us | 2483.7/3681.1/4790.4/8212.4 | 651.6/1078.7/2098.6/17632.7 |
| grove-dense/raw_us | 1871.9/2521.6/2918.4/4896.8 | 1513.9/2149.3/3785.3/5296.4 |
| grove-dense/tree_plan_us | 797.2/1137.7/1369.3/1655.8 | 440.5/650.5/1092.4/1512.7 |
| grove/tree_plan_us | 251.0/428.6/582.8/899.9 | 223.2/356.4/515.3/1098.8 |

Largest final P50 increases are listed even where additional vegetation is the
intended cause. They do not establish a universal FPS regression threshold.

| Scene/metric | v11 P50 us | v12 P50 us | Change |
| --- | --- | --- | --- |
| beach/raw_us | 692.6 | 1421.1 | +105.2% |
| birch/worker_us | 569.7 | 715.2 | +25.5% |
| taiga/worker_us | 701.0 | 863.4 | +23.2% |
| river/worker_us | 566.3 | 676.5 | +19.5% |
| grove-dense/raw_us | 1303.3 | 1513.9 | +16.2% |
| grove-dense/worker_us | 1177.7 | 1350.4 | +14.7% |
| birch/raw_us | 481.2 | 551.0 | +14.5% |
| grove/raw_us | 1127.0 | 1276.1 | +13.2% |


All per-frame, worker stage, queue, installation, allocation, resource and tail
observations—including maxima and slower individual scenes—remain under the
four `raw/*-final-*` directories. Whole-process peaks are cloud observations, not
a game RAM requirement. Native Gentoo/Void/Windows and Radeon comparisons using
identical settings remain pending; the procedure is in README.md. No rendering,
streaming architecture or runtime-distance overhaul is hidden in this slice.
