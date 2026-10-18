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
        1 | 9 | 0xA => Some(4),
        6 => Some(12),
        _ => None,
    }
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
}
fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2).map(|b| crate::u16_of([b[0], b[1]])).ok_or("Read out of range".into())
}

pub(crate) fn ptr(r: &Resource, p: u32) -> Result<usize, String> {
    let off = (p & 0x0FFF_FFFF) as usize;
    match p >> 28 {
        5 => Ok(off),
        6 => Ok(system_size(r.flags) + off),
        _ => Err(format!("Bad resource pointer {p:08X}")),
    }
}

fn array(r: &Resource, at: usize) -> Result<Vec<usize>, String> {
    let count = be16(&r.data, at + 4)? as usize;
    if count == 0 {
        return Ok(vec![]);
    }
    let list = ptr(r, be32(&r.data, at)?)?;
    (0..count).map(|i| ptr(r, be32(&r.data, list + i * 4)?)).collect()
}

fn c_string(d: &[u8], at: usize) -> String {
    String::from_utf8_lossy(d[at..].split(|&b| b == 0).next().unwrap_or_default()).into_owned()
}

fn shaders(r: &Resource, group: usize) -> Result<Vec<(Option<String>, String)>, String> {
    let d = &r.data;
    array(r, group + 8)?
        .into_iter()
        .map(|shader| {
            let preset = ptr(r, be32(d, shader + 0x48)?).map(|at| c_string(d, at)).unwrap_or_default();
            let count = be32(d, shader + 0x1C)? as usize;
            if count == 0 {
                return Ok((None, preset));
            }
            let (params, types) = (ptr(r, be32(d, shader + 0x14)?)?, ptr(r, be32(d, shader + 0x24)?)?);
            for i in 0..count {
                let value = be32(d, params + i * 4)?;
                if d[types + i] == 0 && value != 0 {
                    let tex = ptr(r, value)?;
                    return Ok((Some(c_string(d, ptr(r, be32(d, tex + 0x14)?)?)), preset));
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
    array(r, ptr(r, be32(d, drawable + 0x08)?)? + 8)?
        .into_iter()
        .map(|shader| {
            let mut out = ShaderParams { preset: ptr(r, be32(d, shader + 0x48)?).map(|at| c_string(d, at)).unwrap_or_default(), ..Default::default() };
            let count = be32(d, shader + 0x1C)? as usize;
            if count == 0 {
                return Ok(out);
            }
            let (params, types, hashes) =
                (ptr(r, be32(d, shader + 0x14)?)?, ptr(r, be32(d, shader + 0x24)?)?, ptr(r, be32(d, shader + 0x34)?)?);
            for i in 0..count {
                let (value, hash) = (be32(d, params + i * 4)?, be32(d, hashes + i * 4)?);
                if value == 0 {
                    continue;
                }
                let at = ptr(r, value)?;
                if d[types + i] == 0 {
                    if let Ok(name) = ptr(r, be32(d, at + 0x14)?) {
                        out.textures.push((hash, c_string(d, name)));
                    }
                } else {
                    let f = |k: usize| be32(d, at + k * 4).map(f32::from_bits);
                    out.vectors.push((hash, [f(0)?, f(1)?, f(2)?, f(3)?]));
                }
            }
            Ok(out)
        })
        .collect()
}

pub fn drawables(r: &Resource) -> Result<Vec<usize>, String> {
    if be32(&r.data, 0)? & 0xFF_FFFF == 0x83_6100 { array(r, 0x18) } else { Ok(vec![0]) }
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
    let vb = ptr(r, be32(&r.data, geometry + 0x0C)?)?;
    let decl = ptr(r, be32(&r.data, vb + 0x10)?)?;
    let d = &r.data;
    Ok(Declaration { usage: be32(d, decl)?, stride: d[decl + 4], types: u64::from_be_bytes(d[decl + 8..decl + 16].try_into().unwrap()) })
}

fn geometries(r: &Resource, drawable: usize) -> Result<Vec<(usize, usize)>, String> {
    let d = &r.data;
    let lod = be32(d, drawable + 0x40)?;
    if lod == 0 {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for model in array(r, ptr(r, lod)?)? {
        let shaders = ptr(r, be32(d, model + 0x10)?)?;
        for (i, g) in array(r, model + 4)?.into_iter().enumerate() {
            out.push((be16(d, shaders + i * 2)? as usize, g));
        }
    }
    Ok(out)
}

fn half(h: u16) -> f32 {
    let (sign, exp, mant) = ((h >> 15) as u32, ((h >> 10) & 0x1F) as i32, (h & 0x3FF) as f32);
    let v = match exp {
        0 => mant * 2f32.powi(-24),
        31 => if mant == 0.0 { f32::INFINITY } else { f32::NAN },
        e => (1.0 + mant / 1024.0) * 2f32.powi(e - 15),
    };
    if sign == 1 { -v } else { v }
}

fn dec3n(v: u32) -> [f32; 3] {
    let s = |x: u32| { let x = (x & 0x3FF) as i32; (if x & 0x200 != 0 { x - 1024 } else { x }) as f32 / 511.0 };
    [s(v), s(v >> 10), s(v >> 20)]
}

pub fn dictionary(r: &Resource) -> Result<Vec<(u32, usize)>, String> {
    let entries = array(r, 0x18)?;
    if entries.is_empty() {
        return Ok(vec![]);
    }
    let hashes = ptr(r, be32(&r.data, 0x10)?)?;
    entries.into_iter().enumerate().map(|(i, at)| Ok((be32(&r.data, hashes + i * 4)?, at))).collect()
}

pub fn embedded_textures(r: &Resource, drawable: usize) -> Option<usize> {
    let group = ptr(r, be32(&r.data, drawable + 0x08).ok()?).ok()?;
    ptr(r, be32(&r.data, group + 0x04).ok()?).ok()
}

pub fn fragment_root(r: &Resource) -> Result<usize, String> {
    ptr(r, be32(&r.data, 0xB4)?)
}

pub fn fragment_inertia(r: &Resource) -> Result<[f32; 3], String> {
    let f = |at: usize| be32(&r.data, at).map(f32::from_bits);
    let (list, count) = (ptr(r, be32(&r.data, 0xD4)?)?, *r.data.get(0x1F3).ok_or("Truncated fragment")? as usize);
    let mut total = 0.0;
    for i in 0..count {
        total += f(ptr(r, be32(&r.data, list + i * 4)?)? + 4)?;
    }
    let arch = ptr(r, be32(&r.data, 0xE4)?)?;
    Ok([f(arch + 0x30)? / total, f(arch + 0x34)? / total, f(arch + 0x38)? / total])
}

pub fn fragment_damping(r: &Resource) -> Result<[f32; 6], String> {
    let mut out = [0.0; 6];
    for (i, d) in out.iter_mut().enumerate() {
        *d = f32::from_bits(be32(&r.data, 0x50 + i * 16)?);
    }
    Ok(out)
}

pub fn read_drawable(r: &Resource, drawable: usize) -> Result<Vec<Mesh>, String> {
    if r.kind != 112 {
        return meshes(r, drawable, drawable);
    }
    let (list, count) = (ptr(r, be32(&r.data, 0xD4)?)?, *r.data.get(0x1F6).ok_or("Truncated fragment")? as usize);
    let mut out = meshes(r, drawable, drawable)?;
    for i in 0..count {
        let child = ptr(r, be32(&r.data, list + i * 4)?)?;
        match be32(&r.data, child + 0x94)? {
            0 => {}
            p => out.extend(meshes(r, ptr(r, p)?, drawable)?),
        }
    }
    Ok(out)
}

pub fn fragment_children(r: &Resource) -> Result<Vec<(usize, usize)>, String> {
    let (list, count) = (ptr(r, be32(&r.data, 0xD4)?)?, *r.data.get(0x1F6).ok_or("Truncated fragment")? as usize);
    let mut out = vec![];
    for i in 0..count {
        let child = ptr(r, be32(&r.data, list + i * 4)?)?;
        match be32(&r.data, child + 0x90)? {
            0 => {}
            p => out.push((be16(&r.data, child + 0x0E)? as usize, ptr(r, p)?)),
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
    pub flashiness: u8,
    pub kind: u8,
    pub corona_scale: f32,
    pub light_fade: f32,
    pub volume_fade: f32,
    pub bone: i32,
}

pub fn lights(r: &Resource, drawable: usize) -> Result<Vec<LightAttr>, String> {
    let at = if r.kind == 112 { 0x210 } else { drawable + 0x80 };
    let d = &r.data;
    let count = be16(d, at + 4)? as usize;
    if count == 0 {
        return Ok(vec![]);
    }
    let list = ptr(r, be32(d, at)?)?;
    let f = |a: usize| be32(d, a).map(f32::from_bits);
    let v3 = |a: usize| -> Result<[f32; 3], String> { Ok([f(a)?, f(a + 4)?, f(a + 8)?]) };
    (0..count).map(|i| {
        let s = list + i * 0x6C;
        let b = d.get(s..s + 0x6C).ok_or("Truncated light")?;
        Ok(LightAttr {
            position: v3(s + 0x04)?,
            direction: v3(s + 0x10)?,
            tangent: v3(s + 0x1C)?,
            colour: [b[0x28], b[0x29], b[0x2A]],
            corona_intensity: f(s + 0x2C)?,
            volume_intensity: f(s + 0x30)?,
            volume_size: f(s + 0x34)?,
            falloff: f(s + 0x38)?,
            intensity: f(s + 0x3C)?,
            corona_size: f(s + 0x40)?,
            inner_cone: f(s + 0x44)?,
            outer_cone: f(s + 0x48)?,
            flags: be32(d, s + 0x4C)?,
            flashiness: b[0x58],
            kind: b[0x5B],
            corona_scale: f(s + 0x5C)?,
            light_fade: f(s + 0x60)?,
            volume_fade: f(s + 0x64)?,
            bone: be32(d, s + 0x68)? as i32,
        })
    }).collect()
}

pub fn meshes(r: &Resource, geometry: usize, shading: usize) -> Result<Vec<Mesh>, String> {
    let d = &r.data;
    let shaders = shaders(r, ptr(r, be32(d, shading + 0x08)?)?)?;
    let params = shader_params(r, shading).unwrap_or_default();
    geometries(r, geometry)?
        .into_iter()
        .map(|(shader, g)| {
            let decl = declaration(r, g)?;
            let vb = ptr(r, be32(d, g + 0x0C)?)?;
            let ib = ptr(r, be32(d, g + 0x1C)?)?;
            let (vcount, stride) = (be16(d, vb + 4)? as usize, decl.stride as usize);
            let (vdata, idata) = (ptr(r, be32(d, vb + 8)?)?, ptr(r, be32(d, ib + 8)?)?);
            let (texture, preset) = shaders.get(shader).cloned().unwrap_or_default();
            let mut mesh = Mesh { texture, shader: preset, params: params.get(shader).cloned().unwrap_or_default(), ..Default::default() };
            let map_count = be16(d, g + 0x3E)? as usize;
            if map_count > 0 {
                let map = ptr(r, be32(d, g + 0x38)?)?;
                mesh.bone_map = (0..map_count).map(|i| be16(d, map + i * 2)).collect::<Result<_, _>>()?;
            }
            mesh.indices = (0..be32(d, ib + 4)? as usize).map(|i| be16(d, idata + i * 2).map(u32::from)).collect::<Result<_, _>>()?;
            let mut offset = 0;
            for bit in 0..32 {
                if decl.usage & (1 << bit) == 0 {
                    continue;
                }
                let t = ((decl.types >> (bit * 4)) & 0xF) as u32;
                let size = type_size(t).ok_or(format!("Unknown vertex element type {t} (usage bit {bit})"))?;
                for v in 0..vcount {
                    let at = vdata + v * stride + offset;
                    let f = |k: usize| be32(d, at + k * 4).map(f32::from_bits);
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
                            let i = (bit - 7) as usize;
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
