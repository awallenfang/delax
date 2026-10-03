use super::*;
use crate::delay_engine::engine::DelayEngine;

// ---------- shared helpers ----------

fn walk_order(jumps: &[Jump], size: usize) -> (Vec<usize>, usize) {
    let mut pos = 0;
    let mut order = Vec::with_capacity(size);
    for _ in 0..size {
        order.push(pos);
        pos = jumps
            .iter()
            .find(|j| j.from == pos)
            .map(|j| j.to)
            .unwrap_or(pos + 1);
    }
    (order, pos)
}

fn assert_in_bounds(jumps: &[Jump], size: usize) {
    for j in jumps {
        assert!(
            j.from < size && j.to < size,
            "jump ({}, {}) out of bounds for size {size}",
            j.from,
            j.to
        );
    }
}

fn assert_full_coverage(jumps: &[Jump], size: usize) {
    let (mut order, end) = walk_order(jumps, size);
    order.sort_unstable();
    assert_eq!(
        order,
        (0..size).collect::<Vec<_>>(),
        "table has gaps or repeats"
    );
    assert_eq!(end, 0, "table does not wrap back to 0");
}

fn engine_read_order(jumps: &[Jump], size: usize) -> Vec<usize> {
    let mut engine = DelayEngine::new(size, 44100.);
    for i in 0..size {
        engine.write_sample(i as f32);
    }
    engine.set_raw_read_jumps(jumps);
    (0..size).map(|_| engine.pop_sample() as usize).collect()
}

fn read_positions(jumps: &[Jump], size: usize, n: usize) -> Vec<usize> {
    let mut engine = DelayEngine::new(size, 44100.);
    for i in 0..size {
        engine.write_sample(i as f32);
    }
    engine.set_raw_read_jumps(jumps);
    (0..n).map(|_| engine.pop_sample() as usize).collect()
}

fn walk_table(jumps: &[Jump], size: usize, limit: usize) -> Vec<usize> {
    let mut pos = 0usize;
    let mut seen = Vec::with_capacity(size);
    for _ in 0..limit {
        if pos >= size || (!seen.is_empty() && pos == 0) {
            break;
        }
        seen.push(pos);
        pos = jumps
            .iter()
            .find(|j| j.from == pos)
            .map(|j| j.to)
            .unwrap_or(pos + 1);
    }
    seen
}

fn visit_order(size: usize, jumps: &[Jump]) -> Vec<usize> {
    JumpBuilder::from_jumps(size, jumps).play_order()
}

fn assert_ranks_match(builder: &JumpBuilder) {
    let jumps = builder.build();
    let segs = builder.segments();
    let ends: Vec<_> = segs.iter().map(|s| s.end).collect();
    for (pos, j) in jumps.iter().enumerate() {
        assert_eq!(j.rank, pos);
        let seg = ends.iter().position(|&e| e == j.from).unwrap();
        assert_eq!(segs[seg].order, pos);
    }
}

// ---------- JumpBuilder ----------

#[test]
fn empty_is_single_wrapping_cycle() {
    assert_eq!(JumpBuilder::empty(8).build(), vec![Jump::new(7, 0, 0)]);
    assert_eq!(JumpBuilder::empty(1).build(), vec![Jump::new(0, 0, 0)]);
    assert_eq!(
        JumpBuilder::from_jumps(6, &[]).build(),
        vec![Jump::new(5, 0, 0)]
    );

    let jumps = JumpBuilder::empty(5).build();
    assert_in_bounds(&jumps, 5);
    assert_full_coverage(&jumps, 5);
    assert_eq!(engine_read_order(&jumps, 5), vec![0, 1, 2, 3, 4]);

    let mut engine = DelayEngine::new(5, 44100.);
    for i in 0..5 {
        engine.write_sample(i as f32);
    }
    engine.set_raw_read_jumps(&jumps);
    for i in 0..5 {
        assert_eq!(engine.pop_sample() as usize, i);
    }
    assert_eq!(engine.pop_sample() as usize, 0);

    // build() returns an independent copy
    let mut builder = JumpBuilder::split_evenly(8, 2);
    let first = builder.build();
    builder.shuffle_seeded(42);
    assert_eq!(first, vec![Jump::new(3, 4, 0), Jump::new(7, 0, 1)]);
    assert_eq!(builder.build().len(), 2);
}

#[test]
fn split_evenly_shapes() {
    let jumps = JumpBuilder::split_evenly(12, 3).build();
    assert_eq!(
        jumps,
        vec![Jump::new(3, 4, 0), Jump::new(7, 8, 1), Jump::new(11, 0, 2)]
    );
    assert_eq!(engine_read_order(&jumps, 12), (0..12).collect::<Vec<_>>());

    let jumps = JumpBuilder::split_evenly(10, 3).build();
    assert_eq!(
        jumps,
        vec![Jump::new(2, 3, 0), Jump::new(5, 6, 1), Jump::new(9, 0, 2)]
    );
    assert_in_bounds(&jumps, 10);
    assert_full_coverage(&jumps, 10);

    let jumps = JumpBuilder::split_evenly(3, 5).build();
    assert_eq!(
        jumps,
        vec![Jump::new(0, 1, 0), Jump::new(1, 2, 1), Jump::new(2, 0, 2)]
    );
    assert_full_coverage(&jumps, 3);

    assert_eq!(
        JumpBuilder::split_evenly(6, 1).build(),
        JumpBuilder::empty(6).build()
    );
}

