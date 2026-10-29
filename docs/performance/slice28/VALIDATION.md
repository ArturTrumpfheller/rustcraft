# Slice28 validation

Required commands, run from the repository root with Rust1.99.1 / locked Cargo:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --release --locked -p rustcraft
python3 tools/generate_textures.py --check
```

Final local results: **all six commands passed** (the five required commands plus
building the renderer gallery). The workspace suite passed **193 client tests and
303 core tests**, with four pre-existing client measurement tests ignored. Format,
Clippy with denied warnings, locked release client build and texture verification
all passed. [Machine-readable command results](validation.json). The cloud host is Linux;
native Void X11/Wayland and Windows GUI/physical-GPU acceptance remain manual.
The existing `Rust checks` GitHub Actions matrix checks Ubuntu and Windows on
push/PR. Final remote run results are attached to the pull request; no Windows
execution is implied by local Linux checks.

## Additional evidence

- Existing generator1–7 fixture values retained; v8 includes independent CRCs for
  signed/distant/edge columns, reverse generation, parallel queries and regeneration.
- Procedural point/chunk agreement, connected centerlines, closed real cave pools,
  wet-bank caps/dry entrances, uniform source chunks and water mesh seams.
- Quiet untouched oceans/lakes/rivers/pools, maximum784-chunk source installation,
  ordinary swimming/underwater detection, bucket exchange, mined-pocket flow,
  torch washing, construction, flow convergence and overlay authority.
- Source collection/Air/construction/inventory recovery at SessionPrepared,
  WorldSaved and PlayerSaved journal boundaries without procedural edit records.
- CPU, queues, allocation/memory, save/reload and forward/back streaming:
  [full measurements](README.md), checked-in CSV/text files and reproducible tools.
- Seven genuine v8 production-renderer images with exact cameras, seed, biome,
  original material/shaders and declared torch/fog fixtures; no image generation.

An optional `cargo test --release --locked -p voxel_core --lib` run also revealed
pre-existing exact-f32 AO color comparisons differing by roughly8e-9. The same
failure was reproduced on unchanged Slice27 `b0a5bbf` with:

```sh
cargo test --release --locked -p voxel_core mesh::ao_tests::isolated_and_flat_surfaces_remain_bright_and_merge
```

Actual release values `[0.75,0.044478588,1,1]` versus expected
`[0.75,0.04447858,1,1]`; the original source fails the same assertion. The required
debug workspace suite is the acceptance command above. No unrelated lighting
change or loosened historical fixture was introduced to hide this optional issue.
