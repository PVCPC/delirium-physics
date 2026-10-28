#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Bounds { pub min: [f32; 3], pub max: [f32; 3] }
impl Bounds {
    pub fn identity_transformed(self) -> Self {
        let center: [f32; 3] = std::array::from_fn(|i| (self.max[i]+self.min[i])*0.5);
        let half: [f32; 3] = std::array::from_fn(|i| (self.max[i]-self.min[i])*0.5);
        Self { min: std::array::from_fn(|i| center[i]-half[i]),
            max: std::array::from_fn(|i| center[i]+half[i]) }
    }
    pub fn overlaps(self, b: Self) -> bool {
        (0..3).all(|i| self.min[i] <= b.max[i] && b.min[i] <= self.max[i])
    }
    pub fn union(self, b: Self) -> Self {
        Self { min: std::array::from_fn(|i| self.min[i].min(b.min[i])),
            max: std::array::from_fn(|i| self.max[i].max(b.max[i])) }
    }
    pub fn padded(self) -> Self {
        Self { min: self.min.map(|v| v-1.), max: self.max.map(|v| v+1.) }
    }
    fn cube(self) -> Self {
        let half: [f32; 3] = std::array::from_fn(|i| (self.max[i]-self.min[i])*0.5);
        let radius = half[0].max(half[1]).max(half[2]);
        let center: [f32; 3] = std::array::from_fn(|i| (self.max[i]+self.min[i])*0.5);
        Self { min: center.map(|v| v-radius), max: center.map(|v| v+radius) }
    }
    fn inner(self) -> Self {
        let padding: [f32; 3] = std::array::from_fn(|i| (self.max[i]-self.min[i])*f32::from_bits(0xbecc_cccd));
        Self { min: std::array::from_fn(|i| self.min[i]-padding[i]),
            max: std::array::from_fn(|i| self.max[i]+padding[i]) }
    }
    fn child(self, index: usize) -> Self {
        let inner = self.inner();
        Self { min: std::array::from_fn(|i| if index & (1<<i) != 0 { inner.min[i] } else { self.min[i] }),
            max: std::array::from_fn(|i| if index & (1<<i) != 0 { self.max[i] } else { inner.max[i] }) }
    }
    fn containing_child(self, entry: Self) -> Option<(usize, Self)> {
        let inner = self.inner();
        let mut index = 0;
        for i in 0..3 {
            let low_margin = entry.min[i]-inner.min[i];
            let high_margin = inner.max[i]-entry.max[i];
            if low_margin <= 0. && high_margin <= 0. { return None; }
            if low_margin > high_margin { index |= 1<<i; }
        }
        Some((index, self.child(index)))
    }
    fn movable(self, entry: Self) -> bool {
        let inner = self.inner();
        (0..3).all(|i| inner.max[i] > entry.max[i] || entry.min[i] > inner.min[i])
    }
}

#[derive(Default)]
struct Bucket { entries: Vec<(usize, bool)>, child: Option<usize> }
struct Node { bounds: Bounds, resident: Vec<usize>, buckets: [Bucket; 8] }
impl Node {
    fn new(bounds: Bounds) -> Self { Self { bounds, resident: Vec::new(), buckets: std::array::from_fn(|_| Bucket::default()) } }
}
pub(super) struct Octree { nodes: Vec<Node>, bounds: Vec<Bounds>, node_capacity: usize }
impl Octree {
    pub fn new(asset_bounds: Bounds, bounds: Vec<Bounds>) -> Result<Self, String> {
        if bounds.len() > 0xffff { return Err("Native grind octree exceeds uint16 entry capacity".into()); }
        let mut tree = Self { node_capacity: bounds.len()/2+1, nodes: vec![Node::new(asset_bounds.cube())], bounds };
        for index in 0..tree.bounds.len() { tree.insert(index); }
        Ok(tree)
    }
    fn insert(&mut self, entry: usize) {
        let mut at = 0;
        loop {
            let node_bounds = self.nodes[at].bounds;
            let target = if (0..3).all(|i| self.bounds[entry].min[i] >= node_bounds.min[i]
                && self.bounds[entry].max[i] <= node_bounds.max[i]) {
                node_bounds.containing_child(self.bounds[entry])
            } else { None };
            let Some((slot, child_bounds)) = target else {
                self.nodes[at].resident.push(entry);
                return;
            };
            if let Some(child) = self.nodes[at].buckets[slot].child { at = child; continue; }
            let movable = child_bounds.movable(self.bounds[entry]);
            let bucket = &mut self.nodes[at].buckets[slot];
            bucket.entries.push((entry, movable));
            if bucket.entries.iter().filter(|(_, movable)| *movable).count() > 3
                && self.nodes.len() < self.node_capacity {
                self.split(at, slot, child_bounds);
            }
            return;
        }
    }
    fn split(&mut self, parent: usize, slot: usize, bounds: Bounds) {
        let entries = std::mem::take(&mut self.nodes[parent].buckets[slot].entries);
        let child = self.nodes.len();
        self.nodes[parent].buckets[slot].child = Some(child);
        let mut node = Node::new(bounds);
        for (entry, movable) in entries.into_iter().rev() {
            if movable {
                let (slot, next_bounds) = bounds.containing_child(self.bounds[entry])
                    .expect("native movable predicate guarantees a containing child");
                node.buckets[slot].entries.push((entry, next_bounds.movable(self.bounds[entry])));
            } else { node.resident.push(entry); }
        }
        self.nodes.push(node);
    }
    pub fn query(&self, query: Bounds, limit: usize) -> Vec<usize> {
        let mut result = Vec::new();
        if limit == 0 { return result; }
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            let mut leaf_slots = Vec::new();
            for slot in 0..8 {
                if node.bounds.child(slot).overlaps(query) {
                    if let Some(child) = node.buckets[slot].child { stack.push(child); }
                    else { leaf_slots.push(slot); }
                }
            }
            for slot in leaf_slots.into_iter().rev() {
                for &(entry, _) in node.buckets[slot].entries.iter().rev() {
                    if self.bounds[entry].overlaps(query) {
                        result.push(entry);
                        if result.len() == limit { return result; }
                    }
                }
            }
            for &entry in node.resident.iter().rev() {
                if self.bounds[entry].overlaps(query) {
                    result.push(entry);
                    if result.len() == limit { return result; }
                }
            }
        }
        result
    }
}
