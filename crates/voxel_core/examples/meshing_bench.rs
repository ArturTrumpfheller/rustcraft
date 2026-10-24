//! Reproducible mesh-only benchmark; shared lighting and world construction excluded.
use std::{collections::BTreeSet, hint::black_box, time::Instant};
use voxel_core::{
    Block, BlockPos, Chunk, ChunkPos, LocalPos, World, lighting::Skylight, mesh_chunk_lit,
};
fn fixture(kind: &str) -> World {
    let mut world = World::default();
    for cx in -1..=1 {
        for cz in -1..=1 {
            for cy in -1..=1 {
                let cp = ChunkPos([cx, cy, cz]);
                let mut chunk = Chunk::default();
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let p = cp
                                .block_position(LocalPos::new([x, y, z]).unwrap())
                                .unwrap();
                            let [wx, wy, wz] = p.0;
                            let b = match kind {
                                "solid" => {
                                    if cp == ChunkPos([0; 3]) {
                                        Block::Stone
                                    } else {
                                        Block::Air
                                    }
                                }
                                "flat" => {
                                    if wy < 4 {
                                        Block::Stone
                                    } else {
                                        Block::Air
                                    }
                                }
                                "mixed" => {
                                    let top = 2 + wx.rem_euclid(8) / 2 + wz.rem_euclid(6) / 3;
                                    if wy < top - 2 {
                                        Block::Stone
                                    } else if wy < top {
                                        Block::Dirt
                                    } else if wy == top {
                                        Block::Grass
                                    } else {
                                        Block::Air
                                    }
                                }
                                "cave" => {
                                    if wy < 12
                                        && !((2..8).contains(&wy)
                                            && (wx.rem_euclid(12) < 7 || wz.rem_euclid(12) < 5))
                                    {
                                        Block::Stone
                                    } else {
                                        Block::Air
                                    }
                                }
                                "trees" => {
                                    if wy < 0 {
                                        Block::Grass
                                    } else if wx.rem_euclid(8) == 3
                                        && wz.rem_euclid(8) == 3
                                        && wy < 6
                                    {
                                        Block::Wood
                                    } else if (4..8).contains(&wy)
                                        && (1..6).contains(&wx.rem_euclid(8))
                                        && (1..6).contains(&wz.rem_euclid(8))
                                    {
                                        Block::Leaves
                                    } else {
                                        Block::Air
                                    }
                                }
                                _ => {
                                    let g = voxel_core::worldgen::Generator::new(
                                        voxel_core::worldgen::GenerationConfig {
                                            seed: 2026,
                                            version: 3,
                                        },
                                    )
                                    .unwrap();
                                    g.block(p)
                                }
                            };
                            chunk.set(LocalPos::new([x, y, z]).unwrap(), b);
                        }
                    }
                }
                world.insert_chunk(cp, chunk);
            }
        }
    }
    world
}
fn main() {
    println!("scene,quads,vertices,triangles,payload_bytes,capacity_bytes,ns_per_batch");
    for scene in ["solid", "flat", "mixed", "cave", "trees", "edits", "stream"] {
        let mut world = fixture(scene);
        if scene == "edits" {
            for x in 0..16 {
                world
                    .set_block(
                        BlockPos([x, 3, 0]),
                        if x % 2 == 0 { Block::Stone } else { Block::Air },
                    )
                    .unwrap();
            }
        }
        let targets: BTreeSet<_> = if scene == "stream" {
            world.chunks().map(|(p, _)| p).collect()
        } else {
            BTreeSet::from([ChunkPos([0; 3])])
        };
        let light = Skylight::for_chunks(&world, &targets);
        let mut counts = [0; 5];
        for &p in &targets {
            let m = mesh_chunk_lit(&world, p, &light);
            counts[0] += m.indices.len() / 6;
            counts[1] += m.positions.len();
            counts[2] += m.indices.len() / 3;
            counts[3] += m.positions.len() * 56 + m.indices.len() * 4;
            counts[4] += m.positions.capacity() * 12
                + m.normals.capacity() * 12
                + m.uvs.capacity() * 8
                + m.colors.capacity() * 16
                + m.atlas_uvs.capacity() * 8
                + m.indices.capacity() * 4;
        }
        let mut samples = [0u128; 5];
        for sample in &mut samples {
            let mut elapsed = 0;
            for step in 0..100 {
                let edit_light;
                let light = if scene == "edits" {
                    world
                        .set_block(
                            BlockPos([step % 16, 3, 0]),
                            if step % 3 == 0 {
                                Block::Air
                            } else {
                                Block::Stone
                            },
                        )
                        .unwrap();
                    edit_light = Skylight::for_chunks(&world, &targets);
                    &edit_light
                } else {
                    &light
                };
                let now = Instant::now();
                for &p in &targets {
                    black_box(mesh_chunk_lit(black_box(&world), p, black_box(light)));
                }
                elapsed += now.elapsed().as_nanos();
            }
            *sample = elapsed / 100;
        }
        samples.sort_unstable();
        println!(
            "{scene},{},{},{},{},{},{}",
            counts[0], counts[1], counts[2], counts[3], counts[4], samples[2]
        );
    }
}
