use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{SeedableRng, rng};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Jump {
    pub from: usize,
    pub to: usize,
    pub rank: usize,
}

impl Jump {
    pub const fn new(from: usize, to: usize, rank: usize) -> Self {
        Self { from, to, rank }
    }
}

impl Serialize for Jump {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Jump", 3)?;
        s.serialize_field("from", &self.from)?;
        s.serialize_field("to", &self.to)?;
        s.serialize_field("rank", &self.rank)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for Jump {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct JumpVisitor;

        impl<'de> de::Visitor<'de> for JumpVisitor {
            type Value = Jump;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("Jump as {from,to,rank} or [from,to,rank]")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let from: usize = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let to: usize = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let rank: usize = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                Ok(Jump { from, to, rank })
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut from = None;
                let mut to = None;
                let mut rank = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "from" => from = Some(map.next_value()?),
                        "to" => to = Some(map.next_value()?),
                        "rank" => rank = Some(map.next_value()?),
                        _ => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                Ok(Jump {
                    from: from.ok_or_else(|| de::Error::missing_field("from"))?,
                    to: to.ok_or_else(|| de::Error::missing_field("to"))?,
                    rank: rank.ok_or_else(|| de::Error::missing_field("rank"))?,
                })
            }
        }

        deserializer.deserialize_any(JumpVisitor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JumpSegment {
    pub start: usize,
    pub end: usize,
    /// Playback order of this segment (0 == first visited).
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
            jumps: vec![Jump::new(size - 1, 0, 0)],
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
            jumps.push(Jump::new(end, next_start, i));
        }
        JumpBuilder { size, jumps }
    }

    /// Shuffle playback order of segments. Returns a new builder for chaining.
    pub fn shuffle(&mut self) -> Self {
        self.shuffle_with(&mut rng())
    }

    pub fn shuffle_seeded(&mut self, seed: u64) -> Self {
        self.shuffle_with(&mut StdRng::seed_from_u64(seed))
    }

    fn shuffle_with(&mut self, rng: &mut impl rand::Rng) -> Self {
        let n = self.jumps.len();
        if n > 1 {
            let starts = self.segment_starts_validated();
            let ends = Self::segment_ends(&starts, self.size);
            let mut order: Vec<usize> = (0..n).collect();
            order.shuffle(rng);
            Self::rotate_to_zero(&mut order);
            self.jumps = Self::assemble_jumps(&starts, &ends, &order);
        }
        self.clone()
    }

    /// Reorder segments according to `order` (permutation of segment ids, 0-anchored after normalization).
    pub fn order(&mut self, order: &[usize]) -> Self {
        let n = self.jumps.len();
        if Self::is_valid_order(order, n) {
            let mut norm = order.to_vec();
            Self::rotate_to_zero(&mut norm);
            let starts = self.segment_starts_validated();
            let ends = Self::segment_ends(&starts, self.size);
            self.jumps = Self::assemble_jumps(&starts, &ends, &norm);
        }
        self.clone()
    }

    /// Swap two segment ids inside an explicit order vector.
    pub fn swap_positions(order: &mut [usize], a: usize, b: usize) {
        if let (Some(pa), Some(pb)) = (
            order.iter().position(|&x| x == a),
            order.iter().position(|&x| x == b),
        ) {
            order.swap(pa, pb);
        }
    }

    fn is_valid_order(order: &[usize], n: usize) -> bool {
        if order.len() != n {
            return false;
        }
        let mut seen = vec![false; n];
        for &s in order {
            if s >= n || seen[s] {
                return false;
            }
            seen[s] = true;
        }
        true
    }

    fn rotate_to_zero(order: &mut Vec<usize>) {
        if let Some(k) = order.iter().position(|&s| s == 0) {
            order.rotate_left(k);
        }
    }

