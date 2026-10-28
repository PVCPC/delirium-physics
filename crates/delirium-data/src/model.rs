use crate::resource::{system_size, Resource};

#[derive(Default, Clone)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub texcoords: Vec<Vec<[f32; 2]>>,
    pub colors: Vec<[u8; 4]>,
    pub tangents: Vec<[f32; 4]>,
    pub weights: Vec<[u8; 4]>,
    pub joints: Vec<[u8; 4]>,
    pub bone_map: Vec<u16>,
    pub indices: Vec<u32>,
    pub texture: Option<String>,
    pub shader: String,
    pub params: ShaderParams,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Declaration {
    pub usage: u32,
    pub stride: u8,
    pub types: u64,
}

const POSITION: u32 = 0;
const BLEND_WEIGHT: u32 = 1;
const BLEND_INDEX: u32 = 2;
const NORMAL: u32 = 3;
const COLOR: u32 = 4;
const TEXCOORD0: u32 = 6;
const TANGENT0: u32 = 14;

pub fn type_size(t: u32) -> Option<usize> {
    match t {
        1 | 9 | 0xA => Some(crate::kv!(1 int)),
        6 => Some(crate::kv!(52 int)),
        _ => None,
    }
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + crate::kv!(1 int)).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
}
fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2).map(|b| crate::u16_of([b[0], b[1]])).ok_or("Read out of range".into())
}

pub(crate) fn ptr(r: &Resource, p: u32) -> Result<usize, String> {
    let off = (p & crate::kv!(82 int)) as usize;
    match p >> crate::kv!(81 int) {
        5 => Ok(off),
        6 => Ok(system_size(r.flags) + off),
        _ => Err(format!("Bad resource pointer {p:08X}")),
    }
}

fn array(r: &Resource, at: usize) -> Result<Vec<usize>, String> {
    let count = be16(&r.data, at + crate::kv!(1 int))? as usize;
    if count == 0 {
        return Ok(vec![]);
    }
    let list = ptr(r, be32(&r.data, at)?)?;
    (0..count).map(|i| ptr(r, be32(&r.data, list + i * crate::kv!(1 int))?)).collect()
}

fn c_string(d: &[u8], at: usize) -> String {
    String::from_utf8_lossy(d[at..].split(|&b| b == 0).next().unwrap_or_default()).into_owned()
}

fn shaders(r: &Resource, group: usize) -> Result<Vec<(Option<String>, String)>, String> {
    let d = &r.data;
    array(r, group + crate::kv!(23 int))?
        .into_iter()
        .map(|shader| {
            let preset = ptr(r, be32(d, shader + crate::kv!(475 int))?).map(|at| c_string(d, at)).unwrap_or_default();
            let count = be32(d, shader + crate::kv!(81 int))? as usize;
            if count == 0 {
                return Ok((None, preset));
            }
            let (params, types) = (ptr(r, be32(d, shader + crate::kv!(118 int))?)?, ptr(r, be32(d, shader + crate::kv!(90 int))?)?);
            for i in 0..count {
                let value = be32(d, params + i * crate::kv!(1 int))?;
                if d[types + i] == 0 && value != 0 {
                    let tex = ptr(r, value)?;
                    return Ok((Some(c_string(d, ptr(r, be32(d, tex + crate::kv!(118 int))?)?)), preset));
                }
            }
            Ok((None, preset))
        })
        .collect()
}

#[derive(Default, Clone, Debug)]
pub struct ShaderParams {
    pub preset: String,
    pub textures: Vec<(u32, String)>,
    pub vectors: Vec<(u32, [f32; 4])>,
}

pub fn shader_params(r: &Resource, drawable: usize) -> Result<Vec<ShaderParams>, String> {
    let d = &r.data;
    array(r, ptr(r, be32(d, drawable + crate::kv!(23 int))?)? + crate::kv!(23 int))?
        .into_iter()
        .map(|shader| {
            let mut out = ShaderParams { preset: ptr(r, be32(d, shader + crate::kv!(475 int))?).map(|at| c_string(d, at)).unwrap_or_default(), ..Default::default() };
            let count = be32(d, shader + crate::kv!(81 int))? as usize;
            if count == 0 {
                return Ok(out);
            }
            let (params, types, hashes) =
                (ptr(r, be32(d, shader + crate::kv!(118 int))?)?, ptr(r, be32(d, shader + crate::kv!(90 int))?)?, ptr(r, be32(d, shader + crate::kv!(376 int))?)?);
            for i in 0..count {
                let (value, hash) = (be32(d, params + i * crate::kv!(1 int))?, be32(d, hashes + i * crate::kv!(1 int))?);
                if value == 0 {
                    continue;
                }
                let at = ptr(r, value)?;
                if d[types + i] == 0 {
                    if let Ok(name) = ptr(r, be32(d, at + crate::kv!(118 int))?) {
                        out.textures.push((hash, c_string(d, name)));
                    }
                } else {
                    let f = |k: usize| be32(d, at + k * crate::kv!(1 int)).map(f32::from_bits);
                    out.vectors.push((hash, [f(0)?, f(1)?, f(2)?, f(crate::kv!(0 int))?]));
                }
            }
            Ok(out)
        })
        .collect()
}

