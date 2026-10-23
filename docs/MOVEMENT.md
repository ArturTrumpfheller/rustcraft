# Movement and safe sneaking (Slice 11)

The controller remains engine-independent `voxel_core::physics`, f64 feet-centre
coordinates, fixed 60 Hz, swept Y/X/Z voxel AABB collision. No new dependencies,
world/player format, block IDs, generator, mesh or streaming policy changes.

## Tuning and controls

| Mode | Slice 10 speed | Slice 11 speed | 20 blocks from rest, old → new |
| --- | --- | --- | --- |
| Walk, WASD | 4.3 | 5.0 blocks/s | 4.683 → 4.033 s |
| Sprint, Left Ctrl or double-W | 5.6 | 8.0 blocks/s | 3.617 → 2.567 s |
| Sneak, Left Shift + WASD | — | 1.8 blocks/s | — → 11.133 s |
| Sprint + held Space | — | at most 9.2 blocks/s | — → 2.500 s |

The deterministic flat-floor test includes acceleration and rounds arrival up to
one simulation tick. The double-W gesture is armed before starting the measured
course; it produces exactly the same sprint command as Ctrl. These are simulated travel times, not wall-clock benchmarks
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

## Sprint activation, jumping and FOV

Two distinct forward press edges within **250 ms inclusive**, with a release
between, latch sprint only for the second forward hold. The configured forward
binding is used, not a hardcoded W. Held-key OS repeats cannot activate it;
backward or sneak cancels the gesture. Sideways alone cannot activate it.
Ctrl still works independently, including sideways/backward. Releasing forward,
focus loss, cursor capture/release, inventory transitions, save-close, respawn,
void recovery and binding changes disarm double-tap state.

Held Space requests a legal jump on the first fixed tick after landing. Actual
support is rechecked each tick: no midair jump, stale-floor jump or invented
support. A short tap still survives a render frame without physics, but is consumed
by one tick. Release stops future jumps; no long-lived airborne tap buffer exists.
After modal/capture/focus/respawn/binding transitions, a held jump key is blocked
until released. This prevents a menu-held key from becoming a surprise launch.
A crouched jump still deliberately bypasses the ledge guard.

A supported sprint launch with directional input, at least walk-speed run-up and
0.5 blocks of overhead clearance earns **+1.2 blocks/s**, capped at **9.2**.
The bonus follows actual horizontal momentum, requires input aligned with it,
and is rejected if the boosted next horizontal step hits a solid/pending barrier.
No bonus for stationary/non-sprint/sneak launches or blocked low ceilings. Earned
speed is retained by sprint air control; vector turning cannot stack additional
speed. Every bonus requires another real grounded launch. Ground movement heads
back toward8.0; releasing sprint/sneaking in air uses normal bounded air control.

Reproducible 100-block flat course from rest, fixed60 Hz:

| Input | Time | Average including startup |
| --- | --- | --- |
| Walk | 20.033 s | 4.992 blocks/s |
| Ctrl or latched double-W sprint | 12.567 s | 7.958 blocks/s |
| Sprint + held Space | 11.200 s | 8.929 blocks/s |

Continuous hops are about12.2% faster over this course, without runaway gain.
Short courses gain less because the first jump from rest earns no launch bonus.
Run `cargo test --locked -p voxel_core flat_course_sprint_jumping -- --nocapture`
to reproduce. These are simulation timings, not an FPS/GPU benchmark.

Moving sprint widens perspective FOV by **9% relative to the existing base**;
the current default45° becomes49.05°. Ctrl while stationary does nothing.
Exponential easing (response10/s, about230 ms to90%) returns smoothly on stop,
sneak, menus or focus loss. A long-frame visual step is limited to50 ms of easing;
normal30/60/144-FPS responses match. External base-FOV edits are respected,
including exact baseline restoration; projection aspect/near/far and floating
origin stay independent. HUD/hotbar scale tracks projection through transforms,
with no new meshes/materials/images or per-frame asset rebuilds. There is no
settings UI yet; the gain/response are named constants in `client::camera`.

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
postures during that transition. Sprint FOV is independent of jump cadence; no
head bob or camera roll is added;
there are no per-tick allocations or new GPU resources.

See [validation](VALIDATION.md) and [Void Fish checklist](PLATFORMS.md#slice-11-movement-test-fish).
