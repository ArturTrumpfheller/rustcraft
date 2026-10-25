use super::*;
use bevy::camera::CameraProjection;

#[test]
fn radius_bounds_cover_the_guaranteed_streamed_circle_and_preserve_interaction_range() {
    for radius in 1..=3 {
        let a = AtmosphereUniform::for_radius(radius);
        assert_eq!(a.fog.y, radius as f32 * voxel_core::CHUNK_SIZE as f32);
        assert!(a.fog.x > 5. && a.fog.x < a.fog.y);
        // Closest edge of the loaded square is at least radius*16 away at
        // every fraction of the center chunk, including negative coordinates.
        for position in [-16.001f64, -16., -0.001, 0., 0.001, 15.999, 16., 16000.5] {
            let fraction = position.rem_euclid(16.);
            let nearest = radius as f64 * 16. + fraction.min(16. - fraction);
            assert!(f64::from(a.fog.y) <= nearest);
        }
    }
    assert_eq!(
        AtmosphereUniform::for_radius(i32::MIN),
        AtmosphereUniform::for_radius(1)
    );
    assert_eq!(
        AtmosphereUniform::for_radius(i32::MAX),
        AtmosphereUniform::for_radius(3)
    );
}

#[test]
fn defaults_are_deterministic_finite_and_do_not_depend_on_world_coordinates() {
    let a = AtmosphereUniform::for_radius(2);
    assert_eq!(a, AtmosphereUniform::for_radius(2));
    assert_eq!(std::mem::size_of::<AtmosphereUniform>(), 80);
    for v in [a.zenith, a.horizon, a.fog] {
        assert!(v.is_finite());
    }
    assert_eq!(a.fog.z, 0.0);
    assert_eq!(a.fog.w, 0.8);
    for normal in [[0, 1, 0], [0, -1, 0], [1, 0, 0], [0, 0, 1]] {
        assert!(voxel_core::lighting::face_color(0, normal)[0] < 0.06);
    }
    assert!(AtmosphereUniform::disabled().fog.y <= AtmosphereUniform::disabled().fog.x);
}

#[test]
fn sky_rays_ignore_camera_translation_and_rotate_without_azimuth_seams() {
    let view_from_clip = PerspectiveProjection {
        fov: 45f32.to_radians(),
        aspect_ratio: 1.5,
        ..default()
    }
    .get_clip_from_view()
    .inverse();
    for pitch in [-1.4f32, -0.2, 0., 0.8, 1.4] {
        for yaw in [-std::f32::consts::PI, 0., std::f32::consts::PI] {
            let rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.);
            let ray = view_from_clip * Vec4::new(0.25, 0.5, 1., 1.);
            let local = ray.truncate().extend(0.);
            let expected = (Mat4::from_quat(rotation) * local).truncate().normalize();
            for translation in [
                Vec3::ZERO,
                Vec3::new(-32760., 5., 32760.),
                Vec3::new(16000., -8., -16000.),
            ] {
                let actual = (Mat4::from_rotation_translation(rotation, translation) * local)
                    .truncate()
                    .normalize();
                assert!((actual - expected).length() < 1e-6);
            }
        }
    }
}

#[test]
fn sky_pipeline_opts_out_of_prepasses_and_shadows() {
    assert!(!SkyMaterial::enable_prepass());
    assert!(!SkyMaterial::enable_shadows());
}

