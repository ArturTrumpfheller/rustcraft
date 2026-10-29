# Slice 26 — Vertical world expansion

Project-authored Rust/documentation, MIT OR Apache-2.0. No new dependencies,
assets, terrain tuning, cave algorithm, biomes or movement abilities.

## Identity and compatibility

| Identity | Horizontal blocks | Vertical blocks | Vertical chunks |
| --- | --- | --- | --- |
| Legacy snapshots / generator1 | [-32,32) | [-16,32) | [-1,2) |
| Finite generator2 | [-64,64) | [-16,32) | [-1,2) |
| Regional generators3–5 | [-32768,32768) | [-16,32) | [-1,2) |
| Frozen regional generator6 | [-32768,32768) | [-64,192) | [-4,12) |
| New regional generator7 | [-32768,32768) | [-64,192) | [-4,12) |

`bounds::WorldHeight::for_generator` owns this contract. `World::bounds`,
generation, collision readiness, edit validation, light dependencies, source
snapshots, eviction and recovery derive their ranges from the identity. Minimum
is inclusive, maximum exclusive: Y191 is editable; Y192 is known outside Air.
Unknown generators are rejected by generation and persistence before loading.
New catalogs select7; existing identities and their heights never change.

Slice27 retains these bounds/storage budgets. V7 adds bounded cave plans below
-16, four Stone foundation layers[-64,-60), protected spawn and ordinary five-block
roofs. Only chunks with real mixed cave geometry allocate voxel vectors. The
existing light classifier includes those chunks automatically, with no full-height
mode switch. The frozen v6 algorithm and measurements below remain historical;
see [current v7 algorithm](WORLDGEN.md) and [v7 evidence](performance/slice27/README.md).

Manifest5's generator field already supplies all required information, so v6
uses the same exact64-byte envelope. Regional format1 still uses signed i32
coordinates, generator, nonce, seed, sorted14-byte records and CRC. Player mode5,
world fingerprints, inventory IDs, journal replay, catalog, backups and locks
keep their protocols. A Slice25 client rejects generator6; use closed copies
when comparing clients. Historical byte/generator golden fixtures stay intact.

## Generator6 output contract

Within Y[-16,32), every v6 cell equals v5 for the same seed/X/Z, including five
biomes, material transitions, periodic caves, tree candidates and crown limits.
The new Y[-64,-16) interval is Stone. Y[32,192) is Air. Outside bounds is Air.
There is no hollow seam at Y=-16. Surface geography, climate noise and tree/cave
appearance are frozen; no new caves, water or mountains are generated.

Column planning is shared with the version-gated v5 stages. Point and chunk
sampling agree, including signed/distant coordinates and arbitrary request
order. The new uniform intervals are identified before voxel sampling; one
16-chunk descriptor batch still evaluates only the historical three voxel slabs.
Future algorithms can replace these proofs behind a new generator identity.

Independent v6 column CRC fixtures use ascending chunk Y, local Y/Z/X and stable
u16 little-endian material IDs:

| Seed | Chunk X/Z | CRC32 |
| --- | --- | --- |
| 0 | 0,0 | 427164329 |
| 42 | -1,1 | 4240216072 |
| 2026 | -2048,2047 | 4166722754 |
| u64::MAX | 173,-261 | 988062637 |

## Readiness, storage and worker ownership

A missing in-bounds descriptor is **pending**, which blocks collision/targeting
and refuses edits. A present uniform Air descriptor is **ready**, allocates no
voxel vector and is traversable. A uniform Stone descriptor similarly allocates
no vector. Mixed blocks/fluids use a4096-cell vector, materialized by the first
change. Removing the last non-Air cell releases that vector while retaining the
ready Air descriptor. Generated uniform slabs compact off-thread; gameplay does
not scan a4096-cell slab on every edit. Chunk equality compares logical values.

Edits remain separate authoritative regional maps, including explicit Air.
An edited empty descriptor and an unedited empty descriptor have the same
collision behavior, with distinct persisted overlay authority. Eviction removes
all descriptors and fluid route/event entries for that horizontal column;
regeneration reapplies the complete captured map. Accepted worker region payloads
only install absent maps, never replace newer live edits.

