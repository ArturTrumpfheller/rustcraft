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
    /// Grid face used for adjacent-cell placement.
    pub normal: [i32; 3],
    /// Exact outward selection-surface normal (tilted for wall torches).
    pub surface_normal: [f64; 3],
    pub distance: f64,
    pub adjacent: Option<BlockPos>,
}

/// Ordinary editing ignores fluids; source collection has its own visibility policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetMode {
    Blocks,
    CollectSource,
}

pub fn raycast(world: &World, ray: Ray) -> Option<RayHit> {
    raycast_with(world, ray, TargetMode::Blocks)
}

/// Invalid/zero directions, nonfinite/out-of-i32 origins or reach outside
/// [0,64] return None. Direction magnitude does not affect distance or reach.
pub fn raycast_with(world: &World, ray: Ray, mode: TargetMode) -> Option<RayHit> {
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
        let block = world.block(cell);
        if mode == TargetMode::CollectSource && block == crate::Block::WaterSource {
            let height = if world.block(cell.offset([0, 1, 0])?).fluid().is_some() {
                1.
            } else {
                f64::from(block.fluid()?.height())
            };
            let bounds = crate::shape::SelectionBox {
                min: [0.; 3],
                max: [1., height, 1.],
            };
            if let Some((d, n)) = intersect_box(
                bounds,
                std::array::from_fn(|a| ray.origin[a] - f64::from(cell.0[a])),
                direction,
            ) && d + EPSILON >= distance
                && d <= ray.reach + EPSILON
            {
                let grid = n.map(|v| v as i32);
                return Some(RayHit {
                    position: cell,
                    normal: grid,
                    surface_normal: n,
                    distance: d,
                    adjacent: if grid == [0; 3] {
                        None
                    } else {
                        cell.offset(grid)
                    },
                });
            }
        }
        if block.is_cube() {
            return Some(RayHit {
                position: cell,
                normal,
                surface_normal: normal.map(f64::from),
                distance,
                adjacent: if normal == [0; 3] {
                    None
                } else {
                    cell.offset(normal)
                },
            });
        }
        if block.is_torch() {
            let frame = block.shape_transform();
            let local = frame.inverse_point(std::array::from_fn(|a| {
                ray.origin[a] - f64::from(cell.0[a])
            }));
            let vector = frame.inverse_vector(direction);
            let mut nearest: Option<(f64, [f64; 3])> = None;
            for bounds in block.selection_boxes() {
                if let Some((d, n)) = intersect_box(*bounds, local, vector)
                    && d + EPSILON >= distance
                    && d <= ray.reach + EPSILON
                    && nearest.is_none_or(|(old, _)| d < old)
                {
                    nearest = Some((d, n));
                }
            }
            if let Some((d, n)) = nearest {
                let surface_normal = frame.vector(n);
                let mut grid = [0; 3];
                let mut axis = 0;
                for a in 1..3 {
                    if surface_normal[a].abs() > surface_normal[axis].abs() + EPSILON {
                        axis = a;
                    }
                }
                if surface_normal[axis] != 0. {
                    grid[axis] = if surface_normal[axis] > 0. { 1 } else { -1 };
                }
                return Some(RayHit {
                    position: cell,
                    normal: grid,
                    surface_normal,
                    distance: d,
                    adjacent: if grid == [0; 3] {
                        None
                    } else {
                        cell.offset(grid)
                    },
                });
            }
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

fn intersect_box(
    bounds: crate::shape::SelectionBox,
    origin: [f64; 3],
    direction: [f64; 3],
) -> Option<(f64, [f64; 3])> {
    let mut enter = 0f64;
    let mut exit = f64::INFINITY;
    let mut normal = [0.; 3];
    for a in 0..3 {
        if direction[a] == 0. {
            if origin[a] < bounds.min[a] || origin[a] > bounds.max[a] {
                return None;
            }
            continue;
        }
        let mut lo = (bounds.min[a] - origin[a]) / direction[a];
        let mut hi = (bounds.max[a] - origin[a]) / direction[a];
        let sign = if direction[a] > 0. { -1. } else { 1. };
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        if lo > enter + EPSILON {
            enter = lo;
            normal = [0.; 3];
            normal[a] = sign;
        }
        exit = exit.min(hi);
        if enter > exit + EPSILON {
            return None;
        }
    }
    (exit >= 0.).then_some((enter, normal))
}

#[cfg(test)]
mod tests;
