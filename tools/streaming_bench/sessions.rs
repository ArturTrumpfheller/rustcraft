// SPDX-License-Identifier: MIT OR Apache-2.0
#[test]
#[ignore = "Matched session switches during active streaming; run via benchmark_streaming.py"]
fn slice32_session_measurements() {
    use crate::slice32_probe as probe;
    assert_eq!(
        voxel_core::streaming::slice32_fingerprint(),
        std::env::var("S32_EXPECT_CORE").unwrap()
    );
    assert_eq!(probe::SOURCE, std::env::var("S32_EXPECT_CLIENT").unwrap());
    let repeats: usize = std::env::var("S32_REPEATS").unwrap().parse().unwrap();
    for repeat in 0..repeats {
        for radius in 1..=3 {
            let mut app = app();
            let baseline = assets(&app);
            let dirs = [Directory::new(), Directory::new()];
            probe::reset();
            let observation = Instant::now();
            for i in 0..12 {
                let start = Instant::now();
                open(&mut app, &dirs[i % 2]);
                app.world_mut()
                    .resource_mut::<streaming::StreamState>()
                    .set_radius(radius);
                app.world_mut()
                    .resource_mut::<player::PlayerState>()
                    .body
                    .position = [1600.5, 24., -1600.5];
                frame(&mut app);
                leave(&mut app);
                probe::duration("session_switch_us", start.elapsed());
                assert_eq!(assets(&app), baseline);
                assert_eq!(
                    app.world_mut()
                        .query_filtered::<Entity, With<SessionEntity>>()
                        .iter(app.world())
                        .count(),
                    0
                );
            }
            probe::duration("observation_us", observation.elapsed());
            probe::report("session", radius, repeat);
            println!("S32_STATE,session,{radius},{repeat},{baseline:?}");
        }
    }
}
