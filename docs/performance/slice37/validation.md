# Slice37 — validation and platform status

Toolchain: Rust1.99.1, Bevy0.20.0, edition2024, locked Cargo dependencies. Exact
baseline3d0f13c7e603ed060d53a722cf73c51619ade308. Local cloud is Linux x86-64;
Windows deterministic execution is checked separately by the two-platform CI.
Native GUI acceptance remains explicitly separate.

## Compatibility and deterministic coverage

- 88 independently emitted baseline columns/biomes span every generator1–11,
 seeds0/42/2026/MAX, negative coordinates and distant regional coordinates.
 Historical golden tests and68 independent v11 density fixtures also remain.
 [Baseline emitter](raw/history_emitter.rs) can be copied to
 `crates/voxel_core/examples/slice37_history.rs` in an exact Slice36 checkout;
 `cargo run --release --locked -p voxel_core --example slice37_history` must
 reproduce `crates/voxel_core/tests/fixtures/history-v1-v11.csv` byte for byte.
- 56 independently implemented Python Q10 climate/elevation/snowline fixtures
 pin signed arithmetic, interpolation and distant boundaries. Rust compares all
 fields. Climate continuity and four distinct seed signatures are tested.
- Whole v11/v12 full-height columns compare every base solid/Air/fluid cell and
 deep geology at signed/distant/world-edge, mountain, coast and both Grove sites.
 Point/sampler/chunk/halo and reverse/parallel order agree. Every generated crown
 stays above actual terrain; roots/plants have valid dry support.
- Real hydrology tests compare classified marine/channel/lake water with actual
 `WaterBody`, sea-level Water Source cells and dry shoreline/material constraints.
 Optimized marine probes match full hydrology, including the protected starting
 shore and world edges. Cached probes also match point probes across complete
 coast/spawn/edge tiles and stay within756 exterior entries. Altitude, warm/dry intermediates and cliff materials have
 targeted checks.
- Actual generated patches distinguish Oak/Birch/Spruce ecosystems, fern and
 flower densities, sparse dry bushes and treeless alpine/wet biomes.
- Generator12 default, stable biome IDs0–14, future13/MAX rejection, identity/save
 envelopes, player/inventory and explicit saved Air are covered. V12 vegetation
 removals, journal interruption/recovery, cross-region edits, save/reload,
 eviction/regeneration and repeated A→B→A world switching use existing lifecycle
 suites extended to12. Existing player-planted support/displacement paths remain.
- V12 cancellation has a gated worker regression: an evicted column cannot reappear
 from a stale result. Existing publication/dirty revisions/cancellation/shutdown
 tests remain. F3 caches a single identity/position and switches correctly at the
 same coordinates between different worlds.

The tests live in `crates/voxel_core/tests/biome_v2.rs`, existing worldgen/
persistence/lifecycle suites, and client streaming/session/debug_ui suites.

## Local commands

The full locked debug workspace run passes576 regular tests with7 deliberately
ignored measurement/acceptance probes. The subsequently added real-water test
passes separately, bringing distinct regular tests to577. The final five biome
unit tests are rerun after the coastal optimization and protected-spawn fallback.
The release core suite includes all current tests, including both added water
checks. Logs record actual commands/results rather than treating ignored probes
as successful tests.

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo test --release --locked -p voxel_core --lib
cargo test --release --locked -p voxel_core --test terrain_v3 --test terrain_v4 --test biome_v2 --test worldgen_integration --test worldgen_lifecycle
cargo test --release --locked -p rustcraft --bin rustcraft streaming:: -- --test-threads=1
cargo test --release --locked -p rustcraft --bin rustcraft session:: -- --test-threads=1
cargo test --release --locked -p rustcraft --bin rustcraft scene:: -- --test-threads=1
cargo test --release --locked -p rustcraft --bin rustcraft debug_ui:: -- --test-threads=1
cargo build --release --locked --workspace
python tools/generate_textures.py --check
python tools/terrain_v3_reference.py --check
python tools/terrain_v4_reference.py --check
python tools/biome_v2_reference.py --check
```

[Local logs](raw/validation/) retain full output. Original asset verification adds
no dependencies/assets and changes no block/item IDs. Optimized ignored benchmark
probes are explicitly invoked by the disposable performance runners; their raw
records are separate from regression results.

## Linux/Windows CI

The existing matrix runs Ubuntu and Windows with the pinned toolchain, original
asset verification, three independent Python fixture sets, format, locked
all-target Clippy, full workspace tests/build, optimized core/integration,
streaming/session/rendering/F3 regressions and release workspace build.

[Live branch Actions results](https://github.com/ArturTrumpfheller/rustcraft/actions?query=branch%3Afeat%2Fslice-37-biome-generation-v2)
provide individual Ubuntu/Windows jobs and logs. The PR description records the
completed branch-run URL and conclusions; the PR is opened only after local
evidence and two-platform branch validation complete.
No merge or auto-merge is requested.

## Actual rendered runtime and native limits

The genuine Bevy gallery and ordinary client/F3 runs are in
[comparison.md](comparison.md). Both new-world client sessions create through real
menus, stream to convergence, walk, pause/resume and save/quit normally with
exit0. Both show Generator12 and the expected ecosystem. They use isolated test
catalogs, never a user save.

Cloud Xvfb/llvmpipe checks neither Void/Gentoo Wayland nor Windows GUI/cursor/driver
behavior and cannot validate the owner's Radeon FPS. Native Gentoo/Void X11 and
Wayland, Windows10/11 and Radeon acceptance remain **pending owner hardware
execution** using the explicit README procedure. This limitation does not imply
Linux/Windows CI parity or the cloud screenshots cover those GUI environments.
