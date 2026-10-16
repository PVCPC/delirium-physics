use bevy::prelude::*;
use delirium_data::bounds::CollisionMesh;
use delirium_data::{bounds, img, resource};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub fn load(game: &str, at: Vec2, radius: f32) -> Result<Vec<CollisionMesh>, String> {
    let dir = Path::new(game);
    let key = crate::getup::key(dir)?;
    let (mut imgs, mut dirs) = (vec![], vec![dir.join("xbox360")]);
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
    let mut out = vec![];
    for path in imgs {
        let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut head = vec![];
        (&mut f).take(4 << 20).read_to_end(&mut head).map_err(|e| e.to_string())?;
        for e in img::read_table(&head, Some(&key)).unwrap_or_default().into_iter().filter(|e| e.name.to_lowercase().ends_with(".xbn")) {
            let mut raw = vec![];
            f.seek(SeekFrom::Start(e.offset)).map_err(|e| e.to_string())?;
            (&mut f).take(e.used_blocks as u64 * 2048).read_to_end(&mut raw).map_err(|e| e.to_string())?;
            let Ok((meshes, _)) = resource::parse(&raw).and_then(|r| bounds::read(&r)) else { continue };
            out.extend(meshes.into_iter().filter(|m| radius == 0.0 || m.positions.iter().any(|p| Vec2::new(p[0], p[1]).distance(at) < radius)));
        }
    }
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