    fn assemble_jumps(starts: &[usize], ends: &[usize], order: &[usize]) -> Vec<Jump> {
        let n = order.len();
        (0..n)
            .map(|i| Jump::new(ends[order[i]], starts[order[(i + 1) % n]], i))
            .collect()
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn build(&self) -> Vec<Jump> {
        self.jumps.clone()
    }

    /// This currently clones the jumps. So keep this out of the audio path
    pub fn from_jumps(size: usize, jumps: &[Jump]) -> Self {
        assert!(size > 0);
        if jumps.is_empty() {
            return Self::empty(size);
        }
        JumpBuilder { size, jumps: jumps.to_owned() }
    }

    pub fn scaled(&self, new_size: usize) -> Self {
        assert!(new_size > 0);
        if new_size == self.size {
            return self.clone();
        }
        let starts = self.segment_starts_validated();
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
        let mut seen = vec![false; new_starts.len()];
        let mut new_order = Vec::with_capacity(order.len());
        for &seg_id in &order {
            let scaled_start = starts[seg_id] * new_size / self.size;
            if let Some(pos) = new_starts.iter().position(|&s| s == scaled_start) {
                if !seen[pos] {
                    seen[pos] = true;
                    new_order.push(pos);
                }
            }
        }
        if new_order.len() <= 1 {
            return Self::empty(new_size);
        }
        Self::rotate_to_zero(&mut new_order);
        let ends = Self::segment_ends(&new_starts, new_size);
        let jumps = Self::assemble_jumps(&new_starts, &ends, &new_order);
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
        let mut order = vec![0];
        let mut visited = vec![false; n];
        visited[0] = true;
        for _ in 0..n {
            let cur = *order.last().unwrap();
            let dest = match self.jumps.iter().find(|j| j.from == ends[cur]) {
                Some(j) => j.to,
                None => break,
            };
            let nxt = match starts.iter().position(|&s| s == dest) {
                Some(i) => i,
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

    fn segment_starts_validated(&self) -> Vec<usize> {
        self.try_segment_starts()
            .unwrap_or_else(|| self.fallback_even_starts())
    }

    fn try_segment_starts(&self) -> Option<Vec<usize>> {
        let n = self.jumps.len();
        let mut starts: Vec<usize> = self.jumps.iter().map(|j| j.to).collect();
        starts.sort_unstable();
        starts.dedup();
        let valid = starts.len() == n && starts[0] == 0 && starts.iter().all(|&s| s < self.size);
        if valid { Some(starts) } else { None }
    }

    fn fallback_even_starts(&self) -> Vec<usize> {
        let n = self.jumps.len();
        let width = (self.size / n.max(1)).max(1);
        (0..n).map(|i| (i * width).min(self.size - 1)).collect()
    }

    fn segment_starts(&self) -> Vec<usize> {
        self.segment_starts_validated()
    }

    pub fn play_order(&self) -> Vec<usize> {
        self.cycle_order(&self.segment_starts())
    }

    pub fn swap_segments(mut self, a: usize, b: usize) -> Self {
        let mut order = self.play_order();
        Self::swap_positions(&mut order, a, b);
        self.order(&order)
    }

    pub fn segments(&self) -> Vec<JumpSegment> {
        let starts = self.segment_starts();
        let order = self.cycle_order(&starts);
        let ends = Self::segment_ends(&starts, self.size);
        let mut rank_of = vec![0usize; starts.len()];
        for (rank, &seg) in order.iter().enumerate() {
            rank_of[seg] = rank;
        }
        (0..starts.len())
            .map(|i| JumpSegment {
                start: starts[i],
                end: ends[i],
                order: rank_of[i],
            })
            .collect()
    }

    pub fn is_covering(&self) -> bool {
        if self.size == 0 || self.jumps.is_empty() {
            return false;
        }
        if self
            .jumps
            .iter()
            .any(|j| j.from >= self.size || j.to >= self.size)
        {
            return false;
        }
        {
            let mut seen = self.jumps.clone();
            seen.sort_by_key(|j| j.from);
            for w in seen.windows(2) {
                if w[0].from == w[1].from {
                    return false;
                }
            }
        }
        let mut visited = vec![false; self.size];
        let mut pos = 0;
        for _ in 0..self.size {
            if pos >= self.size || visited[pos] {
                return false;
            }
            visited[pos] = true;
            // linear search is intentional (n small, audio-side clarity over map).
            pos = match self.jumps.iter().find(|j| j.from == pos) {
                Some(j) => j.to,
                None => pos + 1,
            };
        }
        pos == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delay_engine::engine::DelayEngine;

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

    #[test]
    fn empty_produces_single_wrap_jump() {
        assert_eq!(
            JumpBuilder::empty(8).build(),
            vec![Jump::new(7, 0, 0)]
        );
        assert_eq!(
            JumpBuilder::empty(1).build(),
            vec![Jump::new(0, 0, 0)]
        );
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
        assert_eq!(
            jumps,
            vec![
                Jump::new(3, 4, 0),
                Jump::new(7, 8, 1),
                Jump::new(11, 0, 2)
            ]
        );
        assert_eq!(engine_read_order(&jumps, 12), (0..12).collect::<Vec<_>>());
    }

    #[test]
    fn split_evenly_with_remainder_absorbs_tail() {
        let jumps = JumpBuilder::split_evenly(10, 3).build();
        assert_eq!(
            jumps,
            vec![
                Jump::new(2, 3, 0),
                Jump::new(5, 6, 1),
                Jump::new(9, 0, 2)
            ]
        );
        assert_in_bounds(&jumps, 10);
        assert_full_coverage(&jumps, 10);
    }

    #[test]
    fn split_evenly_more_splits_than_samples_clamps() {
        let jumps = JumpBuilder::split_evenly(3, 5).build();
        assert_eq!(
            jumps,
            vec![
                Jump::new(0, 1, 0),
                Jump::new(1, 2, 1),
                Jump::new(2, 0, 2)
            ]
        );
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
        assert_eq!(
            first,
            vec![Jump::new(3, 4, 0), Jump::new(7, 0, 1)]
        );
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
            let mut thirds: Vec<_> = after.iter().map(|j| j.rank).collect();
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
        assert_eq!(after, vec![Jump::new(7, 0, 0)]);
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
                assert_eq!(j.rank, pos);
                let seg = ends.iter().position(|&e| e == j.from).unwrap();
                assert_eq!(segs[seg].order, pos);
            }
        }
    }

    #[test]
    fn order_rewires_exact_cycle() {
        let jumps = JumpBuilder::split_evenly(12, 3).order(&[2, 1, 0]).build();
        assert_eq!(
            jumps,
            vec![
                Jump::new(3, 8, 0),
                Jump::new(11, 4, 1),
                Jump::new(7, 0, 2)
            ]
        );
        let (order, end) = walk_order(&jumps, 12);
        assert_eq!(order, vec![0, 1, 2, 3, 8, 9, 10, 11, 4, 5, 6, 7]);
        assert_eq!(end, 0);
    }

    #[test]
    fn order_rotation_is_equivalent() {
        let a = JumpBuilder::split_evenly(12, 3).order(&[1, 2, 0]).build();
        let b = JumpBuilder::split_evenly(12, 3).order(&[0, 1, 2]).build();
        assert_eq!(a, b);
        let (wa, _) = walk_order(&a, 12);
        let (wb, _) = walk_order(&b, 12);
        assert_eq!(wa, wb);
    }

    #[test]
    fn order_invalid_input_keeps_table() {
        let mut builder = JumpBuilder::split_evenly(12, 3);
        let before = builder.build();
        builder.order(&[0, 1]);
        builder.order(&[0, 1, 1]);
        builder.order(&[0, 1, 3]);
        builder.order(&[]);
        assert_eq!(builder.build(), before);
    }

    #[test]
    fn order_single_segment() {
        let mut builder = JumpBuilder::empty(8);
        assert_eq!(
            builder.order(&[0]).build(),
            vec![Jump::new(7, 0, 0)]
        );
        assert_eq!(
            builder.order(&[1]).build(),
            vec![Jump::new(7, 0, 0)]
        );
        assert_eq!(
            builder.order(&[]).build(),
            vec![Jump::new(7, 0, 0)]
        );
    }

    #[test]
    fn swap_positions_swaps_ids() {
        let mut order = vec![0, 2, 3, 1];
        JumpBuilder::swap_positions(&mut order, 2, 1);
        assert_eq!(order, vec![0, 1, 3, 2]);
        JumpBuilder::swap_positions(&mut order, 1, 1);
        assert_eq!(order, vec![0, 1, 3, 2]);
        JumpBuilder::swap_positions(&mut order, 1, 9);
        assert_eq!(order, vec![0, 1, 3, 2]);
    }

    #[test]
    fn is_covering_accepts_valid_tables() {
        assert!(JumpBuilder::empty(8).is_covering());
        assert!(JumpBuilder::split_evenly(12, 3).is_covering());
        assert!(
            JumpBuilder::split_evenly(12, 4)
                .shuffle_seeded(7)
                .is_covering()
        );
    }

    #[test]
    fn is_covering_rejects_broken_tables() {
        assert!(!JumpBuilder::from_jumps(10, &[Jump::new(4, 0, 0)]).is_covering());
        assert!(!JumpBuilder::from_jumps(10, &[Jump::new(2, 2, 0)]).is_covering());
        assert!(!JumpBuilder::from_jumps(5, &[Jump::new(9, 0, 0)]).is_covering());
        assert!(JumpBuilder::from_jumps(6, &[]).is_covering());
    }

    #[test]
    fn order_roundtrip_with_segments() {
        let mut builder = JumpBuilder::split_evenly(12, 4).shuffle_seeded(7);
        builder.order(&[3, 1, 0, 2]);
        let jumps = builder.build();
        let segs = builder.segments();
        assert_eq!(visit_order(12, &jumps), vec![0, 2, 3, 1]);
        for (s, seg) in segs.iter().enumerate() {
            let want = [0, 2, 3, 1].iter().position(|&x| x == s).unwrap();
            assert_eq!(seg.order, want);
        }
        let ends: Vec<_> = segs.iter().map(|s| s.end).collect();
        for (pos, j) in jumps.iter().enumerate() {
            assert_eq!(j.rank, pos);
            let seg = ends.iter().position(|&e| e == j.from).unwrap();
            assert_eq!(segs[seg].order, pos);
        }
    }

    #[test]
    fn order_overwrites_shuffle_deterministically() {
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

    fn visit_order(size: usize, jumps: &[Jump]) -> Vec<usize> {
        let b = JumpBuilder::from_jumps(size, jumps);
        let starts = b.segment_starts();
        b.cycle_order(&starts)
    }

    #[test]
    fn scaled_linear_up_is_exact() {
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
    }

    #[test]
    fn scaled_linear_down_with_remainder_is_exact() {
        let scaled = JumpBuilder::split_evenly(12, 3).scaled(10).build();
        assert_eq!(scaled, JumpBuilder::split_evenly(10, 3).build());
        assert_eq!(
            scaled,
            vec![
                Jump::new(2, 3, 0),
                Jump::new(5, 6, 1),
                Jump::new(9, 0, 2)
            ]
        );
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
            JumpBuilder::from_jumps(6, &[]).build(),
            vec![Jump::new(5, 0, 0)]
        );
        assert_eq!(
            JumpBuilder::empty(8).scaled(3).build(),
            vec![Jump::new(2, 0, 0)]
        );
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
            JumpBuilder::from_jumps(6, &[]).build(),
            vec![Jump::new(5, 0, 0)]
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
}
