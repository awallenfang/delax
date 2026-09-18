use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{SeedableRng, rng};

/// A jump inside of the banks. Currently this holds `Jump(from, to, order)`.
/// Both are inclusive, so with `Jump(10,100, 0)` the read order will be 8,9,10,100
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Jump(pub usize, pub usize, pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JumpSegment {
    pub start: usize,
    pub end: usize,
    pub order: usize,
}
#[derive(Clone, Debug)]
pub struct JumpBuilder {
    size: usize,
    jumps: Vec<Jump>,
}

impl JumpBuilder {
    pub fn empty(size: usize) -> Self {
        assert!(size > 0);
        JumpBuilder {
            size,
            jumps: vec![Jump(size - 1, 0, 0)],
        }
    }

    pub fn split_evenly(size: usize, splits: u32) -> Self {
        assert!(size > 0);
        assert!(splits > 0);
        let splits = (splits as usize).min(size);
        let width = size / splits;
        let mut jumps = Vec::with_capacity(splits);
        for i in 0..splits {
            let last = i + 1 == splits;
            let end = if last { size - 1 } else { (i + 1) * width - 1 };
            let next_start = if last { 0 } else { (i + 1) * width };
            jumps.push(Jump(end, next_start, i));
        }
        JumpBuilder { size, jumps }
    }

    pub fn shuffle(&mut self) -> Self {
        self.shuffle_with(&mut rng())
    }

    pub fn shuffle_seeded(&mut self, seed: u64) -> Self {
        self.shuffle_with(&mut StdRng::seed_from_u64(seed))
    }

    fn shuffle_with(&mut self, rng: &mut impl rand::Rng) -> Self {
        let n = self.jumps.len();
        if n > 1 {
            let starts = self.segment_starts();
            let ends = Self::segment_ends(&starts, self.size);
            let mut order: Vec<usize> = (0..n).collect();
            order.shuffle(rng);
            self.jumps = (0..n)
                .map(|i| Jump(ends[order[i]], starts[order[(i + 1) % n]], i))
                .collect();
        }
        self.clone()
    }

    pub fn build(&self) -> Vec<Jump> {
        self.jumps.clone()
    }

    pub fn from_jumps(size: usize, jumps: Vec<Jump>) -> Self {
        assert!(size > 0);
        if jumps.is_empty() {
            return Self::empty(size);
        }
        JumpBuilder { size, jumps }
    }

    pub fn scaled(&self, new_size: usize) -> Self {
        assert!(new_size > 0);
        if new_size == self.size {
            return self.clone();
        }
        let starts = self.segment_starts();
        let order = self.cycle_order(&starts);

        let scaled: Vec<usize> = starts.iter().map(|s| s * new_size / self.size).collect();
        let mut new_starts = Vec::with_capacity(scaled.len());
        for s in scaled {
            if new_starts.last() != Some(&s) {
                new_starts.push(s);
            }
        }
        if new_starts.len() <= 1 {
            return Self::empty(new_size);
        }
        let idx: HashMap<usize, usize> = new_starts
            .iter()
            .enumerate()
            .map(|(i, &s)| (s, i))
            .collect();
        let mut seen = vec![false; new_starts.len()];
        let mut new_order = Vec::with_capacity(order.len());
        for &s in &order {
            let m = idx[&(starts[s] * new_size / self.size)];
            if !seen[m] {
                seen[m] = true;
                new_order.push(m);
            }
        }
        if new_order.len() <= 1 {
            return Self::empty(new_size);
        }
        let ends = Self::segment_ends(&new_starts, new_size);
        let k = new_order.len();
        let jumps = (0..k)
            .map(|i| Jump(ends[new_order[i]], new_starts[new_order[(i + 1) % k]], i))
            .collect();
        JumpBuilder {
            size: new_size,
            jumps,
        }
    }

    fn segment_ends(starts: &[usize], size: usize) -> Vec<usize> {
        let n = starts.len();
        let mut ends = Vec::with_capacity(n);
        for i in 0..n {
            ends.push(if i + 1 < n {
                starts[i + 1].saturating_sub(1)
            } else {
                size - 1
            });
        }
        ends
    }

    fn cycle_order(&self, starts: &[usize]) -> Vec<usize> {
        let n = starts.len();
        if n <= 1 {
            return (0..n).collect();
        }
        let ends = Self::segment_ends(starts, self.size);
        let next: HashMap<usize, usize> = self.jumps.iter().map(|j| (j.0, j.1)).collect();
        let seg_of: HashMap<usize, usize> =
            starts.iter().enumerate().map(|(i, &s)| (s, i)).collect();
        let mut order = vec![0];
        let mut visited = vec![false; n];
        visited[0] = true;
        for _ in 0..n {
            let cur = *order.last().unwrap();
            let dest = match next.get(&ends[cur]) {
                Some(&d) => d,
                None => break,
            };
            let nxt = match seg_of.get(&dest) {
                Some(&i) => i,
                None => break,
            };
            if visited[nxt] {
                break;
            }
            visited[nxt] = true;
            order.push(nxt);
        }
        for i in 0..n {
            if !visited[i] {
                order.push(i);
            }
        }
        order
    }

    fn segment_starts(&self) -> Vec<usize> {
        let n = self.jumps.len();
        let mut starts: Vec<usize> = self.jumps.iter().map(|j| j.1).collect();
        starts.sort_unstable();
        starts.dedup();
        let valid = starts.len() == n && starts[0] == 0 && starts.iter().all(|&s| s < self.size);
        if valid {
            starts
        } else {
            let width = (self.size / n.max(1)).max(1);
            (0..n).map(|i| (i * width).min(self.size - 1)).collect()
        }
    }

    pub fn segments(&self) -> Vec<JumpSegment> {
        let starts = self.segment_starts();
        let order = self.cycle_order(&starts);
        let ends = Self::segment_ends(&starts, self.size);
        let rank: HashMap<usize, usize> = order.iter().enumerate().map(|(r, &s)| (s, r)).collect();
        (0..starts.len())
            .map(|i| JumpSegment {
                start: starts[i],
                end: ends[i],
                order: rank[&i],
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delay_engine::engine::DelayEngine;

    fn walk_order(jumps: &[Jump], size: usize) -> (Vec<usize>, usize) {
        let next: std::collections::HashMap<usize, usize> =
            jumps.iter().map(|j| (j.0, j.1)).collect();
        let mut pos = 0;
        let mut order = Vec::with_capacity(size);
        for _ in 0..size {
            order.push(pos);
            pos = *next.get(&pos).unwrap_or(&(pos + 1));
        }
        (order, pos)
    }

    fn assert_in_bounds(jumps: &[Jump], size: usize) {
        for j in jumps {
            assert!(
                j.0 < size && j.1 < size,
                "jump ({}, {}) out of bounds for size {size}",
                j.0,
                j.1
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

    #[test]
    fn empty_produces_single_wrap_jump() {
        assert_eq!(JumpBuilder::empty(8).build(), vec![Jump(7, 0, 0)]);
        assert_eq!(JumpBuilder::empty(1).build(), vec![Jump(0, 0, 0)]);
    }

    #[test]
    fn empty_traversal_visits_every_sample_once() {
        let jumps = JumpBuilder::empty(5).build();
        assert_in_bounds(&jumps, 5);
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
    }

    #[test]
    fn split_evenly_divisible_creates_linear_segments() {
        let jumps = JumpBuilder::split_evenly(12, 3).build();
        assert_eq!(jumps, vec![Jump(3, 4, 0), Jump(7, 8, 1), Jump(11, 0, 2)]);
        assert_eq!(engine_read_order(&jumps, 12), (0..12).collect::<Vec<_>>());
    }

    #[test]
    fn split_evenly_with_remainder_absorbs_tail() {
        let jumps = JumpBuilder::split_evenly(10, 3).build();
        assert_eq!(jumps, vec![Jump(2, 3, 0), Jump(5, 6, 1), Jump(9, 0, 2)]);
        assert_in_bounds(&jumps, 10);
        assert_full_coverage(&jumps, 10);
    }

    #[test]
    fn split_evenly_more_splits_than_samples_clamps() {
        let jumps = JumpBuilder::split_evenly(3, 5).build();
        assert_eq!(jumps, vec![Jump(0, 1, 0), Jump(1, 2, 1), Jump(2, 0, 2)]);
        assert_full_coverage(&jumps, 3);
    }

    #[test]
    fn split_evenly_single_split_matches_empty() {
        assert_eq!(
            JumpBuilder::split_evenly(6, 1).build(),
            JumpBuilder::empty(6).build()
        );
    }

    #[test]
    #[should_panic]
    fn split_evenly_zero_splits_panics() {
        let _ = JumpBuilder::split_evenly(8, 0);
    }

    #[test]
    #[should_panic]
    fn empty_zero_size_panics() {
        let _ = JumpBuilder::empty(0);
    }

    #[test]
    fn build_returns_independent_copy() {
        let mut builder = JumpBuilder::split_evenly(8, 2);
        let first = builder.build();
        builder.shuffle_seeded(42);
        assert_eq!(first, vec![Jump(3, 4, 0), Jump(7, 0, 1)]);
        assert_eq!(builder.build().len(), 2);
    }

    #[test]
    fn shuffle_seeded_is_deterministic() {
        let a = JumpBuilder::split_evenly(12, 4).shuffle_seeded(123).build();
        let b = JumpBuilder::split_evenly(12, 4).shuffle_seeded(123).build();
        assert_eq!(a, b);
    }

    #[test]
    fn shuffle_preserves_coverage_as_single_cycle() {
        for seed in [0, 1, 2, 7, 12345] {
            let before = JumpBuilder::split_evenly(12, 4).build();
            let after = JumpBuilder::split_evenly(12, 4)
                .shuffle_seeded(seed)
                .build();
            assert_eq!(after.len(), before.len());
            assert_in_bounds(&after, 12);
            let mut from_before: Vec<_> = before.iter().map(|j| j.0).collect();
            let mut from_after: Vec<_> = after.iter().map(|j| j.0).collect();
            from_before.sort_unstable();
            from_after.sort_unstable();
            assert_eq!(from_before, from_after, "seed {seed}: sources changed");
            let mut to_before: Vec<_> = before.iter().map(|j| j.1).collect();
            let mut to_after: Vec<_> = after.iter().map(|j| j.1).collect();
            to_before.sort_unstable();
            to_after.sort_unstable();
            assert_eq!(to_before, to_after, "seed {seed}: destinations changed");
            let mut thirds: Vec<_> = after.iter().map(|j| j.2).collect();
            thirds.sort_unstable();
            assert_eq!(thirds, vec![0, 1, 2, 3], "seed {seed}: stale segment index");
            assert_full_coverage(&after, 12);
        }
    }

    #[test]
    fn shuffle_changes_playback_order() {
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
    }

    #[test]
    fn shuffle_single_segment_is_noop() {
        let mut builder = JumpBuilder::empty(8);
        let after = builder.shuffle_seeded(99).build();
        assert_eq!(after, vec![Jump(7, 0, 0)]);
    }

    #[test]
    fn shuffle_is_chainable_and_stays_valid() {
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
    fn jump_rank_matches_segment_order() {
        for seed in [1, 7, 123] {
            let builder = JumpBuilder::split_evenly(12, 4).shuffle_seeded(seed);
            let jumps = builder.build();
            let segs = builder.segments();
            let ends: Vec<_> = segs.iter().map(|s| s.end).collect();
            for (pos, j) in jumps.iter().enumerate() {
                assert_eq!(j.2, pos);
                let seg = ends.iter().position(|&e| e == j.0).unwrap();
                assert_eq!(segs[seg].order, pos);
            }
        }
    }

    fn visit_order(size: usize, jumps: &[Jump]) -> Vec<usize> {
        let b = JumpBuilder::from_jumps(size, jumps.to_vec());
        let starts = b.segment_starts();
        b.cycle_order(&starts)
    }

    #[test]
    fn scaled_linear_up_is_exact() {
        let scaled = JumpBuilder::split_evenly(12, 3).scaled(24).build();
        assert_eq!(scaled, JumpBuilder::split_evenly(24, 3).build());
        assert_eq!(scaled, vec![Jump(7, 8, 0), Jump(15, 16, 1), Jump(23, 0, 2)]);

        let scaled = JumpBuilder::split_evenly(80, 8).scaled(160).build();
        assert_eq!(scaled, JumpBuilder::split_evenly(160, 8).build());
        assert_eq!(scaled.len(), 8);
        for (i, j) in scaled.iter().enumerate() {
            assert_eq!(*j, Jump((i + 1) * 20 - 1, (i + 1) * 20 % 160, i));
        }
    }

    #[test]
    fn scaled_linear_down_with_remainder_is_exact() {
        let scaled = JumpBuilder::split_evenly(12, 3).scaled(10).build();
        assert_eq!(scaled, JumpBuilder::split_evenly(10, 3).build());
        assert_eq!(scaled, vec![Jump(2, 3, 0), Jump(5, 6, 1), Jump(9, 0, 2)]);
    }

    #[test]
    fn scaled_same_size_is_identity() {
        for seed in [1, 7, 123] {
            let before = JumpBuilder::split_evenly(12, 4).shuffle_seeded(seed);
            let after = before.scaled(12).build();
            assert_eq!(after, before.build());
        }
    }

    #[test]
    fn scaled_shuffled_preserves_visit_order() {
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
    }

    #[test]
    fn scaled_down_merges_but_stays_covering() {
        let scaled = JumpBuilder::split_evenly(12, 8).scaled(5).build();
        assert_eq!(scaled.len(), 3);
        assert_in_bounds(&scaled, 5);
        assert_full_coverage(&scaled, 5);
    }

    #[test]
    fn scaled_collapse_to_single() {
        assert_eq!(
            JumpBuilder::from_jumps(6, vec![]).build(),
            vec![Jump(5, 0, 0)]
        );
        assert_eq!(JumpBuilder::empty(8).scaled(3).build(), vec![Jump(2, 0, 0)]);
    }

    #[test]
    fn scaled_up_then_down_roundtrip_linear() {
        let roundtrip = JumpBuilder::split_evenly(12, 3)
            .scaled(24)
            .scaled(12)
            .build();
        assert_eq!(roundtrip, JumpBuilder::split_evenly(12, 3).build());
    }

    #[test]
    fn from_jumps_empty_falls_back() {
        assert_eq!(
            JumpBuilder::from_jumps(6, vec![]).build(),
            vec![Jump(5, 0, 0)]
        );
    }
}
