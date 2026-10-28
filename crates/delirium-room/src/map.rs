use bevy::prelude::*;
use delirium_data::bounds::CollisionMesh;
use delirium_data::{bounds, img, resource};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub fn load(game: &str, at: Vec2, radius: f32) -> Result<Vec<CollisionMesh>, String> {
    thread_local! {
        static LOADED: std::cell::RefCell<std::collections::HashMap<String, Vec<CollisionMesh>>> = Default::default();
    }
    let k = format!("{game}|{at}|{radius}");
    if let Some(v) = LOADED.with_borrow(|l| l.get(&k).cloned()) {
        return Ok(v);
    }
    let v = load_near(game, at, radius)?;
    LOADED.with_borrow_mut(|l| l.insert(k, v.clone()));
    Ok(v)
}

fn load_near(game: &str, at: Vec2, radius: f32) -> Result<Vec<CollisionMesh>, String> {
    let mut out = vec![];
    let mut open: Option<(String, std::fs::File)> = None;
    for e in index(game)? {
        let near = Vec2::new(at.x.clamp(e.lo.x, e.hi.x), at.y.clamp(e.lo.y, e.hi.y)).distance(at) < radius;
        if radius != 0.0 && !near {
            continue;
        }
        if open.as_ref().is_none_or(|o| o.0 != e.img) {
            open = Some((e.img.clone(), std::fs::File::open(&e.img).map_err(|x| x.to_string())?));
        }
        let f = &mut open.as_mut().unwrap().1;
        let Ok((meshes, _)) = read_xbn(f, e.offset, e.blocks).and_then(|r| bounds::read(&r)) else { continue };
        out.extend(meshes.into_iter().filter(|m| radius == 0.0 || m.positions.iter().any(|p| Vec2::new(p[0], p[1]).distance(at) < radius)));
    }
    Ok(out)
}

struct Entry {
    img: String,
    offset: u64,
    blocks: u32,
    lo: Vec2,
    hi: Vec2,
}

fn read_xbn(f: &mut std::fs::File, offset: u64, blocks: u32) -> Result<resource::Resource, String> {
    let mut raw = vec![];
    f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
    f.take(blocks as u64 * 2048).read_to_end(&mut raw).map_err(|e| e.to_string())?;
    resource::parse(&raw)
}

