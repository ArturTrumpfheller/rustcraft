//! Deterministic bounded service: two jobs at most from either class while both
//! are eligible. Every fourth service offers an aged request a turn. Only an
//! unknown column intersecting the player's immediate collision envelope bypasses
//! load aging; motion is a tie-breaker inside the nearest ring.
use super::*;

const AGED: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Load,
    Mesh,
}
#[derive(Default)]
pub(super) struct Scheduler {
    last: Option<Kind>,
    streak: u8,
    loads: u64,
    meshes: u64,
}
impl Scheduler {
    pub(super) fn dispatched(&mut self, work: &Work) {
        let kind = match work {
            Work::Load(_) => {
                self.loads = self.loads.wrapping_add(1);
                Kind::Load
            }
            Work::Mesh(_) => {
                self.meshes = self.meshes.wrapping_add(1);
                Kind::Mesh
            }
        };
        self.streak = if self.last == Some(kind) {
            self.streak.saturating_add(1).min(2)
        } else {
            1
        };
        self.last = Some(kind);
    }
    #[allow(clippy::too_many_arguments)] // Bounded queues plus explicit player/clock inputs for deterministic replay.
    pub(super) fn choose(
        &self,
        queued: &BTreeSet<Column>,
        queued_at: &BTreeMap<Column, Instant>,
        dirty: &BTreeMap<ChunkPos, Instant>,
        failures: &BTreeMap<Column, (Instant, String)>,
        player: &voxel_core::physics::Player,
        center: Column,
        now: Instant,
    ) -> Option<Work> {
        let blocked = |c: &Column| failures.get(c).is_some_and(|(until, _)| *until > now);
        let loads: BTreeSet<_> = queued.iter().filter(|c| !blocked(c)).copied().collect();
        // A 0.6-block horizontal envelope also anticipates the next fixed step
        // at the existing sprint cap; no residency prediction is introduced.
        let urgent: BTreeSet<_> = loads
            .iter()
            .filter(|c| {
                [0, 1].into_iter().all(|a| {
                    let p = player.position[a * 2];
                    p + 0.6 >= f64::from(c[a]) * 16. && p - 0.6 < f64::from(c[a] + 1) * 16.
                })
            })
            .copied()
            .collect();
        let load = if !urgent.is_empty() {
            streaming::nearest_ahead(&urgent, center, [0.; 2])
        } else if self.loads % 4 == 3 {
            loads
                .iter()
                .filter(|c| {
                    queued_at
                        .get(*c)
                        .is_some_and(|t| now.saturating_duration_since(*t) >= AGED)
                })
                .min_by_key(|c| (queued_at[*c], **c))
                .copied()
                .or_else(|| {
                    streaming::nearest_ahead(
                        &loads,
                        center,
                        [player.velocity[0], player.velocity[2]],
                    )
                })
        } else {
            // Age precedes motion once requests have waited, but never displaces
            // a nearer ring except in the explicit fourth-service aging slot.
            let nearest =
                streaming::nearest_ahead(&loads, center, [player.velocity[0], player.velocity[2]]);
            nearest.map(|first| {
                let ring = distance(first, center);
                loads
                    .iter()
                    .filter(|c| distance(**c, center) == ring)
                    .filter(|c| {
                        queued_at
                            .get(*c)
                            .is_some_and(|t| now.saturating_duration_since(*t) >= AGED)
                    })
                    .min_by_key(|c| (queued_at[*c], **c))
                    .copied()
                    .unwrap_or(first)
            })
        };
        let targets = dirty_component(dirty, player.position, self.meshes % 4 == 3, now, blocked);
        match (load, targets.is_empty()) {
            (None, true) => None,
            (Some(c), true) => Some(Work::Load(c)),
            (None, false) => Some(Work::Mesh(targets)),
            (Some(c), false) => {
                let load_turn = if self.streak == 2 {
                    self.last == Some(Kind::Mesh)
                } else {
                    !urgent.is_empty()
                };
                if load_turn {
                    Some(Work::Load(c))
                } else {
                    Some(Work::Mesh(targets))
                }
            }
        }
    }
}
fn distance(c: Column, center: Column) -> i64 {
    (i64::from(c[0]) - i64::from(center[0]))
        .abs()
        .max((i64::from(c[1]) - i64::from(center[1])).abs())
}

fn dirty_component(
    dirty: &BTreeMap<ChunkPos, Instant>,
    player: [f64; 3],
    age_turn: bool,
    now: Instant,
    blocked: impl Fn(&Column) -> bool,
) -> BTreeSet<ChunkPos> {
    let mut eligible = dirty.clone();
    while !eligible.is_empty() {
        let oldest = age_turn
            .then(|| {
                eligible
                    .iter()
                    .filter(|(_, t)| now.saturating_duration_since(**t) >= AGED)
                    .min_by_key(|(p, t)| (**t, **p))
                    .map(|(&p, _)| p)
            })
            .flatten();
        let group = component(
            &eligible,
            oldest.unwrap_or_else(|| nearest_dirty_seed(&eligible, player)),
        );
        if group.iter().all(|p| !blocked(&[p.0[0], p.0[2]])) {
            return group;
        }
        // A failed member blocks its whole publication unit, never just one face.
        for p in group {
            eligible.remove(&p);
        }
    }
    BTreeSet::new()
}
fn nearest_dirty_seed(dirty: &BTreeMap<ChunkPos, Instant>, player: [f64; 3]) -> ChunkPos {
    let pc = player.map(|v| (v.floor() as i32).div_euclid(16));
    *dirty
        .keys()
        .min_by_key(|p| {
            (
                p.0.iter()
                    .zip(pc)
                    .map(|(&a, b)| (i64::from(a) - i64::from(b)).pow(2))
                    .sum::<i64>(),
                **p,
            )
        })
        .unwrap()
}
pub(super) fn component(
    dirty: &BTreeMap<ChunkPos, Instant>,
    first: ChunkPos,
) -> BTreeSet<ChunkPos> {
    let mut result = BTreeSet::from([first]);
    let mut frontier = vec![first];
    while let Some(p) = frontier.pop() {
        for axis in 0..3 {
            for sign in [-1, 1] {
                let mut n = p.0;
                n[axis] += sign; // Only bounded world chunk positions enter dirty.
                let n = ChunkPos(n);
                if dirty.contains_key(&n) && result.insert(n) {
                    frontier.push(n);
                }
            }
        }
    }
    result
}
#[cfg(test)]
pub(super) fn nearest_dirty(
    dirty: &BTreeMap<ChunkPos, Instant>,
    player: [f64; 3],
) -> BTreeSet<ChunkPos> {
    if dirty.is_empty() {
        BTreeSet::new()
    } else {
        component(dirty, nearest_dirty_seed(dirty, player))
    }
}

#[cfg(test)]
mod tests;
