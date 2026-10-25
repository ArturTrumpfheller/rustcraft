# Slice 22 — water foundation

RustCraft's first water implementation keeps simulation in `voxel_core` and
presentation in the Bevy client. No dependency, generator, terrain atlas, opaque
vertex layout, world envelope, player envelope or journal protocol changes.
The original code, procedural 16×16 ripple tile and screenshots are
MIT OR Apache-2.0. No Minecraft code, artwork or texture pack is included.

## State and behavior

`Block` remains **one byte**. Existing block IDs 0–10 retain their exact meanings.
Explicit additional stored IDs are **11 source**, **12–18 horizontal levels 1–7**,
and **19 falling**. `fluid::Fluid` validates its small state registry and exposes
`FluidKind::Water`; the packed block variants keep dry chunk storage unchanged.
A future lava kind needs its own explicit IDs, rules and material, rather than
reinterpreting these values. No unused reaction, viscosity or fluid framework.
Item IDs 1–6 are unchanged; **Water Source = item 7**.

Only explicit placement/import makes sources. A source remains until removed or
replaced, supplies downward water without consumption and never converts nearby
flow into new sources. Downward supply has priority; a supported water cell
supplies the strongest horizontal candidate plus one, stopping after level 7.
A falling column hitting support starts a fresh seven-block horizontal reach at
its foot. Strength decays along detours around obstacles. No slope-search routing
or infinite-source mechanic is implemented.

Each queued cell computes from the same pre-tick state. The batch commits after
all candidates have been examined. Edits wake the cell and its six dependencies;
actual flow changes wake their neighbors. Removing the last source causes
unsupported levels to weaken and disappear. There are no upward dependency
edges: downward columns and bounded horizontal levels settle without continual
updates. Multiple sources combine by minimum level; queue budget and activation
order converge to the same fixed point in the tested fixtures.

## Scheduling, bounds and ownership

`World` owns transient `FluidUpdates`: FIFO positions plus hash-set deduplication.
Hash iteration never chooses processing order. The queue reserves/grows flat
buffers; it does not allocate a linked node per update. Overlay record allocations
remain necessary for persistent edits. The supported resident maximum is
147×4096 pending positions; unknown cells are never enqueued. Eviction removes
obsolete requests. No fluid operation requests a chunk or creates a worker.

The client runs **5 fluid ticks/s**, sharing the existing 60-Hz physics gate, with
**256 processed cells/tick** by default. `FluidConfig::budget` and the client's
`FluidRuntime.config` are explicit tuning points; this slice adds no settings UI.
The adapter retains a reusable change vector and coalesces mesh dirtiness per
batch. Pause, Options, focus loss and final saving stop simulation; inventory
continues it. A paused phase resets, so resume cannot accumulate fluid catch-up.
F3 reports resident water cells, pending requests, last tick processed and CPU ms.
The count reads per-chunk counters, never scans voxel contents for diagnostics.

Worker mesh snapshots deliberately omit transient fluid scheduling. One existing
terrain worker remains the only terrain/mesh job in flight. For a water/Air-only
streamed batch with valid applied light, it reuses exact chunk light samples and
builds **only water geometry**: no light flood or opaque greedy rebuild. Solid,
AO or torch changes retain the normal coherent lighting/meshing path. Mixed
batches use that conservative path. Legacy finite worlds use their existing
bounded light/mesh worker for changes. Animation changes one retained water
uniform, never mesh data. Dry frames do not update water material uniforms.

## Streaming and persistence

Loaded neighboring chunks exchange flow normally. Unknown/pending neighbors form
closed boundaries, and an existing boundary flow cell is retained until all its
local dependencies are known. This avoids retracting saved water using missing
source data. Installing a chunk wakes its water plus touching neighboring water;
dry chunks skip their contents in constant time. Lifecycle scanning is limited to
newly installed wet chunks/touching wet faces, never performed every frame.

Eviction removes voxel/GPU caches and transient requests, while regional overlays
remain authoritative. Loading restores every source and flow level, then locally
reconciles it. A frontier with unavailable dependencies can remain frozen until
those chunks return. No global pending-boundary map, forced loading or infinite
exploration history is retained.

Water changes use the existing atomic in-memory batch, regional COW overlays,
revision tokens and coordinated world/inventory redo journal. Regions stay bounded
at 196,608 records; the existing eight-dirty-region backpressure and pressure-save
rules apply. A batch preflights newly dirty regions together. Save backpressure
retains bounded requests for retry rather than dropping state. Saves capture the
actual current flow, including a partially settled body, and replay it safely.

Existing Slice21 saves load unchanged, including old player v1/v2 files and
world v1–v4 modes. Generators 1–3 are untouched. New readers accept additive water
IDs; **older clients reject saves containing these IDs**. Use complete closed
world-directory copies for compatibility testing. Existing inventories are never
refilled or overwritten; new worlds receive 32 Water Sources in hotbar slot 7.

## Rendering and interaction

Water uses a dedicated `WaterMesh` and shared blended `WaterSurface`, separate
from opaque terrain and its greedy vertex layout. Each resident chunk has at
most one additional water entity/mesh. Handles are retained across replacements;
empty meshes, eviction and session teardown release them. The shared original
16×16 blue ripple tile scrolls in quantized shader UV steps. No texture uploads,
remeshing, reflections, refraction or GPU readback are needed for animation.

