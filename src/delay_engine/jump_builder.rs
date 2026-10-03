use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{SeedableRng, rng};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use wgpu::naga::Statement::Continue;

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
        JumpBuilder {
            size,
            jumps: jumps.to_owned(),
        }
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

const MIN_EDITOR_SIZE: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Portal {
    /// Describes a jump that departs from just the segments
    pub exit: usize,
    pub entry: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SegmentEditor {
    /// Setup the segment structure
    pub size: usize,
    pub starts: Vec<usize>,
    pub order: Vec<usize>,
    pub portals: Vec<Option<Portal>>,
}

#[derive(Deserialize)]
struct SegmentEditorRaw {
    #[serde(default)]
    size: usize,
    #[serde(default)]
    starts: Vec<usize>,
    #[serde(default)]
    order: Vec<usize>,
    #[serde(default)]
    portals: Vec<Option<Portal>>,
}

impl<'de> Deserialize<'de> for SegmentEditor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = SegmentEditorRaw::deserialize(deserializer)?;
        Ok(Self::checked(raw.size, raw.starts, raw.order, raw.portals)
            .unwrap_or_else(|| Self::split_evenly(raw.size.max(MIN_EDITOR_SIZE), 8)))
    }
}

impl SegmentEditor {
    /// Just a single full segment
    pub fn single(size: usize) -> Self {
        assert!(size > 0);
        Self {
            size,
            starts: vec![0],
            order: vec![0],
            portals: vec![None],
        }
    }

    /// Split evently in several segments
    pub fn split_evenly(size: usize, splits: u32) -> Self {
        assert!(size > 0);
        assert!(splits > 0);
        let n = (splits as usize).clamp(1, size);
        let width = size / n;
        let starts: Vec<usize> = (0..n).map(|i| (i * width).min(size - 1)).collect();
        Self {
            size,
            starts,
            order: (0..n).collect(),
            portals: vec![None; n],
        }
    }