#[test]
fn constructors_reject_zero_size() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    assert!(catch_unwind(AssertUnwindSafe(|| JumpBuilder::empty(0))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| JumpBuilder::split_evenly(8, 0))).is_err());
}

#[test]
fn shuffle_properties() {
    // deterministic
    let a = JumpBuilder::split_evenly(12, 4).shuffle_seeded(123).build();
    let b = JumpBuilder::split_evenly(12, 4).shuffle_seeded(123).build();
    assert_eq!(a, b);

    // preserves coverage as a single cycle, keeps sources/destinations/ranks
    for seed in [0, 1, 2, 7, 12345] {
        let before = JumpBuilder::split_evenly(12, 4).build();
        let after = JumpBuilder::split_evenly(12, 4)
            .shuffle_seeded(seed)
            .build();
        assert_eq!(after.len(), before.len());
        assert_in_bounds(&after, 12);
        let mut from_before: Vec<_> = before.iter().map(|j| j.from).collect();
        let mut from_after: Vec<_> = after.iter().map(|j| j.from).collect();
        from_before.sort_unstable();
        from_after.sort_unstable();
        assert_eq!(from_before, from_after, "seed {seed}: sources changed");
        let mut to_before: Vec<_> = before.iter().map(|j| j.to).collect();
        let mut to_after: Vec<_> = after.iter().map(|j| j.to).collect();
        to_before.sort_unstable();
        to_after.sort_unstable();
        assert_eq!(to_before, to_after, "seed {seed}: destinations changed");
        let mut ranks: Vec<_> = after.iter().map(|j| j.rank).collect();
        ranks.sort_unstable();
        assert_eq!(ranks, vec![0, 1, 2, 3], "seed {seed}: stale rank");
        assert_full_coverage(&after, 12);
    }

    // changes playback order for at least one seed, without losing samples
    let linear = (0..12).collect::<Vec<_>>();
    let mut found = None;
    for seed in 1..=20u64 {
        let jumps = JumpBuilder::split_evenly(12, 4)
            .shuffle_seeded(seed)
            .build();
        let order = engine_read_order(&jumps, 12);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, linear, "seed {seed}: shuffle lost samples");
        if order != linear {
            found = Some(seed);
            break;
        }
    }
    assert!(found.is_some(), "no seed in 1..=20 reordered playback");

    // single segment is a noop
    let mut builder = JumpBuilder::empty(8);
    assert_eq!(builder.shuffle_seeded(99).build(), vec![Jump::new(7, 0, 0)]);

    // chainable and stays valid (seeded + unseeded)
    let jumps = JumpBuilder::split_evenly(12, 3).shuffle_seeded(7).build();
    assert_eq!(jumps.len(), 3);
    assert_in_bounds(&jumps, 12);
    assert_full_coverage(&jumps, 12);
    let jumps = JumpBuilder::split_evenly(16, 4).shuffle().build();
    assert_eq!(jumps.len(), 4);
    assert_in_bounds(&jumps, 16);
    assert_full_coverage(&jumps, 16);
}

#[test]
fn order_rewiring() {
    let jumps = JumpBuilder::split_evenly(12, 3).order(&[2, 1, 0]).build();
    assert_eq!(
        jumps,
        vec![Jump::new(3, 8, 0), Jump::new(11, 4, 1), Jump::new(7, 0, 2)]
    );
    let (order, end) = walk_order(&jumps, 12);
    assert_eq!(order, vec![0, 1, 2, 3, 8, 9, 10, 11, 4, 5, 6, 7]);
    assert_eq!(end, 0);

    // rotation is equivalent (0-anchored normalization)
    let a = JumpBuilder::split_evenly(12, 3).order(&[1, 2, 0]).build();
    let b = JumpBuilder::split_evenly(12, 3).order(&[0, 1, 2]).build();
    assert_eq!(a, b);
    assert_eq!(walk_order(&a, 12), walk_order(&b, 12));

    // invalid input keeps the table
    let mut builder = JumpBuilder::split_evenly(12, 3);
    let before = builder.build();
    builder.order(&[0, 1]);
    builder.order(&[0, 1, 1]);
    builder.order(&[0, 1, 3]);
    builder.order(&[]);
    assert_eq!(builder.build(), before);

    // single segment
    let mut builder = JumpBuilder::empty(8);
    assert_eq!(builder.order(&[0]).build(), vec![Jump::new(7, 0, 0)]);
    assert_eq!(builder.order(&[1]).build(), vec![Jump::new(7, 0, 0)]);
    assert_eq!(builder.order(&[]).build(), vec![Jump::new(7, 0, 0)]);

    // swap_positions helper
    let mut order = vec![0, 2, 3, 1];
    JumpBuilder::swap_positions(&mut order, 2, 1);
    assert_eq!(order, vec![0, 1, 3, 2]);
    JumpBuilder::swap_positions(&mut order, 1, 1);
    assert_eq!(order, vec![0, 1, 3, 2]);
    JumpBuilder::swap_positions(&mut order, 1, 9);
    assert_eq!(order, vec![0, 1, 3, 2]);
}

