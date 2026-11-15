# Executable Horizon cave-stage reference

`python tools/horizon_reference/prepare.py /absolute/new/scratch-directory`
checks out immutable Horizon and public functional registry revisions. It copies
only the three necessary original MIT crates into a minimal temporary workspace.
Review the upstream LICENSE and THIRD_PARTY_NOTICES.md retained there. Its first
Cargo build prunes the upstream lockfile to this workspace; subsequent builds
use `--locked`. No game JAR, decompiled source, texture or asset is needed.
The functional JSON definitions remain external scratch inputs: they are not
vendored here or claimed to be Horizon's MIT work.

Run the printed build command, then:

```sh
./target/release/examples/rustcraft_cave_reference METADATA/data/minecraft/worldgen PLAN.csv OUTPUT
./target/release/examples/rustcraft_noise_reference METADATA/data/minecraft/worldgen/noise > noise.csv
```

The cave exporter executes the original density graph, interpolation, positional
noise, noise-based aquifer and all three ordered Overworld carvers. All 56 Overworld
biomes in the pinned registry select the same cave/cave-extra/canyon set. Four
logical states classify Stone/Air/Water/Lava without importing block assets.
Carver application runs downwards and skips only uncarvable bedrock (absent from
this noise stage). No surface material rules, bedrock placement, structures,
features, decorations, block entities, ticks or lighting are exported. Fresh
world blend defaults and zero beardifier apply. This is **Horizon cave-stage
geometry**, not a complete playable Horizon world or a rerun of the JVM oracle.

RCV1 uses the common analyzer's format at actual reference bounds [-64,320).
Water/lava remain distinct and obstruct dry floor traversal. Natural surface is
the highest final Stone in each column. Tests of the numeric primitives and the
source/binary/input provenance accompany the measured report in
[the Horizon follow-up](../../docs/performance/slice39/horizon/README.md).
The upstream parity harness still needs official Minecraft libraries/resources;
its published results are not represented as locally rerun validation.

`rustcraft_terrain_reference METADATA/data/minecraft/worldgen` emits the separate
four-seed terrain-conditioned plan used in the report. It queries only the
original surface-level root and chooses the first qualifying candidate out of
4096, without evaluating cave volume. Its plains/mountain thresholds and search
attempts are explicit in the original RustCraft adapter.

## Generator16 exposure diagnostics

The normal adapter and pinned revisions remain unchanged. Optional
`rustcraft_cave_reference DATA PLAN OUT --uncaved` creates a counterfactual
geological control: cave graph branches and carvers are disabled while terrain
slides/interpolation/squeeze and fluid handling remain. This is an analysis
adapter, not an unmodified vanilla-world export. The normal mode is separately
checked against the original reference-volume hashes.

`rustcraft_terrain_reference DATA --exposure` emits24 terrain-conditional sites
for the independent seeds123,777,4099,900001. Combine these with the16
unconditional rows from `cave_volume --exposure-plan`, preserving their X/Z.
Selection uses terrain only, with the same bounded search and regional strata.
For geometric exposure analysis, pass `--surface-offset=61` to
`tools/analyze_cave_exposure.py` for native Horizon terrain-category labels.
This affects labels only; exported Y, slopes, cover and cave geometry stay native.
See [the follow-up report](../../docs/performance/slice42/entrances/README.md)
for definitions, exact plans, optional controls and limitations.
