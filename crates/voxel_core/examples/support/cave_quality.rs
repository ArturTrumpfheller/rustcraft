// SPDX-License-Identifier: MIT OR Apache-2.0
//! Disposable bounded survey. Air topology and a conservative standing-player
//! floor graph are deliberately different measurements. No game-state changes.
use std::collections::VecDeque;
use voxel_core::{
    Block, ChunkPos, LocalPos,
    worldgen::{GenerationConfig, Generator},
};
pub const SEEDS: [u64; 4] = [0, 42, 2026, u64::MAX];
pub struct Volume {
    pub n: usize,
    pub cells: Vec<u8>,
    pub heights: Vec<i32>,
    pub uniform: [usize; 3],
    pub water: usize,
    pub leaks: usize,
}
impl Volume {
    pub fn new(seed: u64, version: u32, min: [i32; 2], n: usize) -> Self {
        assert!(n.is_multiple_of(16) && n <= 256 && min.iter().all(|v| v.rem_euclid(16) == 0));
        let g = Generator::new(GenerationConfig { seed, version }).unwrap();
        let mut v = Self {
            n,
            cells: vec![0; n * n * 256],
            heights: vec![0; n * n],
            uniform: [0; 3],
            water: 0,
            leaks: 0,
        };
        for cz in 0..n / 16 {
            for cx in 0..n / 16 {
                let s = g.sampler([min[0] / 16 + cx as i32, min[1] / 16 + cz as i32]);
                for z in 0..16 {
                    for x in 0..16 {
                        v.heights[(cz * 16 + z) * n + cx * 16 + x] = s
                            .biome_sample(
                                min[0] + (cx * 16 + x) as i32,
                                min[1] + (cz * 16 + z) as i32,
                            )
                            .unwrap()
                            .height;
                    }
                }
                for cy in -4..12 {
                    let cp = ChunkPos([min[0] / 16 + cx as i32, cy, min[1] / 16 + cz as i32]);
                    let c = s.chunk(cp).unwrap();
                    v.uniform[match c.uniform() {
                        Some(Block::Air) => 0,
                        Some(Block::Stone) => 1,
                        _ => 2,
                    }] += 1;
                    for z in 0..16 {
                        for x in 0..16 {
                            for y in 0..16 {
                                let yy = (cy * 16 + 64) as usize + y;
                                let xx = cx * 16 + x;
                                let zz = cz * 16 + z;
                                let b = c.get(LocalPos::new([x, y, z]).unwrap());
                                v.cells[(zz * n + xx) * 256 + yy] = if b == Block::Air {
                                    1
                                } else if b.is_solid() {
                                    0
                                } else {
                                    2
                                };
                                if b == Block::WaterSource {
                                    v.water += 1;
                                    let p = cp
                                        .block_position(LocalPos::new([x, y, z]).unwrap())
                                        .unwrap();
                                    for d in
                                        [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                                    {
                                        let b = s.block(p.offset(d).unwrap());
                                        if !b.is_solid() && b.fluid().is_none() {
                                            v.leaks += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        v
    }
    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        (z * self.n + x) * 256 + y
    }
    pub fn air(&self, x: usize, y: usize, z: usize) -> bool {
        y < 256 && self.cells[self.index(x, y, z)] == 1
    }
    pub fn underground(&self) -> Vec<u8> {
        self.cells
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                u8::from(
                    b == 1
                        && i % 256 >= 4
                        && i % 256 <= ((self.heights[i / 256] + 64).clamp(0, 255) as usize),
                )
            })
            .collect()
    }
    pub fn standing(&self) -> Vec<u8> {
        self.cells
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                let y = i % 256;
                u8::from(
                    b == 1
                        && (4..254).contains(&y)
                        && self.cells[i + 1] == 1
                        && self.cells[i - 1] == 0
                        && y < (self.heights[i / 256] + 64) as usize,
                )
            })
            .collect()
    }
    fn neighbors(&self, i: usize, standing: bool) -> Vec<usize> {
        let y = i % 256;
        let x = i / 256 % self.n;
        let z = i / (256 * self.n);
        let mut out = Vec::with_capacity(6);
        if !standing {
            if y > 0 {
                out.push(i - 1);
            }
            if y < 255 {
                out.push(i + 1);
            }
        }
        for (valid, j) in [
            (x > 0, i.wrapping_sub(256)),
            (x + 1 < self.n, i + 256),
            (z > 0, i.wrapping_sub(256 * self.n)),
            (z + 1 < self.n, i + 256 * self.n),
        ] {
            if valid {
                out.push(j);
                // Head clearance through a one-block jump/drop. We don't equate
                // arbitrary vertical air contact with a walkable edge.
                if standing && y < 253 && self.cells[i + 2] == 1 {
                    out.push(j + 1);
                }
                if standing && y > 0 && self.cells[j + 1] == 1 {
                    out.push(j - 1);
                }
            }
        }
        out
    }
    pub fn components(&self, mut mask: Vec<u8>, standing: bool) -> Vec<Component> {
        let mut result = Vec::new();
        let mut queue = VecDeque::new();
        for i in 0..mask.len() {
            if mask[i] == 0 {
                continue;
            }
            mask[i] = 0;
            queue.push_back(i);
            let mut c = Component {
                cells: 0,
                min: [usize::MAX; 3],
                max: [0; 3],
                surface: false,
            };
            while let Some(j) = queue.pop_front() {
                c.cells += 1;
                let p = [j / 256 % self.n, j % 256, j / (256 * self.n)];
                for (k, &v) in p.iter().enumerate() {
                    c.min[k] = c.min[k].min(v);
                    c.max[k] = c.max[k].max(v);
                }
                c.surface |= p[1] as i32 >= self.heights[j / 256] + 63;
                for next in self.neighbors(j, standing) {
                    if mask[next] != 0 {
                        mask[next] = 0;
                        queue.push_back(next);
                    }
                }
            }
            result.push(c);
        }
        result.sort_by_key(|c| std::cmp::Reverse(c.cells));
        result
    }
    /// Dry Air/solid faces below the ordinary roof guard, in unit block².
    /// Window boundaries are excluded, so this is a clipped lower bound.
    pub fn interior_area(&self) -> usize {
        self.underground()
            .iter()
            .enumerate()
            .filter(|(i, b)| **b != 0 && (*i % 256) as i32 <= self.heights[*i / 256] + 56)
            .map(|(i, _)| {
                self.neighbors(i, false)
                    .into_iter()
                    .filter(|&n| self.cells[n] == 0)
                    .count()
            })
            .sum()
    }
    /// Two-sweep shortest-path lower bound for the largest clipped component.
    pub fn geodesic(&self, mask: Vec<u8>, standing: bool) -> usize {
        let flood = |start: usize, visited: &mut [u8]| {
            let mut q = VecDeque::from([(start, 0usize)]);
            visited[start] = 0;
            let mut far = (start, 0);
            let mut count = 0;
            while let Some((i, d)) = q.pop_front() {
                count += 1;
                if d > far.1 {
                    far = (i, d);
                }
                for n in self.neighbors(i, standing) {
                    if visited[n] != 0 {
                        visited[n] = 0;
                        q.push_back((n, d + 1));
                    }
                }
            }
            (far, count)
        };
        let mut visited = mask.clone();
        let mut best = (0, 0);
        for i in 0..visited.len() {
            if visited[i] != 0 {
                let (far, count) = flood(i, &mut visited);
                if count > best.1 {
                    best = (far.0, count);
                }
            }
        }
        if best.1 == 0 {
            0
        } else {
            flood(best.0, &mut mask.clone()).0.1
        }
    }
    pub fn widths(&self) -> ([usize; 6], [usize; 6]) {
        let mut widths = [0; 6];
        let mut heights = [0; 6];
        for (i, v) in self.standing().iter().enumerate() {
            if *v == 0 {
                continue;
            }
            let x = i / 256 % self.n;
            let z = i / (256 * self.n);
            let y = i % 256;
            let run = |dx: isize, dz: isize| {
                let mut d = 0;
                for step in 1..=64 {
                    let xx = x as isize + dx * step;
                    let zz = z as isize + dz * step;
                    if xx < 0
                        || zz < 0
                        || xx >= self.n as isize
                        || zz >= self.n as isize
                        || !self.air(xx as usize, y, zz as usize)
                    {
                        break;
                    }
                    d += 1;
                }
                d
            };
            let width = (run(-1, 0) + run(1, 0) + 1).min(run(0, -1) + run(0, 1) + 1);
            let height = (y..256).take_while(|&yy| self.air(x, yy, z)).count();
            let bin = |v| {
                if v <= 2 {
                    0
                } else if v <= 4 {
                    1
                } else if v <= 8 {
                    2
                } else if v <= 16 {
                    3
                } else if v <= 32 {
                    4
                } else {
                    5
                }
            };
            widths[bin(width)] += 1;
            heights[bin(height)] += 1;
        }
        (widths, heights)
    }
}
#[derive(Debug)]
pub struct Component {
    pub cells: usize,
    pub min: [usize; 3],
    pub max: [usize; 3],
    pub surface: bool,
}
pub fn mountain(seed: u64) -> [i32; 2] {
    let mut best = ([-512, -3200], 0);
    for z in (-4096..4096).step_by(128) {
        for x in (-4096..4096).step_by(128) {
            let h = voxel_core::worldgen::terrain_v5::height(seed, x, z);
            if h > best.1 {
                best = ([x, z], h);
            }
        }
    }
    best.0.map(|v| (v - 64).div_euclid(16) * 16)
}