#[test]
fn order_rank_consistency() {
    for seed in [1, 7, 123] {
        let builder = JumpBuilder::split_evenly(12, 4).shuffle_seeded(seed);
        assert_ranks_match(&builder);
    }

    let mut builder = JumpBuilder::split_evenly(12, 4).shuffle_seeded(7);
    builder.order(&[3, 1, 0, 2]);
    let jumps = builder.build();
    let segs = builder.segments();
    assert_eq!(visit_order(12, &jumps), vec![0, 2, 3, 1]);
    for (s, seg) in segs.iter().enumerate() {
        let want = [0, 2, 3, 1].iter().position(|&x| x == s).unwrap();
        assert_eq!(seg.order, want);
    }
    assert_ranks_match(&builder);

    let jumps = JumpBuilder::split_evenly(12, 4)
        .shuffle_seeded(99)
        .order(&[3, 1, 0, 2])
        .build();
    assert_eq!(
        jumps,
        vec![
            Jump::new(2, 6, 0),
            Jump::new(8, 9, 1),
            Jump::new(11, 3, 2),
            Jump::new(5, 0, 3),
        ]
    );
}

#[test]
fn is_covering_accepts_and_rejects() {
    assert!(JumpBuilder::empty(8).is_covering());
    assert!(JumpBuilder::split_evenly(12, 3).is_covering());
    assert!(
        JumpBuilder::split_evenly(12, 4)
            .shuffle_seeded(7)
            .is_covering()
    );
    assert!(JumpBuilder::from_jumps(6, &[]).is_covering());

    assert!(!JumpBuilder::from_jumps(10, &[Jump::new(4, 0, 0)]).is_covering());
    assert!(!JumpBuilder::from_jumps(10, &[Jump::new(2, 2, 0)]).is_covering());
    assert!(!JumpBuilder::from_jumps(5, &[Jump::new(9, 0, 0)]).is_covering());
}

#[test]
fn scaled_linear_exact() {
    let scaled = JumpBuilder::split_evenly(12, 3).scaled(24).build();
    assert_eq!(scaled, JumpBuilder::split_evenly(24, 3).build());
    assert_eq!(
        scaled,
        vec![
            Jump::new(7, 8, 0),
            Jump::new(15, 16, 1),
            Jump::new(23, 0, 2)
        ]
    );

    let scaled = JumpBuilder::split_evenly(80, 8).scaled(160).build();
    assert_eq!(scaled, JumpBuilder::split_evenly(160, 8).build());
    assert_eq!(scaled.len(), 8);
    for (i, j) in scaled.iter().enumerate() {
        assert_eq!(*j, Jump::new((i + 1) * 20 - 1, (i + 1) * 20 % 160, i));
    }

    let scaled = JumpBuilder::split_evenly(12, 3).scaled(10).build();
    assert_eq!(scaled, JumpBuilder::split_evenly(10, 3).build());
    assert_eq!(
        scaled,
        vec![Jump::new(2, 3, 0), Jump::new(5, 6, 1), Jump::new(9, 0, 2)]
    );

    for seed in [1, 7, 123] {
        let before = JumpBuilder::split_evenly(12, 4).shuffle_seeded(seed);
        assert_eq!(before.scaled(12).build(), before.build());
    }

    let roundtrip = JumpBuilder::split_evenly(12, 3)
        .scaled(24)
        .scaled(12)
        .build();
    assert_eq!(roundtrip, JumpBuilder::split_evenly(12, 3).build());
}

#[test]
fn scaled_topology_preserving() {
    for seed in [1, 7, 123] {
        let before = JumpBuilder::split_evenly(12, 4).shuffle_seeded(seed);
        let before_jumps = before.build();
        let up = before.scaled(24).build();
        assert_in_bounds(&up, 24);
        assert_full_coverage(&up, 24);
        assert_eq!(visit_order(24, &up), visit_order(12, &before_jumps));

        let down = JumpBuilder::split_evenly(12, 4)
            .shuffle_seeded(seed)
            .scaled(10)
            .build();
        assert_in_bounds(&down, 10);
        assert_full_coverage(&down, 10);
        assert_eq!(
            visit_order(10, &down),
            visit_order(12, &before_jumps),
            "seed {seed}: topology changed"
        );
    }

    let scaled = JumpBuilder::split_evenly(12, 8).scaled(5).build();
    assert_eq!(scaled.len(), 3);
    assert_in_bounds(&scaled, 5);
    assert_full_coverage(&scaled, 5);

    assert_eq!(
        JumpBuilder::empty(8).scaled(3).build(),
        vec![Jump::new(2, 0, 0)]
    );
}

#[test]
fn jump_serde_accepts_both_forms() {
    let j = Jump::new(3, 8, 1);
    let v = serde_json::json!({"from": j.from, "to": j.to, "rank": j.rank});
    let back: Jump = serde_json::from_value(v).unwrap();
    assert_eq!(back, j);
    let legacy = serde_json::json!([3, 8, 1]);
    let back2: Jump = serde_json::from_value(legacy).unwrap();
    assert_eq!(back2, j);
}