The existing single asynchronous worker, nearest/forward scheduling, radius1–3,
connected dirty publication, local revision rejection, cache pruning, failures
and retry remain. A column job publishes at most16 compact descriptors atomically
in one Update. It does not run16 separate generation jobs. Known empty upper
chunks receive no greedy mesh, light sample array, entity or GPU allocation.
Enclosed uniform Stone likewise needs no mesh/sample array. The bottom/exposed
boundary still meshes correctly. Removing the last block publishes an empty
replacement to remove retained geometry. All vertical geometry/AO neighbors and
full-height local light tokens participate in invalidation, including empty
space newly materialized by an edit.

Worst-case ready descriptors:144/400/784 at radius1/2/3. These are readiness
metadata, not full voxel allocations. Mixed-voxel worst case at radius3 is
784×4096 =3,211,264 block bytes. Pristine worker cache stays within81 horizontal
columns; normal v6 surface allocations equal the compact v5 surface allocation.
The larger theoretical mixed-volume bounds are demand-driven, never preallocated.
Meshes, regional maps, allocator overhead and scratch are separate costs.

## Lighting

Historical worlds retain their48-plane solver/cache behavior, including the
water-only path. V6 first classifies the complete source chunk descriptors.
It omits bottom planes only when **every** source column is uniformly opaque,
and upper planes only when **every** source column is known Air. A15-plane
upper margin retains all possible artificial-light propagation. Pending data is
opaque during classification and solving, never evidence of open sky.

The remaining contiguous span uses the established exact direct-sky, lateral
skylight and independent torch flood. Queries in omitted upper planes return
sky15/block0; omitted opaque bottom planes return0. Exact18³ light caches are
kept only for chunks requiring geometry. A completely empty target batch skips
the light solve. Towers, mined shafts, water and emitters anywhere in the new
height range automatically expand the active span. A full256-plane reference
regression compares both channels cell-for-cell with the trimmed solver, with
high shadows, deep torches, a shaft, water and a pending source column.

The maximum dense domain remains bounded by radius:142×142×256 cells. Ordinary
surface jobs solve a much smaller span. Large edited towers/shafts can require
that larger solve; this is a measured limitation, not an approximation of light.
Materializing water in an upper chunk without cached samples may need one full
light job; subsequent water-only jobs reuse exact samples when available. An
empty chunk need not keep a5832-byte cache merely for possible future water.
Day/night still changes material uniforms without light jobs. Mesh transforms,
floating origin, transparent-water rendering and AO retain their existing paths.

## Regional and recovery budgets

V6 keeps the historical **196,608 records per64×64 region**. It may place them
anywhere in its256-block height; a fully edited extended region is deliberately
not representable. Each region file remains at most2,752,576 bytes. Eight full
regions plus envelope/root/player fit the existing32 MiB journal. No capacity or
journal allocation is multiplied by the theoretical height ratio.

New-coordinate edits at capacity fail with `EditError::RegionCapacity` before
voxel/inventory/drop changes. Replacements of existing records remain valid.
Support removal validates the complete staged batch; fluid ticks account for
all earlier accepted candidates before committing. Dirty-region backpressure
and bounded fluid requests retain unsaved data. Saving does not free the total
record budget: it is a complete overlay limit, separate from the eight dirty-map
limit. There is no truncation, automatic record deletion, migration or recovery
journal overwrite. A future larger edit budget needs its own storage/recovery
design. Strict decode/read-size checks apply before large allocations.

## Player and fluids

Movement constants and collision dimensions are unchanged. Spawn searches only
fully ready vertical columns; pending slabs cannot masquerade as supported
spawn ground. Void recovery is one chunk below the selected floor: historical
Y=-32, v6 Y=-80. Reset waits at the selected ceiling while spawn is pending.
The upper world limit creates no invisible collision plane.

Water remains event-driven, budget256 per5-Hz tick, with deduplicated requests,
closed pending dependencies and chunk-indexed route caches. The maximum possible
request count is derived from49 extended columns; no full queue is allocated on
creation. Routes allocate only on demand. Water never loads chunks. Floor checks
stop propagation at Y=-64; removal recedes through all16 chunks. Source states,
buckets, washed torches, drops and saved fluid IDs are unchanged.

