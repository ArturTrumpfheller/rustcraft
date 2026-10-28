# Slice23 native acceptance — Issue #40

Run on Void Linux **glibc** using the normal X11/Wayland session and AMD driver.
Repeat Windows10/11 GUI checks with the same constructions. Do not use production
saves for initial testing. No renderer override is required.

```fish
git fetch origin
git switch feat/slice-23-fluid-behavior-v2
git pull --ff-only
set slice23_test_dir (mktemp -d /tmp/rustcraft-slice23.XXXXXX)
echo $slice23_test_dir
env RUSTCRAFT_DATA_DIR="$slice23_test_dir" cargo run --release --locked -p rustcraft
# Create a world with seed42. Record the test path for restart.
```

Open E to obtain Stone, torches and both buckets from the existing item palette.
Enable F3 to read coordinates and water pending/processed/CPU. Build a flat stone
pad at least17×17 with a second solid layer underneath to catch drainage. Leave
three clear blocks overhead. Let S=(X,Y,Z) denote the water cell above its center;
the upper pad is at Y-1. Observe each scene until F3 pending reaches zero.

1. **Flat spreading:** place one source at S. With no holes, water spreads all
   four ways to seven horizontal levels. After settling, pending/processed must
   stay zero. Collect it: all non-source water must eventually recede.
2. **Shortest downhill path:** remove pad blocks (X-3,Y-1,Z) and (X+2,Y-1,Z),
   then replace the source. Only the right branch should carry water from S.
   Move the right hole to X+3: both equally short left/right branches should flow.
   Add a wall between S and a hole: count the route around the corner, rather
   than the straight-line distance. A fully enclosed hole must not attract flow.
3. **New drainage:** restore the pad, settle flat flow, then open (X+4,Y-1,Z).
   The already-running source should choose the right route and other branches
   should recede. No bucket replacement should be needed to activate it.
4. **Blocked drainage:** fill that hole with Stone. Normal spreading returns.
   Repeat opening/closing it several times; the queue must settle each time.
   Repeat with a dry new hole on the opposite side of an already preferred route.
5. **One-block shore:** build a three-block-wide water channel on the pad, ending
   at a one-block-high stone bank with clear space above. Place a source at the
   far end so level1–7 flow reaches the bank. Stand in the flowing cell against
   the bank, hold forward and Jump: rise and move onto the bank continuously.
   Repeat near the source, in thinner water, across a corner and at a waterfall
   foot. Without Jump there must be no automatic step. A two-block-high wall or
   a low roof must block climbing without clipping, repeated boosts or teleport.
6. **Swimming/land:** hold Jump in a source column; rise normally. Swim away from
   the bank and release Jump: descend normally. On land verify walking, Ctrl
   sprint, Shift sneak, normal/continuous sprint jumping, camera and view bobbing.
   Releasing/repressing Jump in the air must not give a second jump.
7. **Chunk seams:** rebuild the source/hole scene across X or Z=16n and a corner
   at (16n,16m). Open/close drainage while flowing. Leave render distance and
   return. Loading must resume routes without loading distant terrain for water.
8. **Existing interactions:** place/collect sources, check Empty/Water Bucket
   counts, mine behind water, place Stone into water, wash floor/wall torches.
   Check one item per torch, pickup/full-inventory behavior and lost torch light
   at night. Pause/Options/focus freeze simulation; E leaves it running.
9. **Persistence:** save a modified drainage/shore scene with F5, wait for SAVED,
   close normally and restart with the same test directory. Source, hole, flow
   states, inventory and dropped items must survive and reconcile. Repeat saving
   during active flow, after collecting the last source and through autosave.

For performance use identical release builds of merged Slice22
`aed57cb42e4c7f949aa8a4812ab07cbeb89c090b` and this PR, separate copied worlds,
1920×1080, radius2 and VSync off. Record GPU/driver/backend, build SHA, X11 or
Wayland, fluid update CPU, processed/pending cells, frame P50/P95/P99 and RSS for
dry land, a settled pool, several sources, a large waterfall, obstacles, holes,
repeated drainage edits and chunk seams. The owner's ~240-FPS report is historical
context; cloud software-rendered figures do not predict RX9060 XT performance.

Native Void, physical Radeon performance, Wayland and Windows GUI acceptance
remain unchecked until these steps are performed on those systems.
