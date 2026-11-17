# Raw evidence

`compressed-data-manifest.json` maps each gzip archive to its original scratch
path, uncompressed byte count and SHA-256 digest. Archives use gzip with a fixed
zero timestamp; CSV/JSONL records are retained without statistical filtering.
For example:

```sh
gzip -dc terrain-v17.csv.gz > /tmp/terrain-v17.csv
sha256sum /tmp/terrain-v17.csv
```

`provenance.json` records the merged baseline, final candidate source hashes,
reference revisions, adapter/executable fingerprints and validation results.
`../artifact-sha256.json` covers every committed Slice43 evidence file except
itself; paths are relative to `docs/performance/slice43/`. The manifests let a
reviewer identify the exact input/output even though executables, exported voxel
volumes and external reference source/registry files are deliberately not
committed. The two small plan CSVs use LF line endings; their original
CRLF source digests and the normalization are recorded in provenance.

The geographic CSVs compare frozen v16, final v17, executed normal Horizon final
density and a separately labeled cave-disabled reference control. Independent
seeds use the `terrain-validation-*` and `horizon-validation32` files. The old
coarse surface search remains explicitly named `horizon-surface-proxy32`.
`horizon-final-volume32` and the complete, verified scalar prefix support the
122,708-location scalar/volume API comparison. Native router values are separate
from the shared geography CSV schema's placeholder columns.

The original v16 cave-exposure metrics, 29 freshly reanalyzed baseline checks,
120 freshly analyzed v17 windows, control volume parity manifests and sixteen
independent deep-window records remain separate. The Slice42 cohort labels
describe their original v16 selection, not a new balanced v17 terrain sample.
See [integration](../integration.md) before interpreting the counts.

`performance/` contains all retained core samples and both orders of the actual
headless streaming traces, methods, state/resource records and source/executable
fingerprints. `validation/` contains all final required check logs and focused
numerical/reference/regression checks. `captures/` contains compressed runtime
logs; image/camera metadata and the actual PNGs are in `../captures/`.

External source/input hashes are preserved in
`initial-reference-provenance.json`, with final adapter verification in
`reference-final-verification.json`. Executed graph geometry, the ablation
control, complete cave-stage exports and upstream-reported JVM parity have
different meanings; none establishes locally executed Minecraft parity.
