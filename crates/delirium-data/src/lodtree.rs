use crate::map::LodGroup;

#[derive(Clone, Debug, PartialEq)]
pub struct QuadNode {
    pub rect: [f32; 4],
    pub items: Vec<usize>,
    pub children: [Option<usize>; 4],
    pub depth: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuadTree {
    pub nodes: Vec<QuadNode>,
}

impl QuadTree {
    pub fn new(rect: [f32; 4], depth: i32) -> QuadTree {
        QuadTree { nodes: vec![QuadNode { rect, items: vec![], children: [None; 4], depth }] }
    }

    pub fn quarter(r: [f32; 4], q: usize) -> [f32; 4] {
        let (cx, cy) = ((r[0] + r[2]) * 0.5, (r[3] + r[1]) * 0.5);
        match q {
            0 => [r[0], r[1], cx, cy],
            1 => [cx, r[1], r[2], cy],
            2 => [r[0], cy, cx, r[3]],
            _ => [cx, cy, r[2], r[3]],
        }
    }

    fn meets(&self, node: usize, r: [f32; 4], q: usize) -> bool {
        let c = Self::quarter(self.nodes[node].rect, q);
        !(c[0] > r[2]) && !(c[2] < r[0]) && !(c[3] > r[1]) && !(c[1] < r[3])
    }

    pub fn insert(&mut self, node: usize, item: usize, r: [f32; 4]) {
        if self.nodes[node].depth == 0 {
            self.nodes[node].items.push(item);
            return;
        }
        for q in 0..4 {
            if !self.meets(node, r, q) {
                continue;
            }
            let child = match self.nodes[node].children[q] {
                Some(c) => c,
                None => {
                    let n = &self.nodes[node];
                    let c = QuadNode { rect: Self::quarter(n.rect, q), items: vec![], children: [None; 4], depth: n.depth - 1 };
                    self.nodes.push(c);
                    self.nodes[node].children[q] = Some(self.nodes.len() - 1);
                    self.nodes.len() - 1
                }
            };
            self.insert(child, item, r);
        }
    }

    fn quarter_at(&self, node: usize, p: [f32; 2]) -> Option<usize> {
        let n = &self.nodes[node];
        if n.depth == 0 {
            return None;
        }
        let (cx, cy) = ((n.rect[2] + n.rect[0]) * 0.5, (n.rect[3] + n.rect[1]) * 0.5);
        let top = !(p[1] < cy);
        Some(match (!(p[0] < cx), top) {
            (false, true) => 0,
            (true, true) => 1,
            (false, false) => 2,
            (true, false) => 3,
        })
    }

    pub fn find(&self, p: [f32; 2]) -> Vec<usize> {
        let mut path = vec![0];
        while let Some(c) = self.quarter_at(*path.last().unwrap(), p).and_then(|q| self.nodes[*path.last().unwrap()].children[q]) {
            path.push(c);
        }
        path.iter().rev().flat_map(|&n| self.nodes[n].items.iter().copied()).collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct LodGroups {
    pub groups: Vec<LodGroup>,
    pub next: Vec<Option<usize>>,
    pub tree: Option<QuadTree>,
    pub current: Option<usize>,
}

impl LodGroups {
    pub fn new(groups: impl IntoIterator<Item = LodGroup>) -> LodGroups {
        let mut out = LodGroups::default();
        for g in groups {
            out.add(g);
        }
        out
    }

    pub fn add(&mut self, g: LodGroup) {
        let tree = self.tree.get_or_insert_with(|| QuadTree::new([-3000.0, 3000.0, 3000.0, -3000.0], 5));
        let i = self.groups.len();
        let middle = [(g.min[0] + g.max[0]) * 0.5, (g.min[1] + g.max[1]) * 0.5];
        let same = tree.find(middle).into_iter().find(|&e| e != i && self.groups[e].min == g.min && self.groups[e].max == g.max);
        let rect = [g.min[0], g.max[1], g.max[0], g.min[1]];
        self.groups.push(g);
        self.next.push(None);
        match same {
            Some(e) => {
                self.next[i] = self.next[e];
                self.next[e] = Some(i);
            }
            None => tree.insert(0, i, rect),
        }
    }

    fn holds(g: &LodGroup, p: [f32; 3]) -> bool {
        (0..3).all(|k| !(p[k] > g.max[k]) && !(g.min[k] > p[k]))
    }

    pub fn find(&self, p: [f32; 3]) -> Option<usize> {
        self.tree.as_ref()?.find([p[0], p[1]]).into_iter().find(|&g| Self::holds(&self.groups[g], p))
    }

    pub fn update(&mut self, p: [f32; 3]) {
        if let Some(c) = self.current {
            if Self::holds(&self.groups[c], p) {
                return;
            }
            self.current = None;
        }
        self.current = self.find(p);
    }

    pub fn hidden(&self) -> std::collections::HashSet<u32> {
        let mut out = std::collections::HashSet::new();
        let mut g = self.current;
        while let Some(i) = g {
            out.extend(self.groups[i].models.iter().map(|m| m.0));
            g = self.next[i];
        }
        out
    }
}
