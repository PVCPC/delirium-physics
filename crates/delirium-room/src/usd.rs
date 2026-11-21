use bevy::math::{DMat4, DQuat, DVec3, Vec2, Vec3};
use openusd::{sdf, sdf::Value, usd};

#[derive(Default, Clone, Debug)]
pub struct Tex {
    pub name: String,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Default, Clone, Debug)]
pub struct Mat {
    pub base: [f32; 4],
    pub base_tex: Option<Tex>,
    pub metallic: f32,
    pub roughness: f32,
    pub metallic_tex: Option<Tex>,
    pub roughness_tex: Option<Tex>,
    pub normal_tex: Option<Tex>,
    pub occlusion_tex: Option<Tex>,
    pub emissive: [f32; 3],
    pub emissive_tex: Option<Tex>,
}

#[derive(Default, Debug)]
pub struct Part {
    pub pos: Vec<Vec3>,
    pub nrm: Vec<Vec3>,
    pub uv: Vec<Vec2>,
    pub mat: Option<usize>,
    pub skin: Vec<([u16; 4], [f32; 4])>,
}

#[derive(Default, Debug)]
pub struct Scene {
    pub parts: Vec<Part>,
    pub mats: Vec<Mat>,
    pub joints: Vec<Vec3>,
    pub floppy: Vec<usize>,
}

fn v(p: &usd::Prim, name: &str) -> Option<Value> {
    p.attribute(name).get::<Value>().ok().flatten()
}

fn vec3s(x: Value) -> Option<Vec<Vec3>> {
    Some(match x {
        Value::Vec3fVec(a) => a.iter().map(|q| Vec3::new(q.x, q.y, q.z)).collect(),
        Value::Vec3dVec(a) => a.iter().map(|q| Vec3::new(q.x as f32, q.y as f32, q.z as f32)).collect(),
        Value::Vec3hVec(a) => a.iter().map(|q| Vec3::new(q.x.to_f32(), q.y.to_f32(), q.z.to_f32())).collect(),
        _ => return None,
    })
}

fn vec2s(x: Value) -> Option<Vec<Vec2>> {
    Some(match x {
        Value::Vec2fVec(a) => a.iter().map(|q| Vec2::new(q.x, q.y)).collect(),
        Value::Vec2dVec(a) => a.iter().map(|q| Vec2::new(q.x as f32, q.y as f32)).collect(),
        _ => return None,
    })
}

fn ints(x: Value) -> Option<Vec<i64>> {
    Some(match x {
        Value::IntVec(a) => a.iter().map(|&i| i as i64).collect(),
        Value::Int64Vec(a) => a,
        _ => return None,
    })
}

fn d3(x: &Value) -> Option<DVec3> {
    Some(match x {
        Value::Vec3f(q) => DVec3::new(q.x as f64, q.y as f64, q.z as f64),
        Value::Vec3d(q) => DVec3::new(q.x, q.y, q.z),
        Value::Vec3h(q) => DVec3::new(q.x.to_f64(), q.y.to_f64(), q.z.to_f64()),
        _ => return None,
    })
}

fn f1(x: &Value) -> Option<f64> {
    Some(match x {
        Value::Float(f) => *f as f64,
        Value::Double(f) => *f,
        Value::Half(f) => f.to_f64(),
        _ => return None,
    })
}

fn local(p: &usd::Prim) -> (DMat4, bool) {
    let Some(Value::TokenVec(order)) = v(p, "xformOpOrder") else { return (DMat4::IDENTITY, false) };
    let (mut m, mut reset) = (DMat4::IDENTITY, false);
    for op in order.iter().map(|t| t.as_str().to_string()) {
        if op == "!resetXformStack!" {
            (m, reset) = (DMat4::IDENTITY, true);
            continue;
        }
        let (inv, name) = op.strip_prefix("!invert!").map_or((false, op.as_str()), |n| (true, n));
        let Some(x) = v(p, name) else { continue };
        let kind = name.trim_start_matches("xformOp:").split(':').next().unwrap_or("");
        let r = |a: f64| a.to_radians();
        let t = match kind {
            "transform" => match &x { Value::Matrix4d(q) => DMat4::from_cols_array(&q.0), _ => continue },
            "translate" => DMat4::from_translation(d3(&x).unwrap_or_default()),
            "scale" => DMat4::from_scale(d3(&x).or(f1(&x).map(DVec3::splat)).unwrap_or(DVec3::ONE)),
            "orient" => DMat4::from_quat(match &x {
                Value::Quatf(q) => DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64),
                Value::Quatd(q) => DQuat::from_xyzw(q.x, q.y, q.z, q.w),
                Value::Quath(q) => DQuat::from_xyzw(q.x.to_f64(), q.y.to_f64(), q.z.to_f64(), q.w.to_f64()),
                _ => continue,
            }.normalize()),
            "rotateX" => DMat4::from_rotation_x(r(f1(&x).unwrap_or(0.0))),
            "rotateY" => DMat4::from_rotation_y(r(f1(&x).unwrap_or(0.0))),
            "rotateZ" => DMat4::from_rotation_z(r(f1(&x).unwrap_or(0.0))),
            k if k.starts_with("rotate") && k.len() == 9 => {
                let a = d3(&x).unwrap_or_default();
                k[6..].chars().fold(DMat4::IDENTITY, |acc, c| {
                    let one = match c { 'X' => DMat4::from_rotation_x(r(a.x)), 'Y' => DMat4::from_rotation_y(r(a.y)), _ => DMat4::from_rotation_z(r(a.z)) };
                    one * acc
                })
            }
            _ => continue,
        };
        m *= if inv { t.inverse() } else { t };
    }
    (m, reset)
}

