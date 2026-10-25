//! Grid DDA targeting. Cells are half-open [n,n+1); an origin in solid returns
//! distance zero with no entry face/placement cell. Ties within 1e-10 blocks
//! step X, then Y, then Z, checking each cell (including edge-touch cells).

use crate::{BlockPos, World};

pub const DEFAULT_REACH: f64 = 5.0;
pub const MAX_REACH: f64 = 64.0;
const EPSILON: f64 = 1e-10;

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
    pub reach: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub position: BlockPos,
    pub normal: [i32; 3],
    pub distance: f64,
    pub adjacent: Option<BlockPos>,
}

/// Invalid/zero directions, nonfinite/out-of-i32 origins or reach outside
/// [0,64] return None. Direction magnitude does not affect distance or reach.
pub fn raycast(world: &World, ray: Ray) -> Option<RayHit> {
    if !ray.reach.is_finite()
        || !(0.0..=MAX_REACH).contains(&ray.reach)
        || !ray.origin.iter().all(|v| {
            v.is_finite() && v.floor() >= f64::from(i32::MIN) && v.floor() <= f64::from(i32::MAX)
        })
        || !ray.direction.iter().all(|v| v.is_finite())
    {
        return None;
    }
    let length = ray.direction[0]
        .hypot(ray.direction[1])
        .hypot(ray.direction[2]);
    if length == 0.0 || !length.is_finite() {
        return None;
    }
    let direction = ray.direction.map(|v| v / length);
    let mut cell = BlockPos(ray.origin.map(|v| v.floor() as i32));
    let mut step = [0; 3];
    let mut next = [f64::INFINITY; 3];
    let mut stride = [f64::INFINITY; 3];
    for axis in 0..3 {
        if direction[axis] == 0.0 {
            continue;
        }
        step[axis] = if direction[axis] > 0.0 { 1 } else { -1 };
        let boundary = f64::from(cell.0[axis]) + if step[axis] > 0 { 1.0 } else { 0.0 };
        next[axis] = ((boundary - ray.origin[axis]) / direction[axis]).max(0.0);
        stride[axis] = 1.0 / direction[axis].abs();
    }
    let mut distance = 0.0;
    let mut normal = [0; 3];
    loop {
        if world.block(cell).is_targetable() {
            return Some(RayHit {
                position: cell,
                normal,
                distance,
                adjacent: if normal == [0; 3] {
                    None
                } else {
                    cell.offset(normal)
                },
            });
        }
        let mut axis = 0;
        for candidate in 1..3 {
            if next[candidate] < next[axis] - EPSILON {
                axis = candidate;
            }
        }
        distance = next[axis];
        if distance > ray.reach + EPSILON || !distance.is_finite() {
            return None;
        }
        cell.0[axis] = cell.0[axis].checked_add(step[axis])?;
        normal = [0; 3];
        normal[axis] = -step[axis];
        next[axis] += stride[axis];
    }
}

#[cfg(test)]
mod tests;