Surface corner heights are sampled in world coordinates from all touching water
cells, including diagonals. Sources receive greater weight; flow progressively
lowers the surface. Falling/vertically covered water fills the cell height.
Neighboring chunks compute identical shared edge heights. Water/water faces and
opaque-adjacent faces are hidden; solid terrain retains its visible faces beneath
transparent water. Sky/block light remain independent, transmitted exactly like
Air in this first slice. The existing day/night uniform scales only sky light.

Transparency uses Bevy's standard mesh-level alpha sorting, alpha 0.65 and
backface drawing for underwater views. **There is no per-face sorting or
order-independent transparency**: intersecting waterfalls and stacked surfaces
can blend imperfectly. No realistic optical depth or water absorption yet.

Sources are ray-targetable; flowing water is transparent to targeting so players
can mine a channel through it. Left-click a source to collect it. Right-click a
source with a solid block to replace its occupied cell. Otherwise placement uses
the clicked solid face, and may displace flowing water in its adjacent cell.
Existing collision, displayed-body protection, torch support and item transaction
rules remain. Water does not wash away torches or other non-cube decorations;
they act as small fluid barriers for this foundational slice. Buckets/drops are
not implemented. The temporary held source/icon is a blue build cube.

## Movement

Water is non-collidable. Actual body overlap with the sloping surface selects
fixed-step water drag/control: 2 blocks/s horizontal target (1 while crouched),
slower descent capped at 2 blocks/s, and held Jump accelerates upward to 2.5.
Leaving water immediately selects the original land/air controller constants.
Body detection uses bounded samples of overlapped cells; very thin contact at
a sloping shore is approximate. The eye uses the mesh's triangle heights to select a subtle retained blue overlay;
HUD/inventory remain readable. There is no oxygen, drowning, health, entity
current or automatic surface jump.

## Validation and reproduction

See [measurements and exact limitations](performance/slice22/README.md) for the
Slice21 comparison, active fill/steady CPU work, mesh bytes and software renderer
frame/RSS observations. The standalone gallery settles fixtures before screenshots and steady runs;
`--active` separately exercises expansion/recession while measuring rendered frames.

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --release --locked -p rustcraft
cargo run --release --locked -p voxel_core --example water_bench
cargo build --locked -p rustcraft --example water_gallery
python tools/capture_water.py --output work/water-captures
```

Gallery cases: `dry`, `pool`, `flat`, `waterfall`, `channel`, `night`, `several`,
`boundary`; e.g. `target/debug/examples/water_gallery waterfall 2 --time=6000`.
No saves or gameplay commands are exposed by the gallery.

## Void Linux manual acceptance (Fish)

Use normal Void glibc X11/Wayland drivers; no forced backend. Windows10/11 uses
the same game logic and the existing CI matrix; native GUI acceptance is separate.

```fish
git fetch origin
git switch feat/slice-22-water
set water_test_dir (mktemp -d /tmp/rustcraft-water.XXXXXX)
echo $water_test_dir
env RUSTCRAFT_DATA_DIR="$water_test_dir" cargo run --release --locked -p rustcraft
# Create a new world, seed 42. Keep the same directory for the restart.
```

1. Select **7**, place a source on flat ground; observe seven-block reach and
   lower distant edges. Build an obstacle, dig a channel, then remove support.
   Verify a waterfall reaches support and settles; F3 PENDING must become zero.
2. Place two nearby sources. Remove one, then the other. Remaining supply must
   persist; unsupported water must completely recede. Place stone into water.
   Check normal stone/torch mining, support removal and inventory counts.
3. Repeat at X/Z ±16 and ±64 seams, including a corner. Walk out of render
   distance and back while flowing, remove the source, save and restart. Check
   that removed sources never return and no permanent boundary seams remain.
4. Enter a pool, walk/swim with held Jump, crouch, fall into a waterfall, leave
   water and compare original walk/sprint/jump behavior. Lower eyes beneath the
   surface; check subtle tint, readable UI, no collidable water or stuck posture.
5. During active spreading open **E** (flow continues), **Escape/Pause**, Options
   and switch focus (flow freezes). Resume after several seconds: no burst.
6. Test F5, autosave and normal-close/restart during flow. Compare sources,
   resulting flow, inventory and night torch light. Use complete disposable
   copies to test old worlds and save failures; retain the whole directory.
7. Repeat travel, edits, resize and world A/B switches. Mesh/material counts and
   memory should remain bounded. Compare day/night, underwater, multiple layers
   and waterfalls from both sides for the documented sorting limitation.
8. With VSync off, record resolution, render radius, build, GPU/driver/backend,
   FPS/frame P50/P95/P99 and RSS for dry, pool, waterfall, multiple active sources
   and seam flow. Compare to the accepted native Slice21 baseline. Repeat X11,
   Wayland and Windows where available; software Vulkan cannot predict RX9060 XT.

## Captured production-shader fixtures

Stable source/pool:

![Stationary source](images/slice22/slice22-pool.png)

Horizontal flow:

![Flat flow](images/slice22/slice22-flat.png)

Waterfall:

![Waterfall](images/slice22/slice22-waterfall.png)

Excavated channel:

![Channel](images/slice22/slice22-channel.png)

Night and independent torch light:

![Night water](images/slice22/slice22-night.png)