fn input(stage: &usd::Stage, shader: &usd::Prim, name: &str, dir: &Option<String>, zip: &mut Option<openusd::usdz::Archive>) -> (Option<Value>, Option<Tex>) {
    let a = shader.attribute(format!("inputs:{name}"));
    if let Some(src) = a.connections().ok().and_then(|c| c.into_iter().next()) {
        if let Ok(tex) = stage.prim(src.prim_path()) {
            if let Some(Value::AssetPath(f)) = v(&tex, "inputs:file") {
                let name = f.authored_path.clone();
                let bytes = match zip {
                    Some(z) => z.entry(name.trim_start_matches("./")).ok().map(|b| b.to_vec()),
                    None => dir.as_ref().and_then(|d| std::fs::read(std::path::Path::new(d).join(&name)).ok()),
                };
                return (None, Some(Tex { name, bytes }));
            }
            return (v(&tex, "inputs:fallback"), None);
        }
    }
    (a.get::<Value>().ok().flatten(), None)
}

fn material(stage: &usd::Stage, path: &sdf::Path, dir: &Option<String>, zip: &mut Option<openusd::usdz::Archive>) -> Mat {
    let mut m = Mat { base: [0.8, 0.8, 0.8, 1.0], roughness: 0.5, ..Default::default() };
    let Ok(mp) = stage.prim(path.clone()) else { return m };
    let surf = mp.attribute("outputs:surface").connections().ok().and_then(|c| c.into_iter().next())
        .or_else(|| mp.attribute("outputs:ri:surface").connections().ok().and_then(|c| c.into_iter().next()));
    let Some(sp) = surf.and_then(|s| stage.prim(s.prim_path()).ok()) else { return m };
    let (c, t) = input(stage, &sp, "diffuseColor", dir, zip);
    if let Some(c) = c.as_ref().and_then(d3) {
        m.base = [c.x as f32, c.y as f32, c.z as f32, 1.0];
    }
    if t.is_some() {
        m.base = [1.0; 4];
    }
    m.base_tex = t;
    if let Some(o) = input(stage, &sp, "opacity", dir, zip).0.as_ref().and_then(f1) {
        m.base[3] = o as f32;
    }
    let (c, t) = input(stage, &sp, "metallic", dir, zip);
    m.metallic = c.as_ref().and_then(f1).unwrap_or(if t.is_some() { 1.0 } else { 0.0 }) as f32;
    m.metallic_tex = t;
    let (c, t) = input(stage, &sp, "roughness", dir, zip);
    m.roughness = c.as_ref().and_then(f1).unwrap_or(if t.is_some() { 1.0 } else { 0.5 }) as f32;
    m.roughness_tex = t;
    m.normal_tex = input(stage, &sp, "normal", dir, zip).1;
    m.occlusion_tex = input(stage, &sp, "occlusion", dir, zip).1;
    let (c, t) = input(stage, &sp, "emissiveColor", dir, zip);
    if let Some(c) = c.as_ref().and_then(d3) {
        m.emissive = [c.x as f32, c.y as f32, c.z as f32];
    }
    if t.is_some() {
        m.emissive = [1.0; 3];
    }
    m.emissive_tex = t;
    m
}

fn corners<T: Copy>(vals: Vec<T>, p: &usd::Prim, name: &str, idx: &[i64], counts: &[i64]) -> Option<Vec<T>> {
    let a = p.attribute(name);
    let interp: Option<String> = a.get_metadata::<Value>("interpolation").ok().flatten().and_then(|x| match x { Value::Token(t) => Some(t.as_str().to_string()), _ => None });
    let ix = v(p, &format!("{name}:indices")).and_then(ints);
    let at = |k: usize| -> Option<T> { let k = ix.as_ref().map_or(Some(k), |i| i.get(k).map(|&j| j as usize))?; vals.get(k).copied() };
    let n = idx.len();
    let interp = interp.unwrap_or_else(|| if vals.len() == n && ix.is_none() { "faceVarying".into() } else { "vertex".into() });
    let mut out = Vec::with_capacity(n);
    match interp.as_str() {
        "faceVarying" => for k in 0..n { out.push(at(k)?) },
        "uniform" => for (f, &c) in counts.iter().enumerate() { for _ in 0..c { out.push(at(f)?) } },
        "constant" => for _ in 0..n { out.push(at(0)?) },
        _ => for &k in idx { out.push(at(k as usize)?) },
    }
    Some(out)
}