fn index(game: &str) -> Result<Vec<Entry>, String> {
    let file = crate::getup::config_dir().join("mapindex.txt");
    if let Ok(t) = std::fs::read_to_string(&file) {
        let mut lines = t.lines();
        if lines.next() == Some(game) {
            let v: Option<Vec<Entry>> = lines.map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                let n = |i: usize| c.get(i)?.parse::<f32>().ok();
                Some(Entry { img: c.first()?.to_string(), offset: c.get(1)?.parse().ok()?, blocks: c.get(2)?.parse().ok()?,
                             lo: Vec2::new(n(3)?, n(4)?), hi: Vec2::new(n(5)?, n(6)?) })
            }).collect();
            if let Some(v) = v {
                return Ok(v);
            }
        }
    }
    let dir = Path::new(game);
    let key = crate::getup::key(dir)?;
    let (mut imgs, mut dirs) = (vec![], vec![dir.join(crate::getup::plat("xbox360"))]);
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("img")) {
                imgs.push(p);
            }
        }
    }
    imgs.sort();
    let mut out = vec![];
    for path in imgs {
        let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut head = vec![];
        (&mut f).take(4 << 20).read_to_end(&mut head).map_err(|e| e.to_string())?;
        for e in img::read_table(&head, Some(&key)).unwrap_or_default().into_iter().filter(|e| e.name.to_lowercase().ends_with(".xbn")) {
            let Ok((meshes, _)) = read_xbn(&mut f, e.offset, e.used_blocks as u32).and_then(|r| bounds::read(&r)) else { continue };
            let ps = meshes.iter().flat_map(|m| m.positions.iter().map(|p| Vec2::new(p[0], p[1])));
            let (lo, hi) = ps.fold((Vec2::MAX, Vec2::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
            if lo.x <= hi.x {
                out.push(Entry { img: path.to_string_lossy().into_owned(), offset: e.offset, blocks: e.used_blocks as u32, lo, hi });
            }
        }
    }
    let text: String = std::iter::once(game.to_string()).chain(out.iter().map(|e| format!("{}\t{}\t{}\t{}\t{}\t{}\t{}", e.img, e.offset, e.blocks, e.lo.x, e.lo.y, e.hi.x, e.hi.y)))
        .collect::<Vec<_>>().join("
");
    let _ = std::fs::create_dir_all(file.parent().unwrap()).and_then(|_| std::fs::write(&file, text));
    Ok(out)
}

pub fn stairs(meshes: &[CollisionMesh]) -> Vec<(Vec3, usize, f32, f32)> {
    let mut cells: std::collections::HashMap<(i32, i32), (Vec3, usize, f32, f32)> = Default::default();
    for m in meshes {
        for (t, fl) in m.triangles.iter().zip(&m.flags) {
            if fl & 1 == 0 {
                continue;
            }
            let c = t.iter().map(|&i| Vec3::from(m.positions[i as usize])).sum::<Vec3>() / 3.0;
            let e = cells.entry(((c.x / 4.0).floor() as i32, (c.y / 4.0).floor() as i32)).or_insert((Vec3::ZERO, 0, f32::MAX, f32::MIN));
            (e.0, e.1, e.2, e.3) = (e.0 + c, e.1 + 1, e.2.min(c.z), e.3.max(c.z));
        }
    }
    let mut v: Vec<_> = cells.into_values().map(|(s, n, lo, hi)| (s / n as f32, n, lo, hi)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

pub fn spots(meshes: &[CollisionMesh], near: Vec2, radius: f32) -> Vec<(&'static str, Vec3, f32, f32)> {
    let mut w = delirium::collision::Collision::default();
    for m in meshes {
        w.add_flagged(&m.positions, &m.triangles, &m.materials, &m.flags);
    }
    let ground = |p: Vec2, z: f32| w.contact(p.extend(z + 2.0), -Vec3::Z, 6.0).filter(|h| h.1.z > 0.9).map(|h| z + 2.0 - h.0);
    let open = |p: Vec3| w.cast(p + Vec3::Z * 0.3, Vec3::Z, 40.0).is_none();
    let flat = |p: Vec3, d: Vec2, len: f32| (1..=(len * 2.0) as i32).try_fold(p.z, |z, i| ground(p.truncate() + d * (i as f32 * 0.5), z).filter(|g| (g - z).abs() < 0.04)).is_some();
    let heading = |d: Vec2| (-d.x).atan2(d.y).to_degrees();
    let mut out = vec![];
    for (c, n, lo, hi) in stairs(meshes) {
        if n < 30 || hi - lo < 1.2 || c.truncate().distance(near) > radius {
            continue;
        }
        let Some(zc) = ground(c.truncate(), c.z) else { continue };
        let at = |dx: f32, dy: f32| ground(c.truncate() + Vec2::new(dx, dy), zc);
        let (Some(xp), Some(xm), Some(yp), Some(ym)) = (at(1.0, 0.0), at(-1.0, 0.0), at(0.0, 1.0), at(0.0, -1.0)) else { continue };
        let down = -Vec2::new(xp - xm, yp - ym).normalize_or_zero();
        if down == Vec2::ZERO || !open(c.truncate().extend(zc)) {
            continue;
        }
        let mut p = c.truncate().extend(zc);
        for _ in 0..80 {
            let Some(z) = ground(p.truncate() - down * 0.5, p.z) else { break };
            p = (p.truncate() - down * 0.5).extend(z);
            if flat(p, -down, 2.0) {
                break;
            }
        }
        if flat(p, -down, 2.0) && open(p) && p.z - lo > 1.0 {
            out.push(("stairs", p + down.extend(0.0) * 0.3, heading(down), p.z - lo));
        }
    }
    let r = radius.min(150.0) as i32;
    for gx in -r..=r {
        for gy in -r..=r {
            let q = near + Vec2::new(gx as f32, gy as f32);
            let Some(z) = w.contact(q.extend(200.0), -Vec3::Z, 300.0).filter(|h| h.1.z > 0.9).map(|h| 200.0 - h.0) else { continue };
            let p = q.extend(z);
            if !open(p) {
                continue;
            }
            for k in 0..8 {
                let d = Vec2::from_angle(k as f32 * std::f32::consts::FRAC_PI_4);
                let hit = |h: f32, len: f32| w.cast(p + Vec3::Z * h, d.extend(0.0), len).map(|x| x.0);
                if let (Some(a), None) = (hit(0.5, 0.6), hit(1.4, 1.2)) {
                    let top = (0..10).map(|i| 0.5 + i as f32 * 0.1).take_while(|&h| hit(h, 0.6).is_some()).last().unwrap_or(0.5);
                    let on = ground(q + d * (a + 0.1), z + top).is_some_and(|t| (t - z - top).abs() < 0.15);
                    let behind = (6..12).map(|i| ground(q + d * (a + i as f32 * 0.1), z + top)).find(|b| b.is_some_and(|b| b < z + 0.15 && b > z - 3.0)).flatten();
                    let face = w.cast(p + Vec3::Z * 0.5, d.extend(0.0), 0.6).is_some_and(|x| x.1.z.abs() < 0.2);
                    let side = d.perp();
                    let long = [-1.5f32, -0.75, 0.75, 1.5].iter().all(|&s| {
                        let o = p + (side * s).extend(0.0);
                        w.cast(o + Vec3::Z * 0.5, d.extend(0.0), a + 0.3).is_some() && w.cast(o + Vec3::Z * (top + 0.25), d.extend(0.0), a + 0.6).is_none()
                    });
                    if top >= 0.8 && on && face && long && behind.is_some() && flat(p, -d, 2.0) {
                        out.push(("wall", (q + d * (a - 0.4)).extend(z), heading(-d), (top + 0.1) * 10.0 + (z - behind.unwrap()).round()));
                    }
                }
                let below = w.contact((q + d * 1.0).extend(z + 0.5), -Vec3::Z, 20.0).map(|h| h.0 - 0.5);
                let bare = w.cast(p + Vec3::Z * 0.3, d.extend(0.0), 1.2).is_none();
                let wide = [-1.2f32, 1.2].iter().all(|&s| flat(p, d.perp() * s.signum(), s.abs()) && flat(p - (d * 2.0).extend(0.0), d.perp() * s.signum(), s.abs()));
                if below.is_some_and(|b| (4.0..12.0).contains(&b)) && bare && wide && ground(q + d * 0.5, z).is_some_and(|e| (e - z).abs() < 0.05) && flat(p, -d, 3.0) {
                    out.push(("ledge", (q + d * 0.5).extend(z), heading(-d), below.unwrap()));
                }
            }
        }
    }
    out
}
