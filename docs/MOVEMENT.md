# Movement and safe sneaking (Slice 11)

The controller remains engine-independent `voxel_core::physics`, f64 feet-centre
coordinates, fixed 60 Hz, swept Y/X/Z voxel AABB collision. No new dependencies,
world/player format, block IDs, generator, mesh or streaming policy changes.

## Tuning and controls

| Mode | Slice 10 speed | Slice 11 speed | 20 blocks from rest, old → new |
| --- | --- | --- | --- |
| Walk, WASD | 4.3 | 5.0 blocks/s | 4.683 → 4.033 s |
| Sprint, Left Ctrl + WASD | 5.6 | 8.0 blocks/s | 3.617 → 2.567 s |
| Sneak, Left Shift + WASD | — | 1.8 blocks/s | — → 11.133 s |

The deterministic flat-floor test includes acceleration and rounds arrival up to
one simulation tick. These are simulated travel times, not wall-clock benchmarks
or native gameplay acceptance. Sprint is now 60% faster than walk (previously
30.2%); walking itself is 16.3% faster. Diagonals normalize before target speed.

Ground acceleration is 65 blocks/s² (was 55), braking 75 (was 60). From rest,
walk/sprint/sneak reach target in 5/8/2 ticks (83/133/33 ms); walk/sprint release
stops within 4/7 ticks (67/117 ms). Turning approaches the new velocity with the
same bounded vector acceleration. Air acceleration stays 12, gravity 24,
jump impulse 8.5 and terminal fall speed 55. Releasing direction in the air
preserves horizontal momentum. Sneaking in the air adjusts the desired velocity
only through ordinary air control: no midair speed snap or fall cancellation.

`client::controls::MovementBindings` centralizes locomotion keys and produces
simulation commands. It is replaceable for future rebinding; no settings UI or
binding persistence yet. E, 1–9/wheel, mouse clicks, Escape and F5 retain their
existing adapters. Shift-click transfers while inventory is open, with simulation
paused; it cannot change the body posture. Capture frames discard jump/respawn
edges and pre-capture pointer motion; held movement starts normally.

## Posture, support and edges

Standing body/eye are 0.6×1.8 / 1.62 blocks. Crouched body/eye are 0.6×1.5 / 1.32.
Feet stay fixed when changing posture. Collision, clearance, streamed readiness,
block-placement overlap and targeting all use actual posture. Releasing Shift
stands only if the standing AABB fits; otherwise crouch/sneak speed and edge guard
remain until clear. Shrinking cannot enter solids. Whole-block terrain has no
slabs: crouch does not enable passage through a one-block-high tunnel.

Sneak overrides sprint. On supported ground it caps existing horizontal momentum
to sneak speed immediately, so sprint-to-sneak cannot carry the player over a
ledge. Ground support is rechecked from actual blocks each tick. After ordinary
collision clipping, horizontal X then Z movement is restricted to supported
positions, using bounded probes and 20-step bisection at the first unsupported
position. The ordinary open-interval contact tolerance is shared with collision;
a partial foot overlap suffices, including diagonal corners. Tangential ledge
sliding remains possible without position reversal or signed chunk seams.

The guard only applies when grounded and not jumping. It neither extends support
downward nor teleports/snaps the body to terrain. Space intentionally jumps off a
sneaked edge. Already falling players continue falling; mining the supporting
block immediately invalidates support. Pending chunks remain solid barriers,
never a safe-to-enter guessed floor. Respawn resets to standing, zero velocity,
and existing edited-ground/stream loading logic.

Eye height interpolates over the same single fixed tick as feet, with immediate
mouse rotation. Placement also protects the interpolated feet and taller of both
postures during that transition. No head bob, FOV pulsing or camera roll is added;
there are no per-tick allocations or new GPU resources.

See [validation](VALIDATION.md) and [Void Fish checklist](PLATFORMS.md#slice-11-movement-test-fish).
