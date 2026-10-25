# Movement and safe sneaking (Slice 11)

The controller remains engine-independent `voxel_core::physics`, f64 feet-centre
coordinates, fixed 60 Hz, swept Y/X/Z voxel AABB collision. No new dependencies,
world/player format, block IDs, generator, mesh or streaming policy changes.

## Tuning and controls

| Mode | Current steady speed | 20 blocks from rest | 100 blocks from rest |
| --- | --- | --- | --- |
| Walk, WASD | 4.317 blocks/s | 4.667 s | 23.200 s |
| Sprint, Left Ctrl or double-W | 5.612 blocks/s | 3.600 s | 17.867 s |
| Sneak, Left Shift + WASD | 1.295 blocks/s | — | — |
| Sprint + held Space | capped at 6.4538 blocks/s | 3.333 s | 15.733 s |

Slice 19 adopts approximate Minecraft Java normal flat-ground speeds, in metres
per second. One second of warmup followed by four seconds of measured fixed ticks
achieves exactly 4.317000 / 5.612000 / 1.295000, cardinal and diagonal, at 30/60/144
render FPS; complete end states are identical. Course times include acceleration
and round arrival up to a simulation tick, so averages differ from steady speed.

Ground acceleration stays65 blocks/s², braking75, air acceleration12, gravity24,
jump impulse8.5 and terminal fall speed55. Ground targets are reached within4/6/2
ticks (walk/sprint/sneak); releasing walk/sprint stops within4/5 ticks. Releasing
direction in air preserves horizontal momentum; ordinary bounded air control,
collision, support, jump cadence and ledge protection remain unchanged.

`client::controls::MovementBindings` centralizes locomotion keys and produces
simulation commands. Slice15 uses this resource for persistent rebinding;
see [SETTINGS](SETTINGS.md). E, 1–9/wheel, mouse clicks, Escape and F5 retain their
existing adapters. Shift-click transfers while inventory is open cannot drive
posture or movement. Inventory sends neutral commands to running physics, including
gravity and normal ground braking. Capture/modal transitions discard gameplay
edges and pre-capture motion; held movement/jump must be released before reuse.

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
0.5 blocks of overhead clearance earns **+0.8418 blocks/s**, capped at **6.4538**.
This scales the old+1.2 /8.0 sprint tuning proportionally, retaining 15% launch
headroom rather than increasing the relative bonus as ground sprint slows.
The bonus follows actual horizontal momentum, requires input aligned with it,
and is rejected if the boosted next horizontal step hits a solid/pending barrier.
No bonus for stationary/non-sprint/sneak launches or blocked low ceilings. Earned
speed is retained by sprint air control; vector turning cannot stack additional
speed. Every bonus requires another real grounded launch. Ground movement heads
back toward5.612; releasing sprint/sneaking in air uses normal bounded air control.

The 100-block course now averages 4.310 blocks/s walking,5.597 sprinting and6.356
sprint-hopping. Hops remain about13.6% faster than normal sprint, within the existing
16% regression bound. Run `cargo test --locked -p voxel_core flat_course_sprint_jumping
-- --nocapture` (as one command) to reproduce. See [Slice 19](DEBUG_GAMEPLAY.md) for
speed replay commands and native acceptance; these are simulation, not GPU timings.

Moving sprint widens perspective FOV by **9% relative to the existing base**;
the current default45° becomes49.05°. Ctrl while stationary does nothing.
Exponential easing (response10/s, about230 ms to90%) returns smoothly on stop,
sneak, menus or focus loss. A long-frame visual step is limited to50 ms of easing;
normal30/60/144-FPS responses match. External base-FOV edits are respected,
including exact baseline restoration; projection aspect/near/far and floating
origin stay independent. HUD/hotbar scale tracks projection through transforms,
with no new meshes/materials/images or per-frame asset rebuilds. The settings UI supplies the base FOV; the gain/response remain named constants
in `client::camera`.

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
