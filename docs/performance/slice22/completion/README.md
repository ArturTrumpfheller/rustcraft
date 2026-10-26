# Slice22 completion measurements

The comparison baseline is PR39 head `a976d37b9bc949ba3e283df39e1236812ab74c69`,
which already has the complete original water simulation. The historical
[Slice21 comparison](../README.md) remains separate.

The CPU harness runs five trials each for dry, stationary pool, falling water,
twelve torches in the flow path, and chunk-boundary flow. It records active tick
latencies, total updates, washed torches and10,000 idle ticks. New-only metrics
isolate1,000 item creation/pickup cycles and10,000 full-inventory pickup attempts.
The baseline has no dropped-item API, so a pickup speedup is not claimed.

Renderer comparisons use sequential matching960×640, radius2, fixed-time6000
fixtures with the production shaders. No builds/tests may run during final
measurements. Software Vulkan/lavapipe results are **not native GPU performance**;
no inference about the owner's ~240-FPS RX9060 XT baseline is valid.

Reproduction (source `/workspace/setup/env.sh` in the prepared cloud):

```sh
python tools/prepare_water_completion_baseline.py work/water-completion-baseline
CARGO_TARGET_DIR="$PWD/target" cargo build --release --locked --manifest-path work/water-completion-baseline/Cargo.toml -p voxel_core --example water_completion_bench
cp target/release/examples/water_completion_bench work/water-completion-before
cargo build --release --locked -p voxel_core --example water_completion_bench
cp target/release/examples/water_completion_bench work/water-completion-after
work/water-completion-before > work/before.csv 2> work/before-resource.txt
work/water-completion-after > work/after.csv 2> work/after-resource.txt
# Build/copy water_gallery for each tree similarly, using debug binaries for both.
python tools/measure_water.py work/water-gallery-before work/water-gallery-after work/frames --frames 60 --before-label before --after-label after
cargo test --locked -p rustcraft item_presentation_cpu_measurements -- --ignored --nocapture
```

The minimal ECS item-presentation probe reports CPU update time and logical
retained meshes/vertices/entities, without a renderer or GPU. It does not measure
frame time, VRAM, driver allocations or terrain worker latency. Runtime item
creation shares one lazy mesh/material; all items are capped at4096 stacks and
save data at61,636 bytes. Session-switch tests check complete asset reclamation.

Limits: real Void X11/Wayland and Windows GUI/GPU acceptance remains manual.
Mesh-level translucent sorting can blend intersecting waterfalls incorrectly.
