# Slice 38 validation

Base: `cec8f593d20c012725ed17308bb9c7d44b470e08` (merged Slice37).
Pinned Rust1.99.1, Bevy0.20.0, edition2024; Cargo.lock and runtime dependencies
are unchanged. No original asset files change.

Completed local checks:

- `cargo fmt --all -- --check`.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`.
- Independent v10 height, v11 density, v12 climate and new v13 density fixtures.
- `python tools/generate_textures.py --check`.
- Optimized core library:337 passed,1 intentionally ignored pre-existing benchmark.
- Optimized biome/history integration:7 passed (including256 frozen Slice37
  full-column/biome cases covering v1–v12).
- Optimized v13 integration:5 passed (full-height audit, point/halo/order/parallel
  parity, gravity-stable spawn, watertight sources and all fifteen real ecosystems).
- Optimized saved-Air/construction/tree/plant/regeneration lifecycle:1 passed,
  extended through v13.
- Optimized Bevy terrain-gallery build.

Additional completed checks:

- `cargo test --locked --workspace -- --test-threads=1`:585 passed,7 intentionally
  ignored benchmark tests,0 failures (plus empty doc tests).
- Final v12/v13 cancellation loop separately passed on the final test source;
  final locked all-target Clippy passed again.
- `cargo test --release --locked -p rustcraft --bin rustcraft streaming:: -- --test-threads=1`:47 passed,2 ignored.
- Optimized session:12 passed,1 ignored; rendering:10 passed,1 ignored;
  F3 diagnostics:9 passed,1 ignored.
- Optimized terrain/persistence integration across v10–v13:26 passed.
- `cargo build --release --locked --workspace` and debug workspace build passed.
- [Linux and Windows source CI, run203](https://github.com/ArturTrumpfheller/rustcraft/actions/runs/43422241636)
  passed on `742f0b5885f9c29d8fb9e47131315bcb85ca5ae3` before final evidence-only
  documentation/capture-tool updates. Final PR-head checks are linked on the PR.

 Source/test expectations that treated13 as unknown were updated to14;
unknown versions still fail closed. The new continuity regression distinguishes
quiet ordinary offsets from intentionally retained high-ridge cliffs, rather than
requiring every natural mountain face to be gentle. No historical fixture value
was adjusted to accommodate a generator change.

The existing Linux/Windows CI matrix adds the independent v13 reference and
optimized v13 integration tests. All deterministic fixtures must match both OSes.
Native Gentoo/Void X11/Wayland, Windows GUI/input and physical GPU performance
remain owner acceptance; headless CI and Mesa software captures do not prove them.
