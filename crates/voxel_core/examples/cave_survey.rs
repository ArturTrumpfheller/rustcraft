// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bounded six-connected cave survey, never used during gameplay.
use voxel_core::{
    Block, ChunkPos, LocalPos,
    worldgen::{GenerationConfig, Generator},
};
const N: usize = 256;
const H: usize = 84;
fn main() {
    println!(
        "seed,cave_cells,underground_cells,percent,components_ge64,largest,largest_percent,entrance_components,deep,mid,shallow,upper,mixed,uniform_stone"
    );
    for seed in [0, 42, 2026, u64::MAX] {
        let g = Generator::new(GenerationConfig { seed, version: 7 }).unwrap();
        let mut cells = vec![0u8; N * N * H];
        let mut total = 0usize;
        let mut air = 0usize;
        let mut depth = [0usize; 4];
        let mut mixed = 0;
        let mut stone = 0;
        for cz in -8..8 {
            for cx in -8..8 {
                let s = g.sampler([cx, cz]);
                for cy in -4..2 {
                    let c = s.chunk(ChunkPos([cx, cy, cz])).unwrap();
                    mixed += usize::from(c.uniform().is_none());
                    stone += usize::from(c.uniform() == Some(Block::Stone));
                    for z in 0..16 {
                        for x in 0..16 {
                            let wx = cx * 16 + x as i32;
                            let wz = cz * 16 + z as i32;
                            let h = g.height(wx, wz).unwrap();
                            for y in 0..16 {
                                let wy = cy * 16 + y as i32;
                                if wy < -60 || wy > h || wy >= 24 {
                                    continue;
                                }
                                total += 1;
                                if c.get(LocalPos::new([x, y, z]).unwrap()) == Block::Air {
                                    let i = ((wz + 128) as usize * N + (wx + 128) as usize) * H
                                        + (wy + 60) as usize;
                                    cells[i] = if wy == h { 3 } else { 1 };
                                    air += 1;
                                    depth[((wy + 60) / 20).min(3) as usize] += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut components = Vec::new();
        let mut entrance_components = 0;
        let mut queue = Vec::new();
        for start in 0..cells.len() {
            if cells[start] == 0 || cells[start] == 2 {
                continue;
            }
            let mut entrance = cells[start] == 3;
            cells[start] = 2;
            queue.clear();
            queue.push(start);
            let mut head = 0;
            let mut low = H;
            while head < queue.len() {
                let i = queue[head];
                head += 1;
                let y = i % H;
                let x = (i / H) % N;
                let z = i / (H * N);
                low = low.min(y);
                for (ok, j) in [
                    (y > 0, i.wrapping_sub(1)),
                    (y + 1 < H, i + 1),
                    (x > 0, i.wrapping_sub(H)),
                    (x + 1 < N, i + H),
                    (z > 0, i.wrapping_sub(H * N)),
                    (z + 1 < N, i + H * N),
                ] {
                    if ok && (cells[j] == 1 || cells[j] == 3) {
                        entrance |= cells[j] == 3;
                        cells[j] = 2;
                        queue.push(j);
                    }
                }
            }
            if queue.len() >= 64 {
                components.push(queue.len());
            }
            if entrance && low < 40 {
                entrance_components += 1;
            }
        }
        let largest = components.iter().max().copied().unwrap_or(0);
        println!(
            "{seed},{air},{total},{:.3},{},{largest},{:.2},{entrance_components},{},{},{},{},{mixed},{stone}",
            air as f64 * 100. / total as f64,
            components.len(),
            largest as f64 * 100. / air.max(1) as f64,
            depth[0],
            depth[1],
            depth[2],
            depth[3]
        );
    }
}
