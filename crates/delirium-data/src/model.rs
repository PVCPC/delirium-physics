use crate::drawable::{Drawable, DrawableParts, LodGroup};
use crate::grmmodel::Model;
use crate::instancedata::Value;
use crate::resourcemap::{Links, ResourceBytes};
use crate::shaderfx::{read_shader_slots, Shader};
use crate::shadergroup::{GroupParts, ShaderGroup};
use crate::texture::{read_dictionary_at, Texture};
use crate::resource::{system_size, Resource};
use crate::resourcemap::{Location, ResourceMap};
use crate::vertexbuffer::VertexBufferEditor;
use crate::vertexformat::{Channel, ChannelSet, ElementType, VertexFormat};

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

fn decoded(t: ElementType) -> bool {
    matches!(t, ElementType::Half2 | ElementType::Float3 | ElementType::Colour | ElementType::Dec3n)
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + crate::kv!(1 int)).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
}
fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2).map(|b| crate::u16_of([b[0], b[1]])).ok_or("Read out of range".into())
}

pub(crate) fn ptr(r: &Resource, p: u32) -> Result<usize, String> {
    let off = (p & crate::kv!(87 int)) as usize;
    match p >> crate::kv!(86 int) {
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

struct FileParts<'r> {
    r: &'r Resource,
    textures: bool,
    shaders: bool,
}

impl GroupParts for FileParts<'_> {
    type Textures = Vec<Texture>;
    type Shader = Shader;
    fn textures(&mut self, src: &impl ResourceBytes, map: &ResourceMap, field: Location, links: &mut impl Links)
        -> Result<Option<Vec<Texture>>, String> {
        if !self.textures {
            return Ok(None);
        }
        map.follow(src, field, links)?.map(|at| read_dictionary_at(self.r, at.0 as usize)).transpose()
    }
    fn shader(&mut self, src: &impl ResourceBytes, map: &ResourceMap, slot: Location, links: &mut impl Links)
        -> Result<Option<Shader>, String> {
        if !self.shaders {
            return Ok(None);
        }
        Ok(read_shader_slots(src, map, slot, 1, links)?.pop().flatten())
    }
}

impl DrawableParts for FileParts<'_> {
    type Skeleton = ();
    fn skeleton(&mut self, src: &impl ResourceBytes, map: &ResourceMap, field: Location, links: &mut impl Links)
        -> Result<Option<()>, String> {
        Ok(map.follow(src, field, links)?.map(|_| ()))
    }
}

fn drawable(r: &Resource, at: usize, textures: bool, shaders: bool) -> Result<Drawable<Vec<Texture>, Shader, ()>, String> {
    let map = ResourceMap::of(r);
    Drawable::read(r, &map, Location(at as u32), &mut FileParts { r, textures, shaders }, &mut ())
}

fn group_shaders(group: Option<ShaderGroup<Vec<Texture>, Shader>>) -> Result<Vec<Shader>, String> {
    let group = group.ok_or("A drawable without a shader group")?;
    group.shaders.into_iter().map(|s| s.ok_or_else(|| "An empty shader slot".to_string())).collect()
}

fn preset_of(shader: &Shader) -> String {
    shader.preset_name().unwrap_or_default()
}

fn shaders(shaders: &[Shader]) -> Result<Vec<(Option<String>, String)>, String> {
    shaders
        .iter()
        .map(|shader| {
            let preset = preset_of(shader);
            let texture = shader.instance.variables.iter().find_map(|v| match &v.value {
                Some(Value::Texture(t)) => Some(t.name.clone().ok_or("A texture variable without a name")),
                _ => None,
            });
            Ok((texture.transpose()?, preset))
        })
        .collect()
}

#[derive(Default, Clone, Debug)]
pub struct ShaderParams {
    pub preset: String,
    pub textures: Vec<(u32, String)>,
    pub vectors: Vec<(u32, [f32; 4])>,
}

pub fn shader_params(r: &Resource, drawable_at: usize) -> Result<Vec<ShaderParams>, String> {
    Ok(params_of(&group_shaders(drawable(r, drawable_at, false, true)?.shader_group)?))
}

fn params_of(shaders: &[Shader]) -> Vec<ShaderParams> {
    shaders
        .iter()
        .map(|shader| {
            let mut out = ShaderParams { preset: preset_of(shader), ..Default::default() };
            for v in &shader.instance.variables {
                match &v.value {
                    Some(Value::Texture(t)) => out.textures.extend(t.name.clone().map(|name| (v.name.0, name))),
                    Some(Value::Registers(rows)) => out.vectors.extend(rows.first().map(|&row| (v.name.0, row))),
                    None => {}
                }
            }
            out
        })
        .collect()
}