    /// Create from a list of jumps
    pub fn from_jumps(size: usize, jumps: &[Jump]) -> Self {
        assert!(size > 0);
        let mut starts: Vec<usize> = jumps.iter().map(|j| j.to).collect();
        starts.sort_unstable();
        starts.dedup();
        let usable = !starts.is_empty()
            && starts[0] == 0
            && starts.len() == jumps.len()
            && starts.iter().all(|&s| s < size);
        if !usable {
            return Self::split_evenly(size, 8);
        }
        let n = starts.len();
        let ends = Self::ends_of(&starts, size);
        let mut order = vec![0usize];
        let mut visited = vec![false; n];
        visited[0] = true;
        for _ in 0..n {
            let cur = *order.last().unwrap();
            let dest = Self::table_next(jumps, ends[cur]);
            match starts.iter().position(|&s| s == dest) {
                Some(i) if !visited[i] => {
                    visited[i] = true;
                    order.push(i);
                }
                _ => break,
            }
        }
        for i in 0..n {
            if !visited[i] {
                order.push(i);
            }
        }
        Self::rotate_to_zero(&mut order);
        Self {
            size,
            starts,
            order,
            portals: vec![None; n],
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn starts(&self) -> &[usize] {
        &self.starts
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn portals(&self) -> &[Option<Portal>] {
        &self.portals
    }

    pub fn is_unglued(&self, boundary: usize) -> bool {
        self.portals.get(boundary).is_some_and(|p| p.is_some())
    }

    pub fn can_unglue(&self, boundary: usize) -> bool {
        let n = self.starts.len();
        boundary > 0 && boundary < n && !self.is_unglued(boundary) && self.is_adjacent(boundary)
    }

    fn is_adjacent(&self, boundary: usize) -> bool {
        let n = self.starts.len();
        n > 0 && boundary > 0 && self.rank_of(boundary) == (self.rank_of(boundary - 1) + 1) % n
    }

    /// The specific segments, useful for the UI rendering
    pub fn segments(&self) -> Vec<JumpSegment> {
        let ends = self.ends_all();
        let mut rank = vec![0usize; self.starts.len()];
        for (slot, &seg) in self.order.iter().enumerate() {
            rank[seg] = slot;
        }
        (0..self.starts.len())
            .map(|i| JumpSegment {
                start: self.starts[i],
                end: ends[i],
                order: rank[i],
            })
            .collect()
    }

    /// Build the resulting list of jumps
    pub fn build_jumps(&self) -> Vec<Jump> {
        let n = self.starts.len();
        if n == 0 || self.size == 0 {
            return Vec::new();
        }

        let mut rank = vec![0_usize; n];
        for (slot, &seg) in self.order.iter().enumerate() {
            rank[seg] = slot;
        }

        let ends: Vec<usize> = (0..n)
            .map(|i| {
                if i + 1 < n {
                    self.starts[i + 1].saturating_sub(1)
                } else {
                    self.size - 1
                }
            })
            .collect();

        let mut jumps: Vec<Jump> = (0..n)
            .map(|j| Jump::new(ends[self.order[j]], self.starts[self.order[(j + 1) % n]], j))
            .collect();

        for (i, portal) in self.portals.iter().enumerate() {
            let Some(p) = portal else { continue };
            let left = if i == 0 { n - 1 } else { i - 1 };
            jumps[rank[left]].to = p.entry;
            jumps[(rank[i] + n - 1) % n].from = p.exit;
        }
        jumps
    }

    /// Validate that a list of jumps is valid
    pub fn validate_table(size: usize, jumps: &[Jump]) -> bool {
        Self::cycle_edges(size, jumps)
            .and_then(|e| Self::internal_cycle_len(size, &e))
            .is_some()
    }

    /// Validate the hold table
    pub fn validate_cycle(&self) -> bool {
        Self::validate_table(self.size, &self.build_jumps())
    }

    /// Iterate through cycle and return the list of jump positions
    pub fn visit_cycle(&self) -> Vec<usize> {
        let jumps = self.build_jumps();
        let Some(edges) = Self::cycle_edges(self.size, &jumps) else {
            return Vec::new();
        };
        let Some(len) = Self::internal_cycle_len(self.size, &edges) else {
            return Vec::new();
        };
        let mut cycle = Vec::with_capacity(len);
        cycle.push(0);
        let mut pos = 0usize;
        for _ in 1..len {
            pos = Self::step(&edges, pos);
            cycle.push(pos);
        }
        cycle
    }

    fn internal_cycle_len(size: usize, edges: &[(usize, usize)]) -> Option<usize> {
        let mut pos = 0usize;
        for len in 1..=size {
            let next = Self::step(edges, pos);
            if next >= size {
                return None;
            }
            if next == 0 {
                return Some(len);
            }
            pos = next;
        }
        None
    }

    /// Calculate cycle length
    pub fn cycle_len(&self) -> Option<usize> {
        let jumps = self.build_jumps();
        let edges = Self::cycle_edges(self.size, &jumps)?;
        Self::internal_cycle_len(self.size, &edges)
    }

    fn step(edges: &[(usize, usize)], pos: usize) -> usize {
        match edges.binary_search_by_key(&pos, |&(from, _)| from) {
            Ok(idx) => edges[idx].1,
            Err(_) => pos + 1,
        }
    }

    fn cycle_edges(size: usize, jumps: &[Jump]) -> Option<Vec<(usize, usize)>> {
        if size == 0 || jumps.is_empty() {
            return None;
        }
        let mut edges: Vec<(usize, usize)> = jumps.iter().map(|j| (j.from, j.to)).collect();
        edges.sort_unstable();
        if edges.windows(2).any(|w| w[0].0 == w[1].0) {
            return None;
        }
        if edges.iter().any(|&(from, to)| from >= size || to >= size) {
            return None;
        }
        Some(edges)
    }

    pub fn can_move_boundary(&self, boundary: usize) -> bool {
        boundary > 0 && boundary < self.starts.len()
    }

    pub fn move_boundary(&mut self, boundary: usize, pos: usize) -> bool {
        if !self.can_move_boundary(boundary) {
            return false;
        }
        let n = self.starts.len();
        let lo = self.starts[boundary - 1] + 1;
        let hi = if boundary + 1 < n {
            self.starts[boundary + 1] - 1
        } else {
            self.size - 1
        };
        self.starts[boundary] = pos.clamp(lo, hi);
        true
    }

    pub fn unglue(&mut self, boundary: usize) -> bool {
        if !self.can_unglue(boundary) {
            return false;
        }
        self.portals[boundary] = Some(Portal {
            exit: self.starts[boundary] - 1,
            entry: self.starts[boundary],
        });
        true
    }

    pub fn can_move_exit(&self, boundary: usize) -> bool {
        boundary > 0 && boundary < self.starts.len() && self.is_unglued(boundary)
    }

    pub fn move_exit(&mut self, boundary: usize, pos: usize) -> bool {
        if !self.can_move_exit(boundary) {
            return false;
        }
        let Some(p) = self.portals[boundary] else {
            return false;
        };
        let hi = self.ends_all()[boundary - 1];
        self.portals[boundary] = Some(Portal {
            exit: pos.clamp(self.starts[boundary - 1], hi),
            ..p
        });
        true
    }

    pub fn can_move_entry(&self, boundary: usize) -> bool {
        self.can_move_exit(boundary)
    }

    pub fn move_entry(&mut self, boundary: usize, pos: usize) -> bool {
        if !self.can_move_entry(boundary) {
            return false;
        }
        let Some(p) = self.portals[boundary] else {
            return false;
        };
        self.portals[boundary] = Some(Portal {
            entry: pos.min(self.size.saturating_sub(1)),
            ..p
        });
        true
    }

    pub fn can_reweld(&self, boundary: usize) -> bool {
        boundary > 0 && boundary < self.portals.len() && self.is_unglued(boundary)
    }

    pub fn reweld(&mut self, boundary: usize) -> bool {
        if !self.can_reweld(boundary) {
            return false;
        }
        self.portals[boundary] = None;
        true
    }

    pub fn swap_segments(&mut self, a: usize, b: usize) {
        JumpBuilder::swap_positions(&mut self.order, a, b);
        Self::rotate_to_zero(&mut self.order);
    }

    // Rescale the whole segments to a different buffer length
    pub fn scaled(&self, new_size: usize) -> Self {
        assert!(new_size > 0);
        let n = self.starts.len();
        let scaled: Vec<usize> = self
            .starts
            .iter()
            .map(|s| s * new_size / self.size)
            .collect();
        let mut starts: Vec<usize> = Vec::with_capacity(n);
        for s in scaled.iter().copied() {
            if starts.last() != Some(&s) {
                starts.push(s);
            }
        }
        if starts.len() <= 1 {
            return Self::single(new_size);
        }
        let mut seen = vec![false; starts.len()];
        let mut order: Vec<usize> = Vec::with_capacity(n);
        for &id in &self.order {
            if let Some(pos) = starts.iter().position(|&s| s == scaled[id]) {
                if !seen[pos] {
                    seen[pos] = true;
                    order.push(pos);
                }
            }
        }
        if order.len() <= 1 {
            return Self::single(new_size);
        }
        Self::rotate_to_zero(&mut order);
        let m = starts.len();
        let mut out = Self {
            size: new_size,
            starts,
            order,
            portals: vec![None; m],
        };
        for i in 1..n {
            let Some(p) = self.portals[i] else { continue };
            let left_ok = i == 1 || scaled[i - 1] != scaled[i - 2];
            let right_ok = i + 1 == n || scaled[i] != scaled[i + 1];
            if !left_ok || !right_ok {
                continue;
            }
            let target = scaled[i];
            if let Some(j) = out.starts.iter().position(|&s| s == target) {
                if j > 0 && out.portals[j].is_none() {
                    out.portals[j] = Some(Portal {
                        exit: p.exit * new_size / self.size,
                        entry: p.entry * new_size / self.size,
                    });
                }
            }
        }
        out.reclamp_portals();
        out
    }

    fn width_of(&self, segment: usize) -> usize {
        let n = self.starts.len();
        let hi = if segment + 1 < n {
            self.starts[segment + 1]
        } else {
            self.size
        };
        hi.saturating_sub(self.starts.get(segment).copied().unwrap_or(self.size))
    }

    pub fn can_split(&self, segment: usize) -> bool {
        segment < self.starts.len() && self.width_of(segment) >= 2
    }

    pub fn split_segment(&mut self, segment: usize) -> bool {
        if segment >= self.starts.len() {
            return false;
        }
        let width = self.width_of(segment);
        if width < 2 {
            return false;
        }
        let n = self.starts.len();
        let mid = self.starts[segment] + width / 2;
        let size = self.size;
        let mut starts = self.starts.clone();
        starts.insert(segment + 1, mid);
        let mut order: Vec<usize> = self
            .order
            .iter()
            .map(|&id| if id > segment { id + 1 } else { id })
            .collect();
        let at = order.iter().position(|&id| id == segment).unwrap_or(0);
        order.insert(at + 1, segment + 1);
        let mut portals = vec![None; n + 1];
        for i in 1..n {
            if let Some(p) = self.portals[i] {
                let j = if i > segment { i + 1 } else { i };
                portals[j] = Some(p);
            }
        }
        *self = Self {
            size,
            starts,
            order,
            portals,
        };
        self.reclamp_portals();
        true
    }

    pub fn can_merge(&self, boundary: usize) -> bool {
        self.starts.len() >= 2
            && boundary > 0
            && boundary < self.starts.len()
            && !self.is_unglued(boundary)
    }

    pub fn merge_segments(&mut self, boundary: usize) -> bool {
        if !self.can_merge(boundary) {
            return false;
        }
        let n = self.starts.len();
        let keep = boundary - 1;
        let size = self.size;
        let mut starts = self.starts.clone();
        starts.remove(boundary);
        let mut order: Vec<usize> = Vec::with_capacity(n - 1);
        let mut seen = vec![false; n - 1];
        for &id in &self.order {
            let mapped = match id.cmp(&keep) {
                std::cmp::Ordering::Less => id,
                std::cmp::Ordering::Equal => keep,
                std::cmp::Ordering::Greater if id == boundary => keep,
                std::cmp::Ordering::Greater => id - 1,
            };
            if !seen[mapped] {
                seen[mapped] = true;
                order.push(mapped);
            }
        }
        let mut portals = vec![None; n - 1];
        for i in 1..n {
            if i == boundary {
                continue;
            }
            let j = if i < boundary { i } else { i - 1 };
            portals[j] = self.portals[i];
        }
        Self::rotate_to_zero(&mut order);
        *self = Self {
            size,
            starts,
            order,
            portals,
        };
        self.reclamp_portals();
        true
    }

    /// Set self to a preset with n splits
    pub fn preset_split(&mut self, splits: u32) {
        *self = Self::split_evenly(self.size, splits);
    }

    pub fn checked(
        size: usize,
        starts: Vec<usize>,
        order: Vec<usize>,
        portals: Vec<Option<Portal>>,
    ) -> Option<Self> {
        let n = starts.len();
        if size == 0 || n == 0 || starts[0] != 0 {
            return None;
        }
        if starts.iter().any(|&s| s >= size) || starts.windows(2).any(|w| w[0] >= w[1]) {
            return None;
        }
        if order.len() != n {
            return None;
        }
        let mut seen = vec![false; n];
        for &slot in &order {
            if slot >= n || seen[slot] {
                return None;
            }
            seen[slot] = true;
        }
        if portals.len() != n || portals[0].is_some() {
            return None;
        }
        let mut order = order;
        Self::rotate_to_zero(&mut order);
        Some(Self {
            size,
            starts,
            order,
            portals,
        })
    }

    /// Get corresponding ends to a list of starts
    fn ends_of(starts: &[usize], size: usize) -> Vec<usize> {
        let n = starts.len();
        (0..n)
            .map(|i| {
                if i + 1 < n {
                    starts[i + 1].saturating_sub(1)
                } else {
                    size - 1
                }
            })
            .collect()
    }

    fn ends_all(&self) -> Vec<usize> {
        Self::ends_of(&self.starts, self.size)
    }

    fn rank_of(&self, segment: usize) -> usize {
        self.order.iter().position(|&s| s == segment).unwrap_or(0)
    }

    /// Rotate until the start is at 0
    fn rotate_to_zero(order: &mut Vec<usize>) {
        if let Some(k) = order.iter().position(|&s| s == 0) {
            order.rotate_left(k);
        }
    }

    /// Find the next segment
    fn table_next(jumps: &[Jump], pos: usize) -> usize {
        jumps
            .iter()
            .find(|j| j.from == pos)
            .map(|j| j.to)
            .unwrap_or(pos + 1)
    }

    fn reclamp_portals(&mut self) {
        let n = self.starts.len();
        for i in 1..n {
            let Some(p) = self.portals[i] else { continue };
            let lo = self.starts[i - 1];
            let hi = self.ends_all()[i - 1];
            self.portals[i] = Some(Portal {
                exit: p.exit.clamp(lo, hi),
                entry: p.entry.min(self.size.saturating_sub(1)),
            });
        }
    }
}

#[cfg(test)]
#[path = "jump_builder_tests.rs"]
mod jump_builder_tests;