// ---------- SegmentEditor ----------

#[test]
fn construction_invariants() {
    assert_eq!(
        SegmentEditor::single(8).build_jumps(),
        JumpBuilder::empty(8).build()
    );

    for (size, splits) in [(12usize, 3u32), (10, 3), (3, 5), (6, 1), (16, 4), (80, 8)] {
        assert_eq!(
            SegmentEditor::split_evenly(size, splits).build_jumps(),
            JumpBuilder::split_evenly(size, splits).build(),
            "size {size} splits {splits} diverged from JumpBuilder"
        );
    }

    let e = SegmentEditor::split_evenly(12, 3);
    assert_eq!(e.starts(), &[0, 4, 8]);
    assert_eq!(e.order(), &[0, 1, 2]);
    assert_eq!(e.portals().len(), 3);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    let jumps = e.build_jumps();
    assert_eq!(jumps.len(), 3);
    let segs = e.segments();
    let ends: Vec<usize> = segs.iter().map(|s| s.end).collect();
    let starts: Vec<usize> = segs.iter().map(|s| s.start).collect();
    for (rank, j) in jumps.iter().enumerate() {
        assert_eq!(j.rank, rank, "rank must be the playback slot index");
        assert_eq!(j.from, ends[e.order()[rank]]);
        assert_eq!(j.to, starts[e.order()[(rank + 1) % 3]]);
    }

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.swap_segments(0, 1);
    let segs = e.segments();
    assert_eq!(
        segs.iter().map(|s| s.order).collect::<Vec<_>>(),
        vec![0, 3, 1, 2]
    );
    assert_eq!((segs[0].start, segs[0].end), (0, 2));

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.swap_segments(0, 2);
    e.move_boundary(2, 5);
    assert!(e.validate_cycle());
    assert_full_coverage(&e.build_jumps(), 12);

    assert!(SegmentEditor::validate_table(
        8,
        &JumpBuilder::empty(8).build()
    ));
    assert!(SegmentEditor::validate_table(
        12,
        &JumpBuilder::split_evenly(12, 3).build()
    ));
    assert!(SegmentEditor::validate_table(
        12,
        &JumpBuilder::split_evenly(12, 4).shuffle_seeded(7).build()
    ));

    for splits in [1u32, 2, 3, 4, 8] {
        let e = SegmentEditor::split_evenly(24, splits);
        assert_eq!(
            e.validate_cycle(),
            SegmentEditor::validate_table(24, &e.build_jumps())
        );
    }
}

#[test]
fn validate_table_rejects_invalid() {
    assert!(!SegmentEditor::validate_table(5, &[Jump::new(9, 0, 0)]));
    assert!(!SegmentEditor::validate_table(5, &[Jump::new(4, 7, 0)]));
    assert!(!SegmentEditor::validate_table(
        10,
        &[Jump::new(3, 5, 0), Jump::new(3, 7, 1), Jump::new(9, 0, 2)]
    ));
    assert!(!SegmentEditor::validate_table(10, &[Jump::new(2, 2, 0)]));
    assert!(SegmentEditor::validate_table(
        10,
        &[Jump::new(1, 0, 0), Jump::new(9, 9, 1)]
    ));
    assert!(!SegmentEditor::validate_table(
        10,
        &[Jump::new(3, 1, 0), Jump::new(9, 0, 1)]
    ));
    assert!(!SegmentEditor::validate_table(8, &[]));
    assert!(!SegmentEditor::validate_table(0, &[Jump::new(0, 0, 0)]));
}

#[test]
fn visit_cycle_properties() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    let cycle = e.visit_cycle();
    let mut sorted = cycle.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..12).collect::<Vec<_>>());

    let mut e = SegmentEditor::split_evenly(16, 4);
    e.swap_segments(1, 3);
    let cycle = e.visit_cycle();
    assert_eq!(cycle[0], 0);
    assert!(cycle.iter().all(|&p| p < 16));

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    let jumps = e.build_jumps();
    let cycle = e.visit_cycle();
    assert_eq!(cycle, walk_table(&jumps, 12, 12));
    for pair in cycle.windows(2) {
        let expected = jumps
            .iter()
            .find(|j| j.from == pair[0])
            .map(|j| j.to)
            .unwrap_or(pair[0] + 1);
        assert_eq!(pair[1], expected);
    }

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    let segs = e.segments();
    let mut want = Vec::new();
    for &slot in e.order().iter() {
        for p in segs[slot].start..=segs[slot].end {
            want.push(p);
        }
    }
    assert_eq!(e.visit_cycle(), want);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 1);
    let cycle = e.visit_cycle();
    assert_eq!(cycle, vec![0, 1, 4, 5, 6, 7, 8, 9, 10, 11]);
    assert_eq!(cycle.len(), 10);
    assert!(!cycle.contains(&2) && !cycle.contains(&3));
    assert!(e.validate_cycle());

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_entry(1, 1);
    assert!(!e.validate_cycle());
    assert!(e.visit_cycle().len() <= 12);
}

