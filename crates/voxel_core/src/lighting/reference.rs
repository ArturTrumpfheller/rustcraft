// SPDX-License-Identifier: MIT OR Apache-2.0
//! Frozen Slice30 collector/flood oracle. Keep independent of production changes.
use super::*;
pub(super) fn calculate(
    world: &World,
    targets: &BTreeSet<ChunkPos>,
    (min_y, max_y): (i32, i32),
) -> Skylight {
    assert!(!targets.is_empty());
    let min = [0, 2].map(|a| targets.iter().map(|p| p.0[a]).min().unwrap() * 16 - HALO);
    let max = [0, 2].map(|a| (targets.iter().map(|p| p.0[a]).max().unwrap() + 1) * 16 + HALO);
    let size = [(max[0] - min[0]) as usize, (max[1] - min[1]) as usize];
    // Callers bound targets to resident radius (maximum 7x7 columns).
    assert!(size[0] <= 7 * 16 + 2 * HALO as usize && size[1] <= 7 * 16 + 2 * HALO as usize);
    let world_height = world.world_height();
    let height = (max_y - min_y) as usize;
    let count = size[0] * size[1] * height;
    let mut solid = vec![false; count];
    let mut levels = vec![0; count];
    let mut queue = VecDeque::<u32>::new();
    let mut sources = Vec::new();
    let mut floors = vec![0usize; size[0] * size[1]];
    for z in 0..size[1] {
        for x in 0..size[0] {
            let wx = min[0] + x as i32;
            let wz = min[1] + z as i32;
            let base = (z * size[0] + x) * height;
            let mut sky = true;
            for cy in (min_y.div_euclid(16)..(max_y + 15).div_euclid(16)).rev() {
                let cp = ChunkPos([wx.div_euclid(16), cy, wz.div_euclid(16)]);
                let chunk = world.chunk(cp);
                let pending = chunk.is_none() && !world.ready(BlockPos([wx, cy * 16, wz]));
                for ly in (0..16).rev() {
                    let y = cy * 16 + ly as i32;
                    if !(min_y..max_y).contains(&y) {
                        continue;
                    }
                    let i = base + (y - min_y) as usize;
                    let block = chunk.map_or(crate::Block::Air, |c| {
                        c.get(
                            LocalPos::new([
                                wx.rem_euclid(16) as usize,
                                ly,
                                wz.rem_euclid(16) as usize,
                            ])
                            .unwrap(),
                        )
                    });
                    let opaque = pending || block.is_opaque();
                    if !pending && block.emission() > 0 {
                        sources.push((i as u32, block.emission()));
                    }
                    solid[i] = opaque;
                    if sky && opaque {
                        floors[z * size[0] + x] = (y - min_y + 1) as usize;
                    }
                    sky &= !opaque;
                    if sky {
                        levels[i] = SKY;
                    }
                }
            }
        }
    }
    // Direct sky ends on an opaque cell: only horizontal borders can seed
    // indirect light. Compare column roof heights instead of queueing every
    // sky cell (most of the open volume needs no flood work at all).
    let mut propagated = 0;
    for z in 0..size[1] {
        for x in 0..size[0] {
            let a = z * size[0] + x;
            for (valid, b) in [(x + 1 < size[0], a + 1), (z + 1 < size[1], a + size[0])] {
                if !valid || floors[a] == floors[b] {
                    continue;
                }
                let (lo, hi) = if floors[a] < floors[b] {
                    (a, b)
                } else {
                    (b, a)
                };
                for y in floors[lo]..floors[hi] {
                    let n = hi * height + y;
                    if !solid[n] && levels[n] < SKY - 1 {
                        levels[n] = SKY - 1;
                        queue.push_back(n as u32);
                        propagated += 1;
                    }
                }
            }
        }
    }
    propagated += flood(&solid, &mut levels, &mut queue, size, height);
    // Avoid the second volume allocation and flood entirely without sources.
    // Reuse the drained skylight queue for the independent artificial channel.
    let mut block_levels = if sources.is_empty() {
        Vec::new()
    } else {
        vec![0; count]
    };
    for (i, emission) in sources {
        block_levels[i as usize] = emission;
        queue.push_back(i);
    }
    let block_propagated = flood(&solid, &mut block_levels, &mut queue, size, height);
    Skylight {
        min,
        size,
        min_y,
        max_y,
        world_height,
        levels,
        propagated,
        block_levels,
        block_propagated,
    }
}

fn flood(
    solid: &[bool],
    levels: &mut [u8],
    queue: &mut VecDeque<u32>,
    size: [usize; 2],
    height: usize,
) -> usize {
    let mut propagated = 0;
    while let Some(i) = queue.pop_front() {
        let i = i as usize;
        let value = levels[i].saturating_sub(1);
        if value == 0 {
            continue;
        }
        let y = i % height;
        let column = i / height;
        let x = column % size[0];
        let z = column / size[0];
        let neighbors = [
            (y > 0, i.wrapping_sub(1)),
            (y + 1 < height, i + 1),
            (x > 0, i.wrapping_sub(height)),
            (x + 1 < size[0], i + height),
            (z > 0, i.wrapping_sub(height * size[0])),
            (z + 1 < size[1], i + height * size[0]),
        ];
        for (valid, n) in neighbors {
            if valid && !solid[n] && levels[n] < value {
                levels[n] = value;
                queue.push_back(n as u32);
                propagated += 1;
            }
        }
    }
    propagated
}