## Validation and evidence

[Measurements, test results and limitations](performance/slice26/README.md).
Historical v1–v5 goldens are retained. New tests cover bounds, independent v6
CRC fixtures, point/chunk/order agreement, sparse allocation/readiness, eviction,
last-block removal, revision-safe publication, rendered asset lifetime, signed
vertical mesh seams, lighting/reference equality, deep/high torches, full-height
water/recession, pending collision, jumping/targeting and journal recovery at
all coordinated commit boundaries. Native Void/Windows acceptance remains manual.

## Void Linux glibc / Fish acceptance

Use normal Mesa/Radeon drivers and the project's existing Void build packages.
Test X11 and Wayland separately. Keep complete closed save backups. Commands
below create isolated roots; replace the source placeholder only for a **copy**
of an already closed Slice25 data root.

```fish
git fetch origin
git switch feat/slice-26-vertical-world-expansion
git pull --ff-only
set new_root (mktemp -d /tmp/rustcraft-slice26-new.XXXXXX)
set old_root (mktemp -d /tmp/rustcraft-slice26-v5-copy.XXXXXX)
# Copy the COMPLETE closed Slice25 data root, including regions/sidecars/backups:
cp -a /path/to/closed-slice25-data-root/. "$old_root/"
env RUSTCRAFT_DATA_DIR="$new_root" RUSTCRAFT_WORLD_SEED=42 RUSTCRAFT_LOAD_RADIUS=2 cargo run --release --locked -p rustcraft
# After normal close, reopen the same isolated root:
env RUSTCRAFT_DATA_DIR="$new_root" RUSTCRAFT_LOAD_RADIUS=2 cargo run --release --locked -p rustcraft
# Inspect the copied historical world separately:
env RUSTCRAFT_DATA_DIR="$old_root" cargo run --release --locked -p rustcraft
```

1. Create a new named world; F3/logs must show generator6 and all five original
   biome identities. The copied world must still show generator5 with its old
   structures/inventory, Y[-16,32) bounds and unchanged terrain.
2. On the new world use F3 coordinates to mine below-16, crossing-32/-48 to-64.
   Y=-64 is editable; Y=-65 and Y192 are outside. Verify no hollow old-floor seam.
3. Jump/build a narrow tower through Y32/64/128/176 up to191. Place/remove its
   last isolated upper block. Empty upper sky must remain traversable. Verify
   no phantom floor at32 or192 and no missing/duplicate boundary faces.
4. Walk/jump/sprint/sneak on generated ground and tower landings. Mine downward
   near a streaming frontier; pending terrain must remain a barrier. R must wait
   for and return to safe edited origin ground. Falling below-80 must recover.
5. Build/seal/open roofs at positive/negative heights. Place supported floor and
   wall torches deep below-16 and high above31. Look for light/AO seams across
   each16-block boundary; day/night must preserve torch brightness.
6. Obtain buckets from E, pour water high above31 into a shaft spanning several
   chunks. Verify downward flow, swimming across a boundary, torch washing and
   source-removal recession. Repeat near-64; flow must stop at the lower limit.
7. Travel beyond radius and return. Tall structures, mined Air and water must
   regenerate correctly, including transparent meshes. Repeat at signed X/Z
   chunk/region boundaries and after changing radius1→3→1.
8. F5, wait Saved, close normally, reopen the same root. Check high/deep edits,
   explicit Air, torches, fluid states, inventory/cursor/selection and world clock.
   Test autosave and return-to-title/another-world round trips as well.
9. At1920×1080 record backend/GPU/driver, radius, VSync, FPS and frameP50/P95/P99
   for surface, tower, mined area and flow. Compare copied v5 under matched
   surface conditions. Logs report ready/nonempty counts, actual voxel bytes,
   worker percentiles, queues, cache, mesh bytes and install/update maxima.
10. Use only another disposable complete copy for interrupted-save experiments.
    Never remove journals/backups or kill the owner's only world to test recovery.

Windows10/11 uses the same gameplay checklist and a distinct copied data root;
set `RUSTCRAFT_DATA_DIR` in PowerShell. CI tests compilation/core/ECS behavior on
Ubuntu and Windows; it cannot establish native driver/compositor/input behavior.