#[test]
fn unglue_reweld_lifecycle() {
    let e = SegmentEditor::split_evenly(12, 3);
    let before = e.build_jumps();
    let mut unglued = e;
    unglued.unglue(1);
    assert_eq!(unglued.portals()[1], Some(Portal { exit: 3, entry: 4 }));
    assert!(unglued.is_unglued(1));
    assert_eq!(unglued.build_jumps(), before);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    assert!(e.is_unglued(1));
    assert!(!e.is_unglued(2));

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    e.unglue(1);
    let refused = e.portals()[1].is_none();
    assert!(
        refused || e.validate_cycle(),
        "unglue left duplicate `from`: {:?}",
        e.build_jumps()
    );

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 0);
    e.move_entry(1, 9);
    assert!(!JumpBuilder::from_jumps(12, &e.build_jumps()).is_covering());
    e.reweld(1);
    assert!(!e.is_unglued(1));
    assert_eq!(e.portals()[1], None);
    assert_full_coverage(&e.build_jumps(), 12);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_entry(1, 9);
    e.reweld(1);
    e.move_boundary(1, 5);
    assert_eq!(e.starts()[1], 5);
    assert_full_coverage(&e.build_jumps(), 12);
}

#[test]
fn move_boundary_properties() {
    let mut e = SegmentEditor::split_evenly(12, 4);
    e.move_boundary(1, 5);
    e.move_boundary(3, 7);
    e.move_boundary(2, 6);
    assert_eq!(e.starts(), &[0, 5, 6, 7]);
    assert!(e.validate_cycle());
    assert_full_coverage(&e.build_jumps(), 12);

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.move_boundary(2, 0);
    assert_eq!(e.starts()[2], 4);
    e.move_boundary(2, 11);
    assert_eq!(e.starts()[2], 8);

    let mut e = SegmentEditor::split_evenly(6, 3);
    e.move_boundary(1, 99);
    assert_eq!(e.starts(), &[0, 3, 4]);
    e.move_boundary(1, 0);
    assert_eq!(e.starts(), &[0, 1, 4]);
    assert_full_coverage(&e.build_jumps(), 6);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.move_boundary(0, 5);
    assert_eq!(e.starts()[0], 0);
}

#[test]
fn portal_entry_exit_behavior() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 99);
    assert_eq!(e.portals()[1].unwrap().exit, 3);
    e.move_exit(1, 0);
    assert_eq!(e.portals()[1].unwrap().exit, 0);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_entry(1, 10);
    assert_eq!(e.portals()[1].unwrap().entry, 10);
    e.move_entry(1, 11);
    assert_eq!(e.portals()[1].unwrap().entry, 11);
    e.move_entry(1, 99);
    assert_eq!(e.portals()[1].unwrap().entry, 11);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_entry(1, 6);
    assert_eq!(e.visit_cycle(), vec![0, 1, 2, 3, 6, 7, 8, 9, 10, 11]);
    assert!(!e.visit_cycle().contains(&4) && !e.visit_cycle().contains(&5));

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_entry(1, 0);
    assert_eq!(e.visit_cycle(), vec![0, 1, 2, 3]);
    assert!(e.validate_cycle());

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_entry(1, 1);
    assert!(!e.validate_cycle());
}

#[test]
fn swap_segments_properties() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    assert_eq!(e.order(), &[0, 2, 1]);
    assert_eq!(
        e.build_jumps(),
        vec![Jump::new(3, 8, 0), Jump::new(11, 4, 1), Jump::new(7, 0, 2)]
    );
    assert_eq!(e.visit_cycle(), vec![0, 1, 2, 3, 8, 9, 10, 11, 4, 5, 6, 7]);

    let mut e = SegmentEditor::split_evenly(12, 3);
    let before = e.starts().to_vec();
    e.swap_segments(0, 1);
    assert_eq!(e.starts(), before.as_slice());

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.swap_segments(0, 3);
    e.swap_segments(1, 2);
    assert!(e.validate_cycle());
    assert_full_coverage(&e.build_jumps(), 12);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 2);
    e.move_entry(1, 9);
    let portal = e.portals()[1];
    e.swap_segments(0, 1);
    assert_eq!(e.portals()[1], portal);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 2);
    e.move_entry(1, 9);
    assert!(e.validate_cycle());
    e.swap_segments(0, 1);
    assert_eq!(
        e.validate_cycle(),
        SegmentEditor::validate_table(12, &e.build_jumps())
    );
}

