use super::*;
fn select(
    s: &Scheduler,
    loads: &BTreeMap<Column, Instant>,
    dirty: &BTreeMap<ChunkPos, Instant>,
    motion: [f64; 2],
    now: Instant,
) -> Option<Work> {
    let mut player = voxel_core::physics::Player::new([-7.5, 24., -7.5]);
    player.velocity = [motion[0], 0., motion[1]];
    s.choose(
        &loads.keys().copied().collect(),
        loads,
        dirty,
        &BTreeMap::new(),
        &player,
        [-1, -1],
        now,
    )
}
#[test]
fn nearest_rings_precede_motion_and_equal_inputs_are_deterministic_at_signed_coordinates() {
    let now = Instant::now();
    let s = Scheduler::default();
    let loads = BTreeMap::from([([-2, -1], now), ([0, -1], now), ([1, -1], now)]);
    for _ in 0..16 {
        assert!(matches!(
            select(&s, &loads, &BTreeMap::new(), [6., 0.], now),
            Some(Work::Load([0, -1]))
        ));
        assert!(matches!(
            select(&s, &loads, &BTreeMap::new(), [-6., 0.], now),
            Some(Work::Load([-2, -1]))
        ));
    }
    let loads = BTreeMap::from([([-2, -1], now), ([1, -1], now)]);
    assert!(matches!(
        select(&s, &loads, &BTreeMap::new(), [1e9, 0.], now),
        Some(Work::Load([-2, -1]))
    ));
}
#[test]
fn both_classes_progress_under_continuous_loads_and_edit_bursts() {
    let now = Instant::now();
    let loads = BTreeMap::from([([0, -1], now)]);
    let dirty = BTreeMap::from([(ChunkPos([-1, 1, -1]), now)]);
    let mut scheduler = Scheduler::default();
    let mut counts = [0; 2];
    let mut last = None;
    let mut streak = 0;
    for _ in 0..120 {
        let work = select(&scheduler, &loads, &dirty, [5.612, 0.], now).unwrap();
        let class = usize::from(matches!(work, Work::Mesh(_)));
        counts[class] += 1;
        streak = if last == Some(class) { streak + 1 } else { 1 };
        assert!(streak <= 2);
        last = Some(class);
        scheduler.dispatched(&work);
    }
    assert!(counts.iter().all(|n| *n >= 40));
}

#[test]
fn radius_four_backlog_services_collision_first_and_cannot_starve_near_edits() {
    let now = Instant::now();
    let old = now - Duration::from_secs(1);
    let mut loads: BTreeMap<_, _> = streaming::desired_with_limit([-1, -1], 4, 2048)
        .into_iter()
        .map(|c| (c, old))
        .collect();
    assert_eq!(loads.len(), 81);
    // Even an aging turn must dispatch the missing body column first.
    let mut scheduler = Scheduler {
        loads: 3,
        ..Default::default()
    };
    let dirty = BTreeMap::from([(ChunkPos([-1, 1, -1]), old)]);
    let mut edits = 0;
    for service in 0..243 {
        let work = select(&scheduler, &loads, &dirty, [5.612, -5.612], now).unwrap();
        if service == 0 {
            assert!(matches!(work, Work::Load([-1, -1])));
        }
        if let Work::Load(c) = &work {
            loads.remove(c);
        } else {
            edits += 1;
        }
        if service == 2 {
            assert!(edits > 0, "near edits must run within three service turns");
        }
        scheduler.dispatched(&work);
        if loads.is_empty() {
            break;
        }
    }
    assert!(
        loads.is_empty(),
        "finite distant requests also converge under edit pressure"
    );
}
#[test]
fn aged_far_loads_and_independent_dirty_groups_receive_bounded_service() {
    let now = Instant::now();
    let old = now - Duration::from_secs(1);
    let loads = BTreeMap::from([([0, -1], now), ([-4, -1], old)]);
    let dirty = BTreeMap::from([(ChunkPos([-1, 1, -1]), now), (ChunkPos([-4, 1, -1]), old)]);
    let mut scheduler = Scheduler::default();
    let mut far_load = false;
    let mut far_mesh = false;
    for _ in 0..12 {
        let work = select(&scheduler, &loads, &dirty, [6., 0.], now).unwrap();
        far_load |= matches!(work, Work::Load([-4, -1]));
        far_mesh |= matches!(&work,Work::Mesh(t) if t.contains(&ChunkPos([-4,1,-1])));
        scheduler.dispatched(&work);
    }
    assert!(far_load && far_mesh);
}
#[test]
fn immediate_unknown_collision_columns_precede_aged_requests_and_backoff_blocks_whole_components() {
    let now = Instant::now();
    let scheduler = Scheduler {
        loads: 3,
        ..Default::default()
    };
    let loads = BTreeMap::from([([-1, -1], now), ([-4, -1], now - Duration::from_secs(1))]);
    assert!(matches!(
        select(&scheduler, &loads, &BTreeMap::new(), [9., 9.], now),
        Some(Work::Load([-1, -1]))
    ));
    let dirty = BTreeMap::from([
        (ChunkPos([-1, 1, -1]), now),
        (ChunkPos([0, 1, -1]), now),
        (ChunkPos([-3, 1, -1]), now),
    ]);
    let group = dirty_component(&dirty, [-7.5, 24., -7.5], false, now, |c| *c == [0, -1]);
    assert_eq!(group, BTreeSet::from([ChunkPos([-3, 1, -1])]));
}