pub fn drawables(r: &Resource) -> Result<Vec<usize>, String> {
    if be32(&r.data, 0)? & crate::kv!(755 int) == crate::kv!(938 int) { array(r, crate::kv!(108 int)) } else { Ok(vec![0]) }
}

pub fn declarations(r: &Resource) -> Result<Vec<VertexFormat>, String> {
    Ok(declaration_records(r)?.iter().map(format_from_record).collect())
}

pub fn declaration_records(r: &Resource) -> Result<Vec<[u8; 16]>, String> {
    let mut out = vec![];
    for at in drawables(r)? {
        for (_, g) in geometries(r, at)? {
            out.push(declaration_record(r, g)?);
        }
    }
    Ok(out)
}

fn declaration_record(r: &Resource, geometry: usize) -> Result<[u8; 16], String> {
    let vb = ptr(r, be32(&r.data, geometry + crate::kv!(55 int))?)?;
    let decl = ptr(r, be32(&r.data, vb + crate::kv!(54 int))?)?;
    let b = r.data.get(decl..decl + crate::kv!(54 int)).ok_or("Read out of range")?;
    Ok(b.try_into().unwrap())
}

pub fn format_from_record(b: &[u8; 16]) -> VertexFormat {
    let types = u64::from_be_bytes(b[crate::kv!(23 int)..crate::kv!(54 int)].try_into().unwrap());
    VertexFormat {
        channels: ChannelSet(u16::from_be_bytes([b[2], b[crate::kv!(0 int)]])),
        vertex_size: b[crate::kv!(1 int)],
        packed_positions: b[crate::kv!(2 int)] & 1 != 0,
        packed: b[crate::kv!(6 int)] != 0,
        types: core::array::from_fn(|n| ElementType::from_code((types >> (crate::kv!(1 int) * n)) as u8)),
    }
}