#[test]
fn split_segment_properties() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.split_segment(1);
    assert_eq!(e.starts(), &[0, 4, 6, 8]);
    assert_eq!(e.portals().len(), 4);
    assert_full_coverage(&e.build_jumps(), 12);

    let mut e = SegmentEditor::split_evenly(9, 3);
    e.split_segment(0);
    assert_eq!(e.starts(), &[0, 1, 3, 6]);
    assert_full_coverage(&e.build_jumps(), 9);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.split_segment(1);
    let order = e.order().to_vec();
    assert_eq!(order.len(), 4);
    let first = order.iter().position(|&s| s == 1).unwrap();
    let second_id = order[first + 1];
    assert_ne!(second_id, 1);
    let segs = e.segments();
    assert_eq!(segs[1].end + 1, segs[second_id].start);
    assert_eq!(segs[1].order, first);
    assert_eq!(segs[second_id].order, first + 1);

    let mut e = SegmentEditor::split_evenly(6, 3);
    e.move_boundary(1, 1);
    e.move_boundary(2, 2);
    assert_eq!(e.starts(), &[0, 1, 2]);
    e.split_segment(1);
    assert_eq!(e.starts(), &[0, 1, 2]);
    assert_full_coverage(&e.build_jumps(), 6);

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.unglue(2);
    e.unglue(3);
    e.split_segment(1);
    assert_eq!(e.starts(), &[0, 3, 4, 6, 9]);
    assert_eq!(e.portals().len(), 5);
    assert_eq!(e.portals()[2], None);
    assert!(e.is_unglued(3));
    assert!(e.is_unglued(4));
    assert!(e.validate_cycle());
}

#[test]
fn merge_segments_properties() {
    let mut e = SegmentEditor::split_evenly(12, 4);
    e.swap_segments(2, 3);
    assert_eq!(e.order(), &[0, 1, 3, 2]);
    e.merge_segments(2);
    assert_eq!(e.starts(), &[0, 3, 9]);
    assert_eq!(e.order(), &[0, 1, 2]);
    let segs = e.segments();
    assert_eq!((segs[1].start, segs[1].end), (3, 8));
    assert_eq!(segs[1].order, 1);
    assert_full_coverage(&e.build_jumps(), 12);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_entry(1, 9);
    let before = e.clone();
    e.merge_segments(1);
    assert_eq!(e.starts(), before.starts());
    assert_eq!(e.order(), before.order());
    assert_eq!(e.portals(), before.portals());

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.unglue(3);
    e.merge_segments(1);
    assert_eq!(e.starts(), &[0, 6, 9]);
    assert_eq!(e.portals().len(), 3);
    assert!(e.is_unglued(2));
    assert!(e.validate_cycle());
}

#[test]
fn preset_split_resets_state() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    e.preset_split(3);
    assert_eq!(e.order(), &[0, 1, 2]);
    assert_eq!(e.build_jumps(), JumpBuilder::split_evenly(12, 3).build());

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.unglue(1);
    e.unglue(2);
    e.preset_split(2);
    assert_eq!(e.portals(), &[None, None]);
    assert!(!e.is_unglued(1));

    let mut e = SegmentEditor::single(4);
    e.preset_split(99);
    assert_eq!(e.starts().len(), 4);
    assert_full_coverage(&e.build_jumps(), 4);
}

#[test]
fn scaled_welded_parity() {
    for (from, to) in [(12usize, 24usize), (12, 10), (16, 4), (80, 160), (12, 12)] {
        let mut a = SegmentEditor::split_evenly(from, 4);
        a.swap_segments(0, 2);
        assert_eq!(
            a.scaled(to).build_jumps(),
            JumpBuilder::split_evenly(from, 4)
                .swap_segments(0, 2)
                .scaled(to)
                .build(),
            "scaling {from} -> {to} diverged from JumpBuilder"
        );
    }

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.swap_segments(0, 3);
    let scaled = e.scaled(24);
    let expected: Vec<usize> = e
        .order()
        .iter()
        .map(|&id| {
            let target = e.starts()[id] * 24 / e.size();
            scaled.starts().iter().position(|&s| s == target).unwrap()
        })
        .collect();
    assert_eq!(scaled.order(), expected);
    assert!(scaled.validate_cycle());

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    assert_eq!(e.scaled(24).scaled(12).build_jumps(), e.build_jumps());
}

#[test]
fn scaled_portal_behavior() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 1);
    e.move_entry(1, 4);
    let scaled = e.scaled(24);
    let portal = scaled.portals()[1].expect("portal must survive 2x rescale");
    assert_eq!(portal.exit, 2);
    assert_eq!(portal.entry, 8);
    assert!(portal.exit < portal.entry);
    assert!(scaled.validate_cycle());
    assert!(scaled.visit_cycle().len() < 24);

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 3);
    e.move_entry(1, 5);
    let scaled = e.scaled(48);
    let segs = scaled.segments();
    let portal = scaled.portals()[1].unwrap();
    assert!((segs[0].start..=segs[0].end).contains(&portal.exit));

    let mut e = SegmentEditor::split_evenly(12, 4);
    e.unglue(1);
    e.unglue(2);
    e.unglue(3);
    let scaled = e.scaled(3);
    assert_eq!(scaled.starts(), &[0, 1, 2]);
    assert_eq!(scaled.portals().len(), 3);
    assert_eq!(scaled.portals().iter().filter(|p| p.is_some()).count(), 1);
    assert_eq!(scaled.portals()[0], None);
    assert!(scaled.validate_cycle());
}

