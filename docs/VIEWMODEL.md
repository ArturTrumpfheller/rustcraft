# First-person hand and held block (Slice 12)

`client::viewmodel` is presentation only. Original authored forearm, cuff, palm
and side thumb are four colored cuboids merged into one mesh. All five held
block meshes reuse the terrain atlas's exact per-face tile/UV orientation,
including turf sides/dirt underside and upright wood end-grain. No image,
dependency, shader, block/item ID, generator or world/player format changes.

## Rendering and resources

A normal camera-child mesh still competes with nearby wall/ceiling depth; merely
shrinking it into the near plane fails at extreme head contacts and FOV changes.
Instead one overlay Camera3d follows the existing first-person camera as an
identity child: order1, color load, depth clear, RenderLayers1. World chunks and
target outline remain layer0. Existing inventory, crosshair and save-HUD meshes
are explicitly layer1, with their original near-camera depths in front of the
hand. There is no third UI camera and no change to terrain lighting/materials.

The overlay copies the perspective FOV/aspect, with local near0.01/far10. A
normalized lower-right anchor and tangent-scaled model keep apparent size/position
steady as sprint FOV changes; narrow aspect ratios reduce model size. Parent
transforms follow look and the floating origin; animation never moves the main
camera, mouse angles, targeting ray, player body or world coordinates.

Six retained meshes (one96-vertex/144-index arm, five24-vertex/36-index cubes),
two unlit materials and four entities (overlay camera, motion root, arm, block)
are created once. Switching/emptying a slot changes a mesh handle/visibility only.
No viewmodel mesh/material/image allocation occurs per frame. The original
32-KiB atlas is shared; no new Image. Vertex/index payload is **11664 bytes**
(48-byte vertices, u32 indices), retained on CPU and uploaded to GPU, excluding
engine/driver alignment and asset metadata. At most **two extra geometry draws**
(60 triangles total), one for an empty hand. There is one additional 3D render
pass/view and per-view culling/uniform cost; this is not a zero-cost overlay.

Bevy0.20's `prepare_core_3d_depth_textures` shares depth by target and MSAA;
both views target the same primary window/sample count. The second pass clears
that depth instead of allocating a second requested target Image. Renderer-managed
color/depth attachments and driver allocations are separate from Assets<Image>
and the tiny mesh payload; short process-RSS measurements cannot prove VRAM cost.

Face modulation is fixed, muted fill (held blocks max0.55 linear, with directional
factors); original skin/slate-cuff colors are vertex-authored. Unlit surfaces stay
readable in caves without emitted lights, shadow maps or terrain-light updates.
There is no environment-light sampling or photorealistic hand shading yet.

## Motion, actions and state

- Idle is still. A distance-driven phase and eased amplitude use actual displayed
  feet movement. Pushing a wall cannot continuously drive bob from held keys.
  Sprint produces faster/larger bob; crouch eases to a lower, quieter pose.
- Vertical velocity gives a small jump/fall offset; falling-to-ground (including
  held-jump relaunch between frames) produces a bounded, decaying landing dip.
- Left click swings forward; right click uses a smaller placement gesture.
  These are gameplay attempts, including misses/refusals, not proof an edit
  succeeded. The existing active/capture/reset gate publishes the action serial.
- Rapid actions restart one240-ms attack/return envelope from its current value,
  blending action strength; they do not queue or add unlimited swings.
- Inventory open/transitions, released cursor, focus/save-close pauses, capture,
  spawn waiting and explicit respawn hide/reset the hand. Action serials are
  consumed while hidden; resume cannot replay a previous click. Large teleports
  reset travel history. Selection still synchronizes while hidden.

The selected actual stack is read after authoritative interactions in the same
render frame. Numbers, wheel, transfer, depletion, mining and restored inventories
need no separate viewmodel item state or persistence. An empty slot shows the same
hand without a cube. World/player save compatibility remains covered by the full
suite. Tests include30/60/144-FPS motion, projection/rebase/asset stress, exact UVs,
real edit transactions and player-session roundtrip. See VALIDATION and the
[Void Fish checklist](PLATFORMS.md#slice-12-viewmodel-test-fish).