pub fn read(path: &str) -> Result<Scene, String> {
    let stage = usd::Stage::open(path).map_err(|e| e.to_string())?;
    let mut zip = path.to_ascii_lowercase().ends_with(".usdz").then(|| openusd::usdz::Archive::open(path).ok()).flatten();
    let dir = std::path::Path::new(path).parent().map(|d| d.to_string_lossy().into_owned());
    let root = stage.prim(sdf::Path::abs_root()).map_err(|e| e.to_string())?;
    let up_z = matches!(root.get_metadata::<Value>("upAxis").ok().flatten(), Some(Value::Token(t)) if t.as_str() == "Z");
    let mpu = root.get_metadata::<Value>("metersPerUnit").ok().flatten().as_ref().and_then(f1).unwrap_or(0.01);
    let fix = DMat4::from_scale(DVec3::splat(mpu)) * if up_z { DMat4::IDENTITY } else { DMat4::from_rotation_x(std::f64::consts::FRAC_PI_2) };
    let mut scene = Scene::default();
    let mut mats: Vec<(sdf::Path, usize)> = vec![];
    let mut st = vec![(root, fix)];
    while let Some((p, parent)) = st.pop() {
        if !p.is_active().unwrap_or(true) {
            continue;
        }
        let ty = p.type_name().ok().flatten().map(|t| t.as_str().to_string()).unwrap_or_default();
        let (l, reset) = local(&p);
        let world = if reset { fix * l } else { parent * l };
        if ty == "Mesh" && let (Some(pts), Some(idx), Some(counts)) = (v(&p, "points").and_then(vec3s), v(&p, "faceVertexIndices").and_then(ints), v(&p, "faceVertexCounts").and_then(ints)) {
            let nrm = v(&p, "normals").and_then(vec3s).and_then(|n| corners(n, &p, "normals", &idx, &counts))
                .or_else(|| v(&p, "primvars:normals").and_then(vec3s).and_then(|n| corners(n, &p, "primvars:normals", &idx, &counts)));
            let stn = ["primvars:st", "primvars:st0", "primvars:UVMap", "primvars:uv", "primvars:map1"].into_iter().find(|n| p.attribute(*n).get::<Value>().ok().flatten().is_some());
            let uv = stn.and_then(|n| v(&p, n).and_then(vec2s).and_then(|u| corners(u, &p, n, &idx, &counts)));
            let left = matches!(v(&p, "orientation"), Some(Value::Token(t)) if t.as_str() == "leftHanded");
            let wm = world.as_mat4();
            let nm = wm.inverse().transpose();
            let flip = left != (wm.determinant() < 0.0);
            let mat = p.relationship("material:binding").targets().ok().and_then(|t| t.into_iter().next()).map(|mp| {
                match mats.iter().find(|m| m.0 == mp) {
                    Some(m) => m.1,
                    None => {
                        scene.mats.push(material(&stage, &mp, &dir, &mut zip));
                        mats.push((mp, scene.mats.len() - 1));
                        scene.mats.len() - 1
                    }
                }
            });
            let mut part = Part { mat, ..Default::default() };
            let mut k = 0usize;
            for &c in &counts {
                let c = c.max(0) as usize;
                for j in 1..c.saturating_sub(1) {
                    let mut tri = [k, k + j, k + j + 1];
                    if flip {
                        tri.swap(1, 2);
                    }
                    if tri.iter().any(|&q| q >= idx.len() || idx[q] as usize >= pts.len()) {
                        continue;
                    }
                    for q in tri {
                        part.pos.push(wm.transform_point3(pts[idx[q] as usize]));
                        if let Some(n) = &nrm {
                            part.nrm.push(nm.transform_vector3(n[q]).normalize_or_zero() * if left { -1.0 } else { 1.0 });
                        }
                        if let Some(u) = &uv {
                            part.uv.push(Vec2::new(u[q].x, 1.0 - u[q].y));
                        }
                    }
                }
                k += c;
            }
            if part.nrm.len() != part.pos.len() {
                part.nrm.clear();
            }
            if part.uv.len() != part.pos.len() {
                part.uv.clear();
            }
            if !part.pos.is_empty() {
                scene.parts.push(part);
            }
        }
        for c in p.children().unwrap_or_default().into_iter().rev() {
            st.push((c, world));
        }
    }
    Ok(scene)
}