#[test]
fn serde_roundtrips() {
    let mut e = SegmentEditor::split_evenly(12, 4);
    e.swap_segments(0, 2);
    e.move_boundary(1, 4);
    let json = serde_json::to_string(&e).unwrap();
    let back: SegmentEditor = serde_json::from_str(&json).unwrap();
    assert_eq!(back, e);
    assert_eq!(back.build_jumps(), e.build_jumps());

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 1);
    e.move_entry(1, 9);
    let json = serde_json::to_string(&e).unwrap();
    let back: SegmentEditor = serde_json::from_str(&json).unwrap();
    assert_eq!(back.portals()[1], Some(Portal { exit: 1, entry: 9 }));
    assert_eq!(back.build_jumps(), e.build_jumps());
}

#[test]
fn checked_accepts_valid_shapes_and_rotates_a_non_zero_order() {
    let e = SegmentEditor::checked(12, vec![0, 4, 8], vec![2, 0, 1], vec![None; 3]);
    assert!(e.is_some(), "a permutation in any rotation is valid");
    assert_eq!(
        e.unwrap().order(),
        &[0, 1, 2],
        "normalised to order[0] == 0"
    );
}

#[test]
fn checked_rejects_each_broken_invariant() {
    let good = vec![None; 3];
    assert!(SegmentEditor::checked(0, vec![0, 4, 8], vec![0, 1, 2], good.clone()).is_none());
    assert!(SegmentEditor::checked(12, vec![1, 4, 8], vec![0, 1, 2], good.clone()).is_none());
    assert!(SegmentEditor::checked(12, vec![0, 4, 4], vec![0, 1, 2], good.clone()).is_none());
    assert!(SegmentEditor::checked(12, vec![0, 4, 12], vec![0, 1, 2], good.clone()).is_none());
    assert!(SegmentEditor::checked(12, vec![0, 4, 8], vec![0, 0, 1], good.clone()).is_none());
    assert!(SegmentEditor::checked(12, vec![0, 4, 8], vec![0, 1, 9], good.clone()).is_none());
    assert!(SegmentEditor::checked(12, vec![0, 4, 8], vec![0, 1], good.clone()).is_none());
    assert!(SegmentEditor::checked(12, vec![0, 4, 8], vec![0, 1, 2], vec![None; 2]).is_none());
    let mut wrap_portal = good.clone();
    wrap_portal[0] = Some(Portal { exit: 0, entry: 11 });
    assert!(SegmentEditor::checked(12, vec![0, 4, 8], vec![0, 1, 2], wrap_portal).is_none());
}

#[test]
fn deserializing_a_corrupt_editor_degrades_instead_of_failing() {
    for bad in [
        r#"{"size":0,"starts":[],"order":[]}"#,
        r#"{"size":12,"starts":[4,8],"order":[0,1]}"#,
        r#"{"size":12,"starts":[0,4,4],"order":[0,1,2]}"#,
        r#"{"size":12,"starts":[0,4,99],"order":[0,1,2]}"#,
        r#"{"size":12,"starts":[0,4,8],"order":[0,0,1]}"#,
        r#"{"size":12,"starts":[0,4,8],"order":[0,1,2],"portals":[null]}"#,
        r#"{"size":12,"starts":[0,4,8],"order":[0,1,2],"portals":[{"exit":0,"entry":11},null,null]}"#,
        r#"{"size":12}"#,
        r#"{}"#,
    ] {
        let back: SegmentEditor = serde_json::from_str(bad).expect("must not error");
        assert!(back.size() > 0, "{bad} degraded to size 0");
        assert_eq!(back.portals().len(), back.starts().len(), "{bad}");
        assert!(
            back.portals()[0].is_none(),
            "{bad} left the wrap portal set"
        );
        assert!(
            back.validate_cycle(),
            "{bad} degraded to a table that does not play"
        );
    }
}

#[test]
fn from_jumps_recovers_and_falls_back() {
    let legacy = JumpBuilder::split_evenly(12, 3).build();
    let e = SegmentEditor::from_jumps(12, &legacy);
    assert_eq!(e.starts(), &[0, 4, 8]);
    assert_eq!(e.order(), &[0, 1, 2]);
    assert_eq!(e.portals(), &[None, None, None]);

    let legacy = JumpBuilder::split_evenly(12, 4).shuffle_seeded(7).build();
    let e = SegmentEditor::from_jumps(12, &legacy);
    assert_eq!(e.order(), JumpBuilder::from_jumps(12, &legacy).play_order());

    for (size, splits) in [(12usize, 3u32), (10, 3), (16, 4), (8, 2), (12, 1)] {
        for seed in [1u64, 7, 123] {
            let legacy = if splits == 1 {
                JumpBuilder::empty(size).build()
            } else {
                JumpBuilder::split_evenly(size, splits)
                    .shuffle_seeded(seed)
                    .build()
            };
            let e = SegmentEditor::from_jumps(size, &legacy);
            assert_eq!(
                e.build_jumps(),
                legacy,
                "size {size} splits {splits} seed {seed} did not round trip"
            );
        }
    }

    let e = SegmentEditor::from_jumps(16, &[]);
    assert_eq!(e.starts().len(), 8);
    assert!(e.validate_cycle());
    assert_full_coverage(&e.build_jumps(), 16);

    for broken in [
        vec![Jump::new(4, 0, 0)],
        vec![Jump::new(9, 0, 0)],
        vec![Jump::new(2, 3, 0), Jump::new(2, 5, 1)],
    ] {
        let e = SegmentEditor::from_jumps(10, &broken);
        assert!(
            e.validate_cycle(),
            "broken table must load playable: {broken:?}"
        );
        assert_in_bounds(&e.build_jumps(), 10);
    }
}

