// SPDX-License-Identifier: MIT OR Apache-2.0
//! Frozen Slice30 lifecycle scan; test oracle for exact queue/order equivalence.
use super::*;
impl World {
    pub(crate) fn wake_installed_fluids_reference(&mut self, cp: ChunkPos) {
        // Only lifecycle events scan a chunk; skip dry chunks in constant time.
        // Neighbor water wakes across both faces even if the installed chunk is dry.
        // Include nearby interior/diagonal water whose bounded route can now see
        // into this chunk. Also used after eviction, with no missing data read.
        for d in (-1..=1).flat_map(|y| (-1..=1).flat_map(move |z| (-1..=1).map(move |x| [x, y, z])))
        {
            let np = ChunkPos(std::array::from_fn(|a| cp.0[a] + d[a]));
            if self.chunk(np).is_none_or(|c| c.water_cells() == 0) {
                continue;
            }
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let lp = LocalPos::new([x, y, z]).unwrap();
                        let p = np.block_position(lp).unwrap();
                        let distance = |a: usize| {
                            let low = cp.0[a] * 16;
                            (low - p.0[a]).max(p.0[a] - (low + 15)).max(0)
                        };
                        // A dry consumer one cell beyond a wet feeder may have
                        // deferred because its next neighbor was pending. Wake
                        // that feeder too when the missing vertical slab arrives.
                        if distance(1) > 2 || distance(0) + distance(2) > ROUTE_DISTANCE {
                            continue;
                        }
                        let block = self.chunk(np).unwrap().get(lp);
                        let active = if block == Block::WaterSource {
                            // A generated source body is already stable. Sources
                            // cannot flow upward, into solids, or into sources.
                            // Wake only real resident consumers; pending boundaries
                            // will be reconsidered by the next lifecycle event.
                            NEIGHBORS.iter().filter(|d| **d != [0, 1, 0]).any(|&d| {
                                p.offset(d).is_some_and(|n| {
                                    self.in_bounds(n)
                                        && self.ready(n)
                                        && self.block(n) != Block::WaterSource
                                        && self.block(n).fluid_replaceable()
                                })
                            })
                        } else {
                            block.fluid().is_some()
                        };
                        if active {
                            self.fluids.invalidate_route(p);
                            self.wake_fluid(p);
                        }
                    }
                }
            }
        }
    }
}
