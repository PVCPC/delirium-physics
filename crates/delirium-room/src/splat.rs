use bevy::math::{Vec3, Vec4};
use bevy_gaussian_splatting::Gaussian3d;

#[derive(Clone, Copy, Debug)]
pub struct Splat {
    pub pos: Vec3,
    pub scale: Vec3,
    pub alpha: f32,
}

fn header(d: &[u8]) -> Option<(usize, Vec<(String, usize, Vec<(String, String)>)>)> {
    let end = d.windows(11).position(|w| w == b"end_header\n")? + 11;
    let text = std::str::from_utf8(&d[..end]).ok()?;
    let mut els: Vec<(String, usize, Vec<(String, String)>)> = vec![];
    for l in text.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        match w.as_slice() {
            ["element", name, n] => els.push((name.to_string(), n.parse().ok()?, vec![])),
            ["property", ty, name] => els.last_mut()?.2.push((ty.to_string(), name.to_string())),
            _ => {}
        }
    }
    Some((end, els))
}

fn size(ty: &str) -> usize {
    match ty {
        "char" | "uchar" | "int8" | "uint8" => 1,
        "short" | "ushort" | "int16" | "uint16" => 2,
        "double" | "float64" => 8,
        _ => 4,
    }
}

fn f32_at(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

pub fn read_ply(d: &[u8]) -> Result<(Vec<Splat>, Vec<Gaussian3d>), String> {
    let mut drawn: Vec<Gaussian3d> = vec![];
    let mut draw = |s: &Splat, rot: [f32; 4], rgb: [f32; 3]| {
        let mut g = Gaussian3d::default();
        g.position_visibility = [s.pos.x, s.pos.y, s.pos.z, 1.0].into();
        let n = rot.iter().map(|r| r * r).sum::<f32>().sqrt().max(1e-9);
        g.rotation = rot.map(|r| r / n).into();
        g.scale_opacity = [s.scale.x, s.scale.y, s.scale.z, s.alpha].into();
        for (k, c) in rgb.iter().enumerate() {
            g.spherical_harmonic.set(k, *c);
        }
        drawn.push(g);
    };
    let (mut at, els) = header(d).ok_or("not a binary .ply")?;
    let mut chunks: Vec<[f32; 18]> = vec![];
    let mut out = vec![];
    for (name, n, props) in &els {
        let stride: usize = props.iter().map(|p| size(&p.0)).sum();
        let need = at + stride * n;
        if need > d.len() {
            return Err(format!("{name}: the file ends early"));
        }
        let col = |k: &str| -> Option<usize> {
            let i = props.iter().position(|p| p.1 == k)?;
            Some(props[..i].iter().map(|p| size(&p.0)).sum())
        };
        match name.as_str() {
            "chunk" => {
                for r in 0..*n {
                    let o = at + r * stride;
                    let mut c = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0f32];
                    for (k, v) in c.iter_mut().enumerate().take((stride / 4).min(18)) {
                        *v = f32_at(d, o + k * 4);
                    }
                    chunks.push(c);
                }
            }
            "vertex" if col("packed_position").is_some() => {
                let (pp, pr, ps, pc) = (col("packed_position").unwrap(), col("packed_rotation").ok_or("no packed_rotation")?,
                                        col("packed_scale").ok_or("no packed_scale")?, col("packed_color").ok_or("no packed_color")?);
                let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
                for r in 0..*n {
                    let o = at + r * stride;
                    let Some(c) = chunks.get(r / 256) else { return Err("fewer chunks than splats need".into()) };
                    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
                    let p = u(o + pp);
                    let t = Vec3::new((p >> 21) as f32 / 2047.0, (p >> 11 & 1023) as f32 / 1023.0, (p & 2047) as f32 / 2047.0);
                    let s = u(o + ps);
                    let ts = Vec3::new((s >> 21) as f32 / 2047.0, (s >> 11 & 1023) as f32 / 1023.0, (s & 2047) as f32 / 2047.0);
                    let col = u(o + pc);
                    let a = (col & 255) as f32 / 255.0;
                    let q = Splat {
                        pos: Vec3::new(lerp(c[0], c[3], t.x), lerp(c[1], c[4], t.y), lerp(c[2], c[5], t.z)),
                        scale: Vec3::new(lerp(c[6], c[9], ts.x).exp(), lerp(c[7], c[10], ts.y).exp(), lerp(c[8], c[11], ts.z).exp()),
                        alpha: a,
                    };
                    let rr = u(o + pr);
                    let part = |sh: u32| ((rr >> sh & 1023) as f32 / 1023.0 - 0.5) * std::f32::consts::SQRT_2;
                    let (x, y, z) = (part(20), part(10), part(0));
                    let m = (1.0 - (x * x + y * y + z * z)).max(0.0).sqrt();
                    let rot = match rr >> 30 { 0 => [m, x, y, z], 1 => [x, m, y, z], 2 => [x, y, m, z], _ => [x, y, z, m] };
                    let rgb = [24u32, 16, 8].map(|sh| col >> sh & 255);
                    let dc = |k: usize| (lerp(c[12 + k], c[15 + k], rgb[k] as f32 / 255.0) - 0.5) / 0.282_094_8;
                    draw(&q, rot, [dc(0), dc(1), dc(2)]);
                    out.push(q);
                }
            }
            "vertex" => {
                let need = |k: &str| col(k).ok_or(format!("no {k}"));
                let (x, y, z, o_, s0, s1, s2) = (need("x")?, need("y")?, need("z")?, need("opacity")?, need("scale_0")?, need("scale_1")?, need("scale_2")?);
                for r in 0..*n {
                    let o = at + r * stride;
                    let f = |k: usize| f32_at(d, o + k);
                    let q = Splat { pos: Vec3::new(f(x), f(y), f(z)), scale: Vec3::new(f(s0).exp(), f(s1).exp(), f(s2).exp()), alpha: 1.0 / (1.0 + (-f(o_)).exp()) };
                    let opt = |k: &str, or: f32| col(k).map_or(or, f);
                    draw(&q, [opt("rot_0", 1.0), opt("rot_1", 0.0), opt("rot_2", 0.0), opt("rot_3", 0.0)], [opt("f_dc_0", 0.0), opt("f_dc_1", 0.0), opt("f_dc_2", 0.0)]);
                    out.push(q);
                }
            }
            _ => {}
        }
        at = need;
    }
    if out.is_empty() {
        return Err("no splats".into());
    }
    Ok((out, drawn))
}