#[test]
fn engine_read_follows_visit_cycle() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.swap_segments(0, 1);
    assert_eq!(engine_read_order(&e.build_jumps(), 12), e.visit_cycle());

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_exit(1, 1);
    let cycle = e.visit_cycle();
    let got = read_positions(&e.build_jumps(), 12, cycle.len() * 2);
    assert_eq!(&got[..cycle.len()], cycle.as_slice());
    assert_eq!(&got[cycle.len()..], cycle.as_slice());

    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);
    e.move_entry(1, 6);
    assert_eq!(
        read_positions(&e.build_jumps(), 12, e.visit_cycle().len()),
        e.visit_cycle()
    );
}

#[test]
fn portal_accessors_ignore_the_inert_wrap_and_out_of_range_slots() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.portals[0] = Some(Portal { exit: 0, entry: 0 });
    e.move_exit(0, 5);
    e.move_entry(0, 5);
    assert_eq!(e.portals()[0], Some(Portal { exit: 0, entry: 0 }));

    e.unglue(1);
    e.move_exit(9, 5);
    e.move_entry(9, 5);
    assert_eq!(e.portals()[1], Some(Portal { exit: 3, entry: 4 }));
}

#[test]
fn the_last_segment_can_be_halved() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    assert!(e.can_split(2));
    assert!(e.split_segment(2));
    assert_eq!(e.starts(), &[0, 4, 8, 10]);
    assert_eq!(e.order().len(), 4);
    assert_full_coverage(&e.build_jumps(), 12);

    let segs = e.segments();
    let left = segs[2].order;
    assert_eq!(segs[3].order, left + 1);
    assert_eq!(segs[2].end + 1, segs[3].start);
}

#[test]
fn a_single_segment_can_still_be_halved() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    for boundary in [2, 1] {
        assert!(e.can_merge(boundary));
        assert!(e.merge_segments(boundary));
    }
    assert_eq!(e.starts(), &[0]);
    assert!(e.can_merge(1) == false, "nothing left to merge");

    assert!(e.can_split(0), "the lone segment must be splittable");
    assert!(e.split_segment(0));
    assert_eq!(e.starts(), &[0, 6]);
    assert_full_coverage(&e.build_jumps(), 12);

    for expected in [3, 4, 5] {
        let last = e.starts().len() - 1;
        assert!(e.split_segment(last));
        assert_eq!(e.starts().len(), expected);
    }
    assert_full_coverage(&e.build_jumps(), 12);
}

#[test]
fn a_one_sample_segment_still_refuses_to_halve() {
    let mut e = SegmentEditor::split_evenly(4, 4);
    for i in 0..4 {
        assert!(!e.can_split(i), "segment {i} is one sample wide");
        assert!(!e.split_segment(i));
    }
    assert_eq!(e.starts(), &[0, 1, 2, 3]);

    assert!(!e.can_split(4), "out of range");
    assert!(!e.split_segment(4));
    assert!(!e.split_segment(usize::MAX));
}

#[test]
fn out_of_range_boundaries_are_refused_rather_than_panicking() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    e.unglue(1);

    assert!(e.is_unglued(1));
    assert!(!e.is_unglued(9));
    assert!(!e.is_unglued(usize::MAX));

    assert!(e.can_reweld(1));
    assert!(e.reweld(1));
    assert!(!e.can_reweld(1), "already welded");
    assert!(!e.reweld(1), "rewelding twice is a no-op");
    assert!(!e.can_reweld(0), "the wrap carries no portal");
    assert!(!e.reweld(0));
    assert!(!e.reweld(9));
    assert!(!e.reweld(usize::MAX));
    assert_eq!(e.portals().len(), e.starts().len());
}

#[test]
fn predicates_predict_what_the_mutators_will_do() {
    let mut e = SegmentEditor::split_evenly(12, 3);
    for boundary in 0..=8 {
        let can = e.can_unglue(boundary);
        assert_eq!(can, e.unglue(boundary), "unglue({boundary})");
    }
    assert!(e.can_move_exit(1) && e.can_move_entry(1));
    assert!(!e.can_unglue(1), "already unglued");
    assert!(!e.unglue(1));
    assert!(!e.can_merge(1), "unglued borders cannot merge");
    assert!(!e.merge_segments(1));
    assert!(e.can_reweld(1) && e.reweld(1));
    assert!(e.can_merge(1) && e.merge_segments(1));

    for boundary in 0..=8 {
        let can = e.can_move_boundary(boundary);
        let before = e.starts().to_vec();
        assert_eq!(can, e.move_boundary(boundary, 6), "move({boundary})");
        if !can {
            assert_eq!(before, e.starts().to_vec(), "move({boundary}) mutated");
        }
    }
}
