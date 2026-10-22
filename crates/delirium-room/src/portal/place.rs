use bevy::prelude::*;

use super::{through, Frame};

const U: f32 = 0.0254;
const MAX_BUMPS: usize = 5;
const MAX_SEGMENTS: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

pub trait World {
    fn ray(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Hit>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    NoHit,
    DoesNotFit,
    Overlap,
}

impl Frame {
    fn right(&self) -> Vec3 {
        self.rot * Vec3::X
    }
    fn up(&self) -> Vec3 {
        self.rot * Vec3::Y
    }
}

fn max_slide(half: Vec2) -> f32 {
    (2.0 * (half.x * half.x + half.y * half.y)).sqrt()
}

fn surface_at(w: &dyn World, p: Vec3, n: Vec3) -> bool {
    w.ray(p + n * U, -n, 2.0 * U).is_some_and(|h| h.distance > 0.0 && (h.distance - U).abs() < 0.5 * U && h.normal.dot(n) > 0.98)
}

fn obstruction(w: &dyn World, from: Vec3, to: Vec3, n: Vec3) -> Option<f32> {
    let d = to - from;
    let len = d.length();
    if len <= 0.0 {
        return None;
    }
    w.ray(from + n * U, d / len, len).map(|h| h.distance)
}

fn blocked_along(w: &dyn World, from: Vec3, to: Vec3, n: Vec3) -> Option<f32> {
    let len = from.distance(to);
    if len <= 0.0 {
        return None;
    }
    let dir = (to - from) / len;
    let ok = |t: f32| surface_at(w, from + dir * t, n);
    let mut best = obstruction(w, from, to, n);
    let step = len * 0.05;
    let mut good = 0.0f32;
    let mut s = step;
    while s <= len + 1e-6 {
        let t = s.min(len);
        if !ok(t) {
            let (mut lo, mut hi) = (good, t);
            for _ in 0..100 {
                if hi - lo <= 0.01 * U {
                    break;
                }
                let mid = 0.5 * (lo + hi);
                if ok(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            best = Some(best.map_or(lo, |b: f32| b.min(lo)));
            break;
        }
        good = t;
        s += step;
    }
    best
}

#[derive(Clone, Copy)]
struct Block {
    dir: Vec3,
    t: f32,
    half: f32,
}

impl Block {
    fn push(&self) -> f32 {
        self.half - self.t + 2.0 * U
    }
}

fn opposing(w: &dyn World, n: Vec3, pos: Vec3, a: Block, b: Block) -> Option<Vec3> {
    let free = |dir: Vec3, h: f32| surface_at(w, pos - dir * 2.0 * h, n);
    if free(a.dir, a.half) {
        Some(-a.dir * a.half)
    } else if free(b.dir, b.half) {
        Some(-b.dir * b.half)
    } else {
        None
    }
}

fn corner(a: Block, b: Block) -> Vec3 {
    let (pa, pb, d) = (a.push(), b.push(), a.dir.dot(b.dir));
    let det = 1.0 - d * d;
    let al = (-pa + d * pb) / det;
    let be = (-pb + d * pa) / det;
    al * a.dir + be * b.dir
}

fn fit_on_surface(w: &dyn World, frame: Frame, half: Vec2) -> Option<Vec3> {
    let n = frame.normal();
    let (r, u) = (frame.right(), frame.up());
    let start = frame.at;
    let mut pos = start;
    for depth in 0..=MAX_BUMPS {
        let mut blocks: Vec<Block> = Vec::with_capacity(4);
        for (dir, h) in [(u, half.y), (-u, half.y), (r, half.x), (-r, half.x)] {
            let reach = (h - 2.0 * U).max(0.0);
            if let Some(t) = blocked_along(w, pos, pos + dir * reach, n) {
                blocks.push(Block { dir, t, half: h });
            }
        }
        if blocks.is_empty() {
            return ((pos - start).length() <= max_slide(half) + 1e-5).then_some(pos);
        }
        if depth == MAX_BUMPS {
            return None;
        }
        let delta = match blocks.as_slice() {
            [a] => -a.dir * a.push(),
            [a, b] if a.dir.dot(b.dir) < -0.9 => opposing(w, n, pos, *a, *b)?,
            [a, b] => corner(*a, *b),
            [_, _, _] => {
                let (i, j, k) = [(0, 1, 2), (0, 2, 1), (1, 2, 0)].into_iter().find(|&(i, j, _)| blocks[i].dir.dot(blocks[j].dir) < -0.9)?;
                opposing(w, n, pos, blocks[i], blocks[j])? + (-blocks[k].dir * blocks[k].push())
            }
            _ => return None,
        };
        pos += delta;
    }
    None
}

fn floor_snap(w: &dyn World, frame: &Frame, half: Vec2) -> Vec3 {
    let (n, up) = (frame.normal(), frame.up());
    if up.z <= 0.7 {
        return frame.at;
    }
    let from = frame.at + n * (0.05 * U);
    let far = w.ray(from, -up, half.y + 1.5 * U);
    let near = w.ray(from, -up, half.y - 0.1 * U);
    match (far, near) {
        (Some(h), None) => h.point - n * (0.05 * U) + up * half.y,
        _ => frame.at,
    }
}

fn corners_backed(w: &dyn World, frame: &Frame, half: Vec2) -> bool {
    let (n, r, u) = (frame.normal(), frame.right(), frame.up());
    let (hw, hh) = (half.x - 2.2 * U, half.y - 2.2 * U);
    [(0.0, 0.0), (hw, hh), (-hw, hh), (hw, -hh), (-hw, -hh)].iter().all(|&(x, y)| surface_at(w, frame.at + r * x + u * y, n))
}

fn obb_overlap(ac: Vec3, aa: [Vec3; 3], ah: [f32; 3], bc: Vec3, ba: [Vec3; 3], bh: [f32; 3]) -> bool {
    let d = bc - ac;
    let mut axes: Vec<Vec3> = Vec::with_capacity(15);
    axes.extend_from_slice(&aa);
    axes.extend_from_slice(&ba);
    for x in aa {
        for y in ba {
            let c = x.cross(y);
            if c.length_squared() > 1e-8 {
                axes.push(c.normalize());
            }
        }
    }
    axes.into_iter().all(|axis| {
        let ra: f32 = (0..3).map(|i| ah[i] * aa[i].dot(axis).abs()).sum();
        let rb: f32 = (0..3).map(|i| bh[i] * ba[i].dot(axis).abs()).sum();
        d.dot(axis).abs() <= ra + rb
    })
}

fn overlaps(new: &Frame, new_half: Vec2, other: &Frame, other_half: Vec2) -> bool {
    if new.normal().dot(other.normal()) < 0.95 {
        return false;
    }
    let a_c = new.at + new.normal() * (0.5 * U);
    let b_c = other.at + other.normal() * (32.0 * U);
    obb_overlap(a_c, [new.normal(), new.right(), new.up()], [0.5 * U, new_half.x, new_half.y],
                b_c, [other.normal(), other.right(), other.up()], [32.0 * U, other_half.x, other_half.y])
}

fn initial_frame(point: Vec3, normal: Vec3, shot_dir: Vec3) -> Option<Frame> {
    let n = normal.try_normalize()?;
    let vertical = n.x.abs() < 0.001 && n.y.abs() < 0.001;
    Frame::new(point, n, if vertical { shot_dir } else { Vec3::Z }).or_else(|| Frame::new(point, n, Vec3::Y))
}

pub fn place(w: &dyn World, hit: Hit, shot_dir: Vec3, half: Vec2, others: &[Frame]) -> Result<Frame, Fail> {
    let frame = initial_frame(hit.point, hit.normal, shot_dir).ok_or(Fail::DoesNotFit)?;
    if !surface_at(w, frame.at, frame.normal()) {
        return Err(Fail::DoesNotFit);
    }
    let mut fitted = Frame { at: fit_on_surface(w, frame, half).ok_or(Fail::DoesNotFit)?, rot: frame.rot };
    fitted.at = floor_snap(w, &fitted, half);
    let grid = U / 512.0;
    fitted.at = (fitted.at / grid).round() * grid;
    if others.iter().any(|o| overlaps(&fitted, half, o, half)) {
        return Err(Fail::Overlap);
    }
    if !corners_backed(w, &fitted, half) {
        return Err(Fail::DoesNotFit);
    }
    Ok(fitted)
}

#[derive(Clone, Copy)]
pub struct Window {
    pub frame: Frame,
    pub exit: Frame,
}

pub struct Trace {
    pub hit: Option<Hit>,
    pub dir: Vec3,
}

pub fn trace_shot(w: &dyn World, origin: Vec3, dir: Vec3, range: f32, half: Vec2, windows: &[Window]) -> Trace {
    let mut o = origin;
    let mut d = dir.normalize();
    let mut remaining = range;
    for _ in 0..MAX_SEGMENTS {
        let hit = w.ray(o, d, remaining);
        let limit = hit.map_or(remaining, |h| h.distance);
        let mut best: Option<(f32, Window)> = None;
        for win in windows {
            let n = win.frame.normal();
            let denom = d.dot(n);
            if denom >= -1e-5 {
                continue;
            }
            let t = (win.frame.at - o).dot(n) / denom;
            if t < 1e-3 || t > limit + 0.01 {
                continue;
            }
            let l = win.frame.local(o + d * t);
            if l.x.abs() > half.x || l.y.abs() > half.y {
                continue;
            }
            if best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, *win));
            }
        }
        if let Some((t, win)) = best {
            let (turn, map) = through(&win.frame, &win.exit);
            let p = o + d * t;
            let out = (turn * d).normalize();
            o = map(p) + out * 1e-3;
            d = out;
            remaining -= t;
            continue;
        }
        return Trace { hit, dir: d };
    }
    Trace { hit: None, dir: d }
}
