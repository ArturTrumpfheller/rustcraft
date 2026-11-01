// SPDX-License-Identifier: MIT OR Apache-2.0
// Appended to the same production streaming test module in both source exports.
#[test]
#[ignore = "Matched frame-paced headless streaming; run via tools/benchmark_streaming.py"]
fn slice32_streaming_measurements() {
    use crate::slice32_probe as probe;
    assert_eq!(
        voxel_core::streaming::slice32_fingerprint(),
        std::env::var("S32_EXPECT_CORE").unwrap()
    );
    assert_eq!(
        crate::slice32_probe::SOURCE,
        std::env::var("S32_EXPECT_CLIENT").unwrap()
    );
    let repeats: usize = std::env::var("S32_REPEATS").unwrap().parse().unwrap();
    let frames: usize = std::env::var("S32_FRAMES").unwrap().parse().unwrap();
    for repeat in 0..repeats {
        for radius in 1..=3 {
            for scene in [
                "stationary",
                "walking",
                "sprinting",
                "diagonal",
                "reversal",
                "teleport",
                "gated_teleport",
                "return",
                "forest",
                "coast",
                "cavern",
                "editing",
                "water",
                "radius",
            ] {
                let dir = RegionalDir::new();
                let observation = Instant::now();
                probe::reset();
                let mut app = large_app_with_generator(&dir.0, 9);
                app.world_mut()
                    .resource_mut::<StreamState>()
                    .set_radius(radius);
                let origin = match scene {
                    "forest" => [-136, -256],
                    "coast" => [6, 6],
                    "cavern" => [3, -4],
                    _ => [-28, 1],
                };
                go(&mut app, origin);
                if scene == "gated_teleport" {
                    let (entered, release) = probe::hold_next_worker();
                    app.world_mut().resource_mut::<PlayerState>().body.position =
                        [1600.5, 24., -1600.5];
                    app.update();
                    entered.recv_timeout(Duration::from_secs(10)).unwrap();
                    app.world_mut().resource_mut::<PlayerState>().body.position =
                        [-1600.5, 24., 1600.5];
                    app.update();
                    release.send(()).unwrap();
                    drain(&mut app);
                }
                let start = Instant::now();
                let initial = [
                    f64::from(origin[0] * 16) + 15.8,
                    24.,
                    f64::from(origin[1] * 16) + 15.8,
                ];
                for frame in 0..frames {
                    let speed = if scene == "walking" { 4.317 } else { 5.612 };
                    let seconds = frame as f64 / 240.;
                    let mut pos = initial;
                    let mut velocity = [0.; 3];
                    match scene {
                        "walking" | "sprinting" | "forest" | "coast" | "cavern" => {
                            pos[0] += seconds * speed;
                            velocity[0] = speed;
                        }
                        "diagonal" => {
                            for a in [0, 2] {
                                pos[a] += seconds * speed / 2f64.sqrt();
                                velocity[a] = speed / 2f64.sqrt();
                            }
                        }
                        "reversal" => {
                            let t = (frame % 48) as f64 / 240.;
                            let sign = if frame % 48 < 24 { 1. } else { -1. };
                            pos[0] += if sign > 0. {
                                t * speed
                            } else {
                                (0.2 - t) * speed
                            };
                            velocity[0] = sign * speed;
                        }
                        "teleport" => {
                            let c = if frame % 12 < 6 { [80, -80] } else { [-80, 80] };
                            pos[0] = f64::from(c[0] * 16) + 0.5;
                            pos[2] = f64::from(c[1] * 16) + 0.5;
                        }
                        "return" => {
                            pos[0] += if frame % 48 < 24 { 64. } else { 0. };
                        }
                        "radius" => {
                            if frame % 12 == 0 {
                                app.world_mut()
                                    .resource_mut::<StreamState>()
                                    .set_radius((frame / 12 % 3 + 1) as i32);
                            }
                        }
                        "editing" | "water" => {
                            pos[0] += seconds * speed;
                            velocity[0] = speed;
                            if frame % 4 == 0 {
                                let p = BlockPos([origin[0] * 16 + 15, 128, origin[1] * 16 + 8]);
                                let block = if frame % 8 == 0 {
                                    if scene == "water" {
                                        Block::WaterSource
                                    } else {
                                        Block::Wood
                                    }
                                } else {
                                    Block::Air
                                };
                                if app.world().resource::<VoxelWorld>().0.ready(p)
                                    && app.world().resource::<VoxelWorld>().0.block(p) != block
                                {
                                    edit(&mut app, p, block);
                                }
                            }
                        }
                        _ => {}
                    }
                    app.world_mut().resource_mut::<PlayerState>().body.position = pos;
                    app.world_mut().resource_mut::<PlayerState>().body.velocity = velocity;
                    let update = Instant::now();
                    app.update();
                    probe::duration("ecs_update_us", update.elapsed());
                    let deadline = start + Duration::from_secs_f64((frame + 1) as f64 / 240.);
                    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                }
                drain(&mut app);
                probe::duration("exploration_us", start.elapsed());
                probe::duration("observation_us", observation.elapsed());
                assert_textured_residency(&mut app);
                let state = app.world().resource::<StreamState>();
                let world = &app.world().resource::<VoxelWorld>().0;
                assert!(state.cache.columns() <= 81 && world.chunk_count() <= 49 * 16);
                let renders = app.world().resource::<ChunkRenders>();
                let surfaces = renders.0.len() + renders.1.len() + renders.2.len();
                assert_eq!(app.world().resource::<Assets<Mesh>>().len(), surfaces);
                let mut hash = 0xcbf29ce484222325u64;
                for (p, c) in world.chunks() {
                    for a in p.0 {
                        hash = (hash ^ a as u64).wrapping_mul(0x100000001b3);
                    }
                    for y in 0..16 {
                        for z in 0..16 {
                            for x in 0..16 {
                                hash = (hash
                                    ^ c.get(voxel_core::LocalPos::new([x, y, z]).unwrap()) as u64)
                                    .wrapping_mul(0x100000001b3);
                            }
                        }
                    }
                }
                for (metric, value) in [
                    ("completed", state.completed),
                    ("discarded", state.discarded),
                    ("resident_descriptors", world.chunk_count()),
                    ("voxel_bytes", world.allocated_voxel_bytes()),
                    ("mesh_assets", surfaces),
                    ("mesh_payload", state.mesh_bytes),
                    ("cache_columns", state.cache.columns()),
                    ("cache_bytes", state.cache.allocated_voxel_bytes()),
                    ("peak_pending", state.peak_pending),
                ] {
                    probe::value(metric, value as f64);
                }
                // S32_EXTRA_METRICS
                if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                    for (key, metric) in [
                        ("VmRSS:", "resident_rss_kib"),
                        ("VmHWM:", "process_high_water_rss_kib"),
                    ] {
                        if let Some(line) = status.lines().find(|l| l.starts_with(key)) {
                            probe::value(
                                metric,
                                line.split_whitespace().nth(1).unwrap().parse().unwrap(),
                            );
                        }
                    }
                }
                println!(
                    "S32_STATE,{scene},{radius},{repeat},{hash},{},{},{},{}",
                    world.chunk_count(),
                    world.allocated_voxel_bytes(),
                    surfaces,
                    state.mesh_bytes
                );
                probe::report(scene, radius, repeat);
                drop(app);
            }
        }
    }
}