pub fn drawables(r: &Resource) -> Result<Vec<usize>, String> {
    if be32(&r.data, 0)? & crate::kv!(720 int) == crate::kv!(836 int) { array(r, crate::kv!(96 int)) } else { Ok(vec![0]) }
}

pub fn declarations(r: &Resource) -> Result<Vec<Declaration>, String> {
    let mut out = vec![];
    for at in drawables(r)? {
        for (_, g) in geometries(r, at)? {
            out.push(declaration(r, g)?);
        }
    }
    Ok(out)
}

fn declaration(r: &Resource, geometry: usize) -> Result<Declaration, String> {
    let vb = ptr(r, be32(&r.data, geometry + crate::kv!(52 int))?)?;
    let decl = ptr(r, be32(&r.data, vb + crate::kv!(51 int))?)?;
    let d = &r.data;
    Ok(Declaration { usage: be32(d, decl)?, stride: d[decl + crate::kv!(1 int)], types: u64::from_be_bytes(d[decl + crate::kv!(23 int)..decl + crate::kv!(51 int)].try_into().unwrap()) })
}

fn geometries(r: &Resource, drawable: usize) -> Result<Vec<(usize, usize)>, String> {
    let d = &r.data;
    let lod = be32(d, drawable + crate::kv!(251 int))?;
    if lod == 0 {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for model in array(r, ptr(r, lod)?)? {
        let shaders = ptr(r, be32(d, model + crate::kv!(51 int))?)?;
        for (i, g) in array(r, model + crate::kv!(1 int))?.into_iter().enumerate() {
            out.push((be16(d, shaders + i * 2)? as usize, g));
        }
    }
    Ok(out)
}

fn half(h: u16) -> f32 {
    let (sign, exp, mant) = ((h >> crate::kv!(191 int)) as u32, ((h >> crate::kv!(77 int)) & crate::kv!(454 int)) as i32, (h & crate::kv!(837 int)) as f32);
    let v = match exp {
        0 => mant * 2f32.powi(-crate::kv!(96 int)),
        31 => if mant == 0.0 { f32::INFINITY } else { f32::NAN },
        e => (1.0 + mant / crate::kv!(756)) * 2f32.powi(e - crate::kv!(191 int)),
    };
    if sign == 1 { -v } else { v }
}

fn dec3n(v: u32) -> [f32; 3] {
    let s = |x: u32| { let x = (x & crate::kv!(837 int)) as i32; (if x & crate::kv!(174 int) != 0 { x - crate::kv!(175 int) } else { x }) as f32 / crate::kv!(838) };
    [s(v), s(v >> crate::kv!(77 int)), s(v >> crate::kv!(118 int))]
}

pub fn dictionary(r: &Resource) -> Result<Vec<(u32, usize)>, String> {
    let entries = array(r, crate::kv!(96 int))?;
    if entries.is_empty() {
        return Ok(vec![]);
    }
    let hashes = ptr(r, be32(&r.data, crate::kv!(51 int))?)?;
    entries.into_iter().enumerate().map(|(i, at)| Ok((be32(&r.data, hashes + i * crate::kv!(1 int))?, at))).collect()
}

pub fn embedded_textures(r: &Resource, drawable: usize) -> Option<usize> {
    let group = ptr(r, be32(&r.data, drawable + crate::kv!(23 int)).ok()?).ok()?;
    ptr(r, be32(&r.data, group + crate::kv!(1 int)).ok()?).ok()
}

pub fn fragment_root(r: &Resource) -> Result<usize, String> {
    ptr(r, be32(&r.data, crate::kv!(484 int))?)
}

pub fn fragment_inertia(r: &Resource) -> Result<[f32; 3], String> {
    let f = |at: usize| be32(&r.data, at).map(f32::from_bits);
    let (list, count) = (ptr(r, be32(&r.data, crate::kv!(389 int))?)?, *r.data.get(crate::kv!(839 int)).ok_or("Truncated fragment")? as usize);
    let mut total = 0.0;
    for i in 0..count {
        total += f(ptr(r, be32(&r.data, list + i * crate::kv!(1 int))?)? + crate::kv!(1 int))?;
    }
    let arch = ptr(r, be32(&r.data, crate::kv!(88 int))?)?;
    Ok([f(arch + crate::kv!(89 int))? / total, f(arch + crate::kv!(376 int))? / total, f(arch + crate::kv!(314 int))? / total])
}

pub fn fragment_damping(r: &Resource) -> Result<[f32; 6], String> {
    let mut out = [0.0; 6];
    for (i, d) in out.iter_mut().enumerate() {
        *d = f32::from_bits(be32(&r.data, crate::kv!(355 int) + i * crate::kv!(51 int))?);
    }
    Ok(out)
}

pub fn read_drawable(r: &Resource, drawable: usize) -> Result<Vec<Mesh>, String> {
    if r.kind != crate::kv!(429 int) {
        return meshes(r, drawable, drawable);
    }
    let (list, count) = (ptr(r, be32(&r.data, crate::kv!(389 int))?)?, *r.data.get(crate::kv!(625 int)).ok_or("Truncated fragment")? as usize);
    let mut out = meshes(r, drawable, drawable)?;
    for i in 0..count {
        let child = ptr(r, be32(&r.data, list + i * crate::kv!(1 int))?)?;
        match be32(&r.data, child + crate::kv!(295 int))? {
            0 => {}
            p => out.extend(meshes(r, ptr(r, p)?, drawable)?),
        }
    }
    Ok(out)
}

pub fn fragment_children(r: &Resource) -> Result<Vec<(usize, usize)>, String> {
    let (list, count) = (ptr(r, be32(&r.data, crate::kv!(389 int))?)?, *r.data.get(crate::kv!(625 int)).ok_or("Truncated fragment")? as usize);
    let mut out = vec![];
    for i in 0..count {
        let child = ptr(r, be32(&r.data, list + i * crate::kv!(1 int))?)?;
        match be32(&r.data, child + crate::kv!(83 int))? {
            0 => {}
            p => out.push((be16(&r.data, child + crate::kv!(53 int))? as usize, ptr(r, p)?)),
        }
    }
    Ok(out)
}

#[derive(Clone, Debug)]
pub struct LightAttr {
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub tangent: [f32; 3],
    pub colour: [u8; 3],
    pub corona_intensity: f32,
    pub volume_intensity: f32,
    pub volume_size: f32,
    pub falloff: f32,
    pub intensity: f32,
    pub corona_size: f32,
    pub inner_cone: f32,
    pub outer_cone: f32,
    pub flags: u32,
    pub texture: u32,
    pub flashiness: u8,
    pub kind: u8,
    pub corona_scale: f32,
    pub light_fade: f32,
    pub volume_fade: f32,
    pub bone: i32,
}

pub fn lights(r: &Resource, drawable: usize) -> Result<Vec<LightAttr>, String> {
    let at = if r.kind == crate::kv!(429 int) { crate::kv!(609 int) } else { drawable + crate::kv!(249 int) };
    let d = &r.data;
    let count = be16(d, at + crate::kv!(1 int))? as usize;
    if count == 0 {
        return Ok(vec![]);
    }
    let list = ptr(r, be32(d, at)?)?;
    let f = |a: usize| be32(d, a).map(f32::from_bits);
    let v3 = |a: usize| -> Result<[f32; 3], String> { Ok([f(a)?, f(a + crate::kv!(1 int))?, f(a + crate::kv!(23 int))?]) };
    (0..count).map(|i| {
        let s = list + i * crate::kv!(421 int);
        let b = d.get(s..s + crate::kv!(421 int)).ok_or("Truncated light")?;
        Ok(LightAttr {
            position: v3(s + crate::kv!(1 int))?,
            direction: v3(s + crate::kv!(51 int))?,
            tangent: v3(s + crate::kv!(81 int))?,
            colour: [b[crate::kv!(91 int)], b[crate::kv!(345 int)], b[crate::kv!(398 int)]],
            corona_intensity: f(s + crate::kv!(406 int))?,
            volume_intensity: f(s + crate::kv!(89 int))?,
            volume_size: f(s + crate::kv!(376 int))?,
            falloff: f(s + crate::kv!(314 int))?,
            intensity: f(s + crate::kv!(338 int))?,
            corona_size: f(s + crate::kv!(251 int))?,
            inner_cone: f(s + crate::kv!(321 int))?,
            outer_cone: f(s + crate::kv!(475 int))?,
            flags: be32(d, s + crate::kv!(358 int))?,
            texture: be32(d, s + crate::kv!(460 int))?,
            flashiness: b[crate::kv!(311 int)],
            kind: b[crate::kv!(277 int)],
            corona_scale: f(s + crate::kv!(344 int))?,
            light_fade: f(s + crate::kv!(282 int))?,
            volume_fade: f(s + crate::kv!(189 int))?,
            bone: be32(d, s + crate::kv!(430 int))? as i32,
        })
    }).collect()
}

pub fn meshes(r: &Resource, geometry: usize, shading: usize) -> Result<Vec<Mesh>, String> {
    let d = &r.data;
    let shaders = shaders(r, ptr(r, be32(d, shading + crate::kv!(23 int))?)?)?;
    let params = shader_params(r, shading).unwrap_or_default();
    geometries(r, geometry)?
        .into_iter()
        .map(|(shader, g)| {
            let decl = declaration(r, g)?;
            let vb = ptr(r, be32(d, g + crate::kv!(52 int))?)?;
            let ib = ptr(r, be32(d, g + crate::kv!(81 int))?)?;
            let (vcount, stride) = (be16(d, vb + crate::kv!(1 int))? as usize, decl.stride as usize);
            let (vdata, idata) = (ptr(r, be32(d, vb + crate::kv!(23 int))?)?, ptr(r, be32(d, ib + crate::kv!(23 int))?)?);
            let (texture, preset) = shaders.get(shader).cloned().unwrap_or_default();
            let mut mesh = Mesh { texture, shader: preset, params: params.get(shader).cloned().unwrap_or_default(), ..Default::default() };
            let map_count = be16(d, g + crate::kv!(301 int))? as usize;
            if map_count > 0 {
                let map = ptr(r, be32(d, g + crate::kv!(314 int))?)?;
                mesh.bone_map = (0..map_count).map(|i| be16(d, map + i * 2)).collect::<Result<_, _>>()?;
            }
            mesh.indices = (0..be32(d, ib + crate::kv!(1 int))? as usize).map(|i| be16(d, idata + i * 2).map(u32::from)).collect::<Result<_, _>>()?;
            let mut offset = 0;
            for bit in 0..crate::kv!(7 int) {
                if decl.usage & (1 << bit) == 0 {
                    continue;
                }
                let t = ((decl.types >> (bit * crate::kv!(1 int))) & crate::kv!(191 int)) as u32;
                let size = type_size(t).ok_or(format!("Unknown vertex element type {t} (usage bit {bit})"))?;
                for v in 0..vcount {
                    let at = vdata + v * stride + offset;
                    let f = |k: usize| be32(d, at + k * crate::kv!(1 int)).map(f32::from_bits);
                    match (bit, t) {
                        (POSITION, 6) => mesh.positions.push([f(0)?, f(1)?, f(2)?]),
                        (NORMAL, 6) => mesh.normals.push([f(0)?, f(1)?, f(2)?]),
                        (NORMAL, 0xA) => mesh.normals.push(dec3n(be32(d, at)?)),
                        (TANGENT0, 0xA) => {
                            let v = be32(d, at)?;
                            let [x, y, z] = dec3n(v);
                            let w = match v >> 30 { 1 => 1.0, 0 => 0.0, _ => -1.0 };
                            mesh.tangents.push([x, y, z, w]);
                        }
                        (BLEND_WEIGHT, 9) => mesh.weights.push(be32(d, at)?.to_be_bytes()),
                        (BLEND_INDEX, 9) => mesh.joints.push(be32(d, at)?.to_be_bytes()),
                        (COLOR, 9) => { let [a, r, g, b] = be32(d, at)?.to_be_bytes(); mesh.colors.push([r, g, b, a]) }
                        (TEXCOORD0, 1) => {
                            let h = |k: usize| half(be16(d, at + k * 2).unwrap_or(0));
                            mesh.uvs.push([h(0), h(1)]);
                        }
                        (7..=13, 1) => {
                            let h = |k: usize| half(be16(d, at + k * 2).unwrap_or(0));
                            let i = (bit - crate::kv!(22 int)) as usize;
                            if mesh.texcoords.len() <= i {
                                mesh.texcoords.resize(i + 1, Vec::new());
                            }
                            mesh.texcoords[i].push([h(0), h(1)]);
                        }
                        _ => break,
                    }
                }
                offset += size;
            }
            if offset != stride {
                return Err(format!("Declaration sizes sum to {offset}, stride is {stride}"));
            }
            Ok(mesh)
        })
        .collect()
}