#[test]
fn unchanged_uniforms_do_not_modify_assets_and_radius_changes_remain_bounded() {
    use crate::terrain_material::{AtlasRepeat, TerrainSurface};
    use std::{hint::black_box, time::Instant};
    let mut terrain = Assets::<TerrainSurface>::default();
    let mut sky = Assets::<SkyMaterial>::default();
    let initial = AtmosphereUniform::for_radius(2);
    let t = terrain.add(TerrainSurface {
        base: StandardMaterial::default(),
        extension: AtlasRepeat {
            atlas: default(),
            atmosphere: initial,
        },
    });
    let s = sky.add(SkyMaterial {
        atmosphere: initial,
    });
    for varying in [false, true] {
        let mut timings = [0u128; 5];
        for sample in &mut timings {
            let mut writes = 0;
            let now = Instant::now();
            for step in 0..50_000 {
                let radius = if varying { 1 + (step / 128) % 3 } else { 2 };
                writes += black_box(update_materials(
                    AtmosphereUniform::for_radius(radius),
                    &t,
                    Some(&s),
                    &mut terrain,
                    &mut sky,
                ));
            }
            *sample = now.elapsed().as_nanos() / 50_000;
            if !varying {
                assert_eq!(writes, 0);
            } else {
                assert!(writes <= 2 * 392);
            }
        }
        timings.sort_unstable();
        println!(
            "ATMOSPHERE_UPDATE varying={varying} ns_per_call={} terrain_assets={} sky_assets={}",
            timings[2],
            terrain.len(),
            sky.len()
        );
        assert_eq!(terrain.len(), 1);
        assert_eq!(sky.len(), 1);
    }
}

#[test]
fn complete_clock_cycles_only_update_two_retained_uniforms() {
    use crate::terrain_material::{AtlasRepeat, TerrainSurface};
    let mut terrain = Assets::<TerrainSurface>::default();
    let mut sky = Assets::<SkyMaterial>::default();
    let initial = AtmosphereUniform::for_radius(2);
    let t = terrain.add(TerrainSurface {
        base: default(),
        extension: AtlasRepeat {
            atlas: default(),
            atmosphere: initial,
        },
    });
    let s = sky.add(SkyMaterial {
        atmosphere: initial,
    });
    let mut clock = voxel_core::time::WorldTime::default();
    for _ in 0..24000 {
        clock.advance(3);
        let a = AtmosphereUniform::at_time(2, clock);
        assert_eq!(a.sun.truncate(), Vec3::from_array(clock.sun_direction()));
        assert_eq!(a.sun.w, clock.daylight());
        for v in [a.zenith, a.horizon, a.fog, a.sun, a.celestial] {
            assert!(v.is_finite());
        }
        assert!(update_materials(a, &t, Some(&s), &mut terrain, &mut sky) <= 2);
        assert_eq!(update_materials(a, &t, Some(&s), &mut terrain, &mut sky), 0);
    }
    assert_eq!(terrain.len(), 1);
    assert_eq!(sky.len(), 1);
    assert_eq!(terrain.get(&t).unwrap().extension.atmosphere, initial);
}

#[test]
#[ignore = "CPU-only clock/uniform cost measurement; run alone with --ignored --nocapture"]
fn clock_uniform_cpu_cost() {
    use crate::terrain_material::{AtlasRepeat, TerrainSurface};
    use std::{hint::black_box, time::Instant};
    let mut terrain = Assets::<TerrainSurface>::default();
    let mut sky = Assets::<SkyMaterial>::default();
    let initial = AtmosphereUniform::for_radius(2);
    let t = terrain.add(TerrainSurface {
        base: default(),
        extension: AtlasRepeat {
            atlas: default(),
            atmosphere: initial,
        },
    });
    let s = sky.add(SkyMaterial {
        atmosphere: initial,
    });
    for radius in 1..=3 {
        let mut clock = voxel_core::time::WorldTime::default();
        let mut costs = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..100_000 {
                clock.advance(1);
                let p = black_box(AtmosphereUniform::at_time(radius, clock));
                black_box(update_materials(p, &t, Some(&s), &mut terrain, &mut sky));
            }
            costs.push(start.elapsed().as_nanos() / 100_000);
        }
        costs.sort_unstable();
        println!(
            "CLOCK_UNIFORM radius={radius} ns_per_tick_and_two_uniforms={} terrain_assets={} sky_assets={}",
            costs[3],
            terrain.len(),
            sky.len()
        );
    }
}