pub fn surface(s: &[Splat], alpha: f32, big: f32, cells: usize, min: usize) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let keep: Vec<Vec3> = s.iter().filter(|q| q.alpha >= alpha && q.scale.max_element() <= big && q.pos.is_finite()).map(|q| q.pos).collect();
    if keep.len() < 8 {
        return (vec![], vec![]);
    }
    let centile = |k: usize, q: f32| -> f32 {
        let mut a: Vec<f32> = keep.iter().map(|p| p[k]).collect();
        let i = ((a.len() - 1) as f32 * q) as usize;
        *a.select_nth_unstable_by(i, f32::total_cmp).1
    };
    let lo = Vec3::new(centile(0, 0.01), centile(1, 0.01), centile(2, 0.01));
    let hi = Vec3::new(centile(0, 0.99), centile(1, 0.99), centile(2, 0.99));
    let cell = ((hi - lo).max_element() / cells.max(1) as f32).max(1e-4);
    let n = ((hi - lo) / cell).ceil().as_uvec3() + 1;
    let idx = |c: [i64; 3]| -> Option<usize> {
        (c.iter().zip(n.to_array()).all(|(&v, m)| v >= 0 && v < m as i64)).then(|| (c[0] + c[1] * n.x as i64 + c[2] * (n.x * n.y) as i64) as usize)
    };
    let mut count = vec![0u32; (n.x * n.y * n.z) as usize];
    for p in &keep {
        let c = ((*p - lo) / cell).floor();
        if let Some(i) = idx([c.x as i64, c.y as i64, c.z as i64]) {
            count[i] += 1;
        }
    }
    let solid = |c: [i64; 3]| idx(c).is_some_and(|i| count[i] >= min as u32);
    let (mut v, mut t) = (vec![], vec![]);
    for z in 0..n.z as i64 {
        for y in 0..n.y as i64 {
            for x in 0..n.x as i64 {
                if !solid([x, y, z]) {
                    continue;
                }
                for (axis, dir) in [(0, 1i64), (0, -1), (1, 1), (1, -1), (2, 1), (2, -1)] {
                    let mut nb = [x, y, z];
                    nb[axis] += dir;
                    if solid(nb) {
                        continue;
                    }
                    let (u, w) = ((axis + 1) % 3, (axis + 2) % 3);
                    let mut corner = |a: f32, b: f32| {
                        let mut q = Vec4::new(x as f32, y as f32, z as f32, 0.0).truncate();
                        q[axis] += if dir > 0 { 1.0 } else { 0.0 };
                        q[u] += a;
                        q[w] += b;
                        v.push(lo + q * cell);
                        v.len() as u32 - 1
                    };
                    let (a, b, c, d) = (corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0));
                    if dir > 0 {
                        t.extend([[a, b, c], [a, c, d]]);
                    } else {
                        t.extend([[a, c, b], [a, d, c]]);
                    }
                }
            }
        }
    }
    (v, t)
}

pub struct Drawing;

impl bevy::app::Plugin for Drawing {
    fn build(&self, app: &mut bevy::app::App) {
        use bevy_gaussian_splatting::{gaussian, material, query, render, Gaussian3d, Gaussian4d, PlanarStoragePlugin};
        app.register_type::<bevy_gaussian_splatting::SphericalHarmonicCoefficients>();
        app.add_plugins((bevy_gaussian_splatting::camera::GaussianCameraPlugin, gaussian::settings::SettingsPlugin,
                         gaussian::cloud::CloudPlugin::<Gaussian3d>::default(), gaussian::cloud::CloudPlugin::<Gaussian4d>::default()));
        app.add_plugins((PlanarStoragePlugin::<Gaussian3d>::default(), PlanarStoragePlugin::<Gaussian4d>::default()));
        app.add_plugins((render::RenderPipelinePlugin::<Gaussian3d>::default(), render::RenderPipelinePlugin::<Gaussian4d>::default()));
        app.add_plugins((material::MaterialPlugin, query::QueryPlugin));
    }
}