fn geometries(r: &Resource, drawable: usize) -> Result<Vec<(usize, usize)>, String> {
    let d = &r.data;
    let lod = be32(d, drawable + crate::kv!(189 int))?;
    if lod == 0 {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for model in array(r, ptr(r, lod)?)? {
        let shaders = ptr(r, be32(d, model + crate::kv!(54 int))?)?;
        for (i, g) in array(r, model + crate::kv!(1 int))?.into_iter().enumerate() {
            out.push((be16(d, shaders + i * 2)? as usize, g));
        }
    }
    Ok(out)
}

fn half(h: u16) -> f32 {
    let (sign, exp, mant) = ((h >> crate::kv!(190 int)) as u32, ((h >> crate::kv!(82 int)) & crate::kv!(463 int)) as i32, (h & crate::kv!(939 int)) as f32);
    let v = match exp {
        0 => mant * 2f32.powi(-crate::kv!(108 int)),
        31 => if mant == 0.0 { f32::INFINITY } else { f32::NAN },
        e => (1.0 + mant / crate::kv!(795)) * 2f32.powi(e - crate::kv!(190 int)),
    };
    if sign == 1 { -v } else { v }
}

fn dec3n(v: u32) -> [f32; 3] {
    let s = |x: u32| { let x = (x & crate::kv!(939 int)) as i32; (if x & crate::kv!(183 int) != 0 { x - crate::kv!(184 int) } else { x }) as f32 / crate::kv!(940) };
    [s(v), s(v >> crate::kv!(82 int)), s(v >> crate::kv!(128 int))]
}

pub fn dictionary(r: &Resource) -> Result<Vec<(u32, usize)>, String> {
    let entries = array(r, crate::kv!(108 int))?;
    if entries.is_empty() {
        return Ok(vec![]);
    }
    let hashes = ptr(r, be32(&r.data, crate::kv!(54 int))?)?;
    entries.into_iter().enumerate().map(|(i, at)| Ok((be32(&r.data, hashes + i * crate::kv!(1 int))?, at))).collect()
}

pub fn embedded_texture_dictionary(r: &Resource, at: usize) -> Vec<Texture> {
    drawable(r, at, true, false).ok().and_then(|d| d.shader_group).and_then(|g| g.textures).unwrap_or_default()
}

pub fn fragment_root(r: &Resource) -> Result<usize, String> {
    ptr(r, be32(&r.data, crate::kv!(98 int))?)
}

pub fn fragment_inertia(r: &Resource) -> Result<[f32; 3], String> {
    let f = |at: usize| be32(&r.data, at).map(f32::from_bits);
    let (list, count) = (ptr(r, be32(&r.data, crate::kv!(399 int))?)?, *r.data.get(crate::kv!(941 int)).ok_or("Truncated fragment")? as usize);
    let mut total = 0.0;
    for i in 0..count {
        total += f(ptr(r, be32(&r.data, list + i * crate::kv!(1 int))?)? + crate::kv!(1 int))?;
    }
    let arch = ptr(r, be32(&r.data, crate::kv!(93 int))?)?;
    Ok([f(arch + crate::kv!(94 int))? / total, f(arch + crate::kv!(386 int))? / total, f(arch + crate::kv!(324 int))? / total])
}

pub fn fragment_damping(r: &Resource) -> Result<[f32; 6], String> {
    let mut out = [0.0; 6];
    for (i, d) in out.iter_mut().enumerate() {
        *d = f32::from_bits(be32(&r.data, crate::kv!(365 int) + i * crate::kv!(54 int))?);
    }
    Ok(out)
}

pub fn read_drawable(r: &Resource, drawable: usize) -> Result<Vec<Mesh>, String> {
    if r.kind != crate::kv!(438 int) {
        return meshes(r, drawable, drawable);
    }
    let (list, count) = (ptr(r, be32(&r.data, crate::kv!(399 int))?)?, *r.data.get(crate::kv!(660 int)).ok_or("Truncated fragment")? as usize);
    let mut out = meshes(r, drawable, drawable)?;
    for i in 0..count {
        let child = ptr(r, be32(&r.data, list + i * crate::kv!(1 int))?)?;
        match be32(&r.data, child + crate::kv!(305 int))? {
            0 => {}
            p => out.extend(meshes(r, ptr(r, p)?, drawable)?),
        }
    }
    Ok(out)
}

pub fn fragment_children(r: &Resource) -> Result<Vec<(usize, usize)>, String> {
    let (list, count) = (ptr(r, be32(&r.data, crate::kv!(399 int))?)?, *r.data.get(crate::kv!(660 int)).ok_or("Truncated fragment")? as usize);
    let mut out = vec![];
    for i in 0..count {
        let child = ptr(r, be32(&r.data, list + i * crate::kv!(1 int))?)?;
        match be32(&r.data, child + crate::kv!(88 int))? {
            0 => {}
            p => out.push((be16(&r.data, child + crate::kv!(56 int))? as usize, ptr(r, p)?)),
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
    let at = if r.kind == crate::kv!(438 int) { crate::kv!(644 int) } else { drawable + crate::kv!(258 int) };
    let d = &r.data;
    let count = be16(d, at + crate::kv!(1 int))? as usize;
    if count == 0 {
        return Ok(vec![]);
    }
    let list = ptr(r, be32(d, at)?)?;
    let f = |a: usize| be32(d, a).map(f32::from_bits);
    let v3 = |a: usize| -> Result<[f32; 3], String> { Ok([f(a)?, f(a + crate::kv!(1 int))?, f(a + crate::kv!(23 int))?]) };
    (0..count).map(|i| {
        let s = list + i * crate::kv!(431 int);
        let b = d.get(s..s + crate::kv!(431 int)).ok_or("Truncated light")?;
        Ok(LightAttr {
            position: v3(s + crate::kv!(1 int))?,
            direction: v3(s + crate::kv!(54 int))?,
            tangent: v3(s + crate::kv!(86 int))?,
            colour: [b[crate::kv!(96 int)], b[crate::kv!(355 int)], b[crate::kv!(408 int)]],
            corona_intensity: f(s + crate::kv!(416 int))?,
            volume_intensity: f(s + crate::kv!(94 int))?,
            volume_size: f(s + crate::kv!(386 int))?,
            falloff: f(s + crate::kv!(324 int))?,
            intensity: f(s + crate::kv!(348 int))?,
            corona_size: f(s + crate::kv!(189 int))?,
            inner_cone: f(s + crate::kv!(331 int))?,
            outer_cone: f(s + crate::kv!(483 int))?,
            flags: be32(d, s + crate::kv!(368 int))?,
            texture: be32(d, s + crate::kv!(469 int))?,
            flashiness: b[crate::kv!(321 int)],
            kind: b[crate::kv!(287 int)],
            corona_scale: f(s + crate::kv!(354 int))?,
            light_fade: f(s + crate::kv!(292 int))?,
            volume_fade: f(s + crate::kv!(204 int))?,
            bone: be32(d, s + crate::kv!(439 int))? as i32,
        })
    }).collect()
}

fn lod_models(lod_group: LodGroup) -> Result<Vec<Model>, String> {
    let [high, ..] = lod_group.lods;
    high.unwrap_or_default().into_iter().map(|m| m.ok_or_else(|| "An empty model slot".to_string())).collect()
}

pub fn meshes(r: &Resource, geometry: usize, shading: usize) -> Result<Vec<Mesh>, String> {
    let Drawable { shader_group, lod_group, .. } = drawable(r, geometry, false, geometry == shading)?;
    let shading_group = if geometry == shading { shader_group } else { drawable(r, shading, false, true)?.shader_group };
    let shader_list = group_shaders(shading_group)?;
    let shaders = shaders(&shader_list)?;
    let params = params_of(&shader_list);
    let geometries = lod_models(lod_group)?.into_iter().flat_map(|model| {
        let shader_indices = model.shader_indices;
        model.geometries.into_iter().enumerate().map(move |(i, g)| (shader_indices.get(i).copied(), g))
    });
    geometries
        .map(|(shader, g)| {
            let g = g.ok_or("A model with an empty geometry slot")?;
            let shader = shader.ok_or("A geometry without a shader index")? as usize;
            let [Some(mut vertex_buffer), ..] = g.vertex_buffers else { return Err("A geometry without a vertex buffer".into()) };
            let [Some(index_buffer), ..] = g.index_buffers else { return Err("A geometry without an index buffer".into()) };
            let decl = vertex_buffer.format;
            let (vcount, stride) = (vertex_buffer.count as usize, vertex_buffer.stride);
            let (texture, preset) = shaders.get(shader).cloned().unwrap_or_default();
            let mut mesh = Mesh { texture, shader: preset, params: params.get(shader).cloned().unwrap_or_default(), ..Default::default() };
            mesh.bone_map = g.bone_map;
            mesh.indices = index_buffer.indices.iter().map(|&i| u32::from(i)).collect();
            let mut sized = decl;
            sized.update_vertex_size();
            if sized.vertex_size != decl.vertex_size || stride != decl.vertex_size as u32 {
                return Err(format!("Vertex format sizes sum to {}, stride is {stride}", sized.vertex_size));
            }
            let editor = VertexBufferEditor::new(&mut vertex_buffer, true, true);
            let vb = &mut *editor.buffer;
            for n in 0..crate::kv!(54 u8) {
                let channel = Channel::from_number(n);
                if !decl.channels.has(channel) {
                    continue;
                }
                let t = decl.types[n as usize];
                if !decoded(t) {
                    return Err(format!("Unknown vertex element type {} (channel {n})", t.code()));
                }
                for v in 0..vcount as u32 {
                    let bytes = vb.element_bytes(v, channel, t.size() as usize).to_vec();
                    let f = |k: usize| f32::from_bits(u32::from_be_bytes(bytes[k * crate::kv!(1 int)..k * crate::kv!(1 int) + crate::kv!(1 int)].try_into().unwrap()));
                    let h = |k: usize| half(u16::from_be_bytes([bytes[k * 2], bytes[k * 2 + 1]]));
                    match (channel, t) {
                        (Channel::Position, ElementType::Float3) => mesh.positions.push([f(0), f(1), f(2)]),
                        (Channel::Normal, ElementType::Float3) => mesh.normals.push([f(0), f(1), f(2)]),
                        (Channel::Normal, ElementType::Dec3n) => mesh.normals.push(dec3n(vb.element_word(v, channel))),
                        (Channel::Tangent(0), ElementType::Dec3n) => {
                            let v = vb.element_word(v, channel);
                            let [x, y, z] = dec3n(v);
                            let w = match v >> 30 { 1 => 1.0, 0 => 0.0, _ => -1.0 };
                            mesh.tangents.push([x, y, z, w]);
                        }
                        (Channel::BlendWeight, ElementType::Colour) => mesh.weights.push(vb.element_word(v, channel).to_be_bytes()),
                        (Channel::BlendIndex, ElementType::Colour) => mesh.joints.push(vb.element_word(v, channel).to_be_bytes()),
                        (Channel::Colour, ElementType::Colour) => {
                            let [a, r, g, b] = vb.element_word(v, channel).to_be_bytes();
                            mesh.colors.push([r, g, b, a])
                        }
                        (Channel::TexCoord(0), ElementType::Half2) => mesh.uvs.push([h(0), h(1)]),
                        (Channel::TexCoord(set), ElementType::Half2) => {
                            let i = (set - 1) as usize;
                            if mesh.texcoords.len() <= i {
                                mesh.texcoords.resize(i + 1, Vec::new());
                            }
                            mesh.texcoords[i].push([h(0), h(1)]);
                        }
                        _ => break,
                    }
                }
            }
            Ok(mesh)
        })
        .collect()
}
