pub const VT_MODEL: u32 = 0x8209_39EC;
pub const MODEL_BYTES: u32 = 0x1C;
pub const MESH_BYTES: u32 = 0x2A0;
pub const MESH_EXTENSION: u32 = 0x8207_9030;
pub const OPTIMISE_MESHES: u32 = 0x831C_2F5C;
pub const OPTIMISE_STEP: u32 = 0x828C_4088;
pub const ACCEPT_HOOK: u32 = 0x831C_2F78;
pub const RELEASE_HOOK: u32 = 0x831C_2F80;

pub trait ModelLoad {
    fn word(&mut self, at: u32) -> u32;
    fn set_word(&mut self, at: u32, v: u32);
    fn set_half(&mut self, at: u32, v: u16);
    fn set_byte(&mut self, at: u32, v: u8);
    fn push_main_allocator(&mut self);
    fn pop_allocator(&mut self);
    fn alloc(&mut self, size: u32) -> u32;
    fn free(&mut self, block: u32);
    fn new_mesh(&mut self, at: u32) -> u32;
    fn read_mesh(&mut self, file: u32, mesh: u32, extension: u32) -> u32;
    fn tidy_mesh(&mut self, mesh: u32, step: u32);
    fn optimise_mesh(&mut self, mesh: u32, step: u32, arg: u32);
    fn build_model(&mut self, model: u32, mesh: u32, a: u32, b: u32, c: u32) -> u32;
    fn delete_object(&mut self, obj: u32);
    fn call_hook(&mut self, f: u32, args: &[u32]) -> u32;
    fn destroy_strings(&mut self, at: u32);
    fn delete_array(&mut self, at: u32);
}

fn ask_hook<G: ModelLoad>(g: &mut G, hook: u32, mesh: u32, file: u32) -> bool {
    let f = g.word(hook.wrapping_add(4));
    if f == 0 {
        return true;
    }
    let obj = g.word(hook);
    let r = if obj != 0 { g.call_hook(f, &[obj, mesh, file]) } else { g.call_hook(f, &[mesh, file]) };
    r & 0xFF != 0
}

pub fn delete_mesh<G: ModelLoad>(g: &mut G, mesh: u32, flags: u32) -> u32 {
    g.destroy_strings(mesh.wrapping_add(0x94));
    if g.word(mesh.wrapping_add(0x14)) != 0 {
        let block = g.word(mesh.wrapping_add(0xC));
        g.free(block);
    }
    if g.word(mesh.wrapping_add(8)) != 0 {
        let arr = g.word(mesh);
        if arr != 0 {
            g.delete_array(arr);
        }
    }
    if flags & 1 != 0 {
        g.free(mesh);
    }
    mesh
}

pub fn new_model<G: ModelLoad>(g: &mut G, mesh: u32, kind: u32, a: u32, b: u32, c: u32) -> u32 {
    let model = g.alloc(MODEL_BYTES);
    if model != 0 {
        g.set_word(model, VT_MODEL);
        g.set_word(model.wrapping_add(4), 0);
        g.set_half(model.wrapping_add(8), 0);
        g.set_half(model.wrapping_add(0xA), 0);
        g.set_word(model.wrapping_add(0xC), 0);
        g.set_word(model.wrapping_add(0x10), 0);
        for off in [0x14, 0x15, 0x17, 0x18, 0x19] {
            g.set_byte(model.wrapping_add(off), 0);
        }
        g.set_half(model.wrapping_add(0x1A), 0);
    }
    if g.build_model(model, mesh, a, b, c) & 0xFF == 0 {
        if model != 0 {
            g.delete_object(model);
        }
        return 0;
    }
    if model != 0 {
        g.set_byte(model.wrapping_add(0x17), kind as u8);
    }
    model
}

pub fn model_from_mesh<G: ModelLoad>(g: &mut G, file: u32, kind: u32, a: u32, b: u32, c: u32) -> u32 {
    g.push_main_allocator();
    let block = g.alloc(MESH_BYTES);
    let mesh = if block != 0 { g.new_mesh(block) } else { 0 };
    let read = g.read_mesh(file, mesh, MESH_EXTENSION) & 0xFF != 0;
    if read {
        for step in [0x828E_1E70, 0x828E_1EC0, 0x828E_1F10] {
            g.tidy_mesh(mesh, step);
        }
        if g.word(OPTIMISE_MESHES) != 0 {
            g.optimise_mesh(mesh, OPTIMISE_STEP, b);
        }
    }
    g.pop_allocator();
    let mut model = 0;
    if read && ask_hook(g, ACCEPT_HOOK, mesh, file) {
        model = new_model(g, mesh, kind, a, b, c);
    }
    g.push_main_allocator();
    if ask_hook(g, RELEASE_HOOK, mesh, file) && mesh != 0 {
        delete_mesh(g, mesh, 1);
    }
    g.pop_allocator();
    model
}

pub const VT_SHADER_FACTORY: u32 = 0x8209_4B84;
pub const VT_MODEL_FACTORY: u32 = 0x8209_51D4;
pub const SHADER_FACTORY: u32 = 0x831C_513C;
pub const MODEL_FACTORY: u32 = 0x831C_51CC;

fn new_factory<G: ModelLoad>(g: &mut G, vtable: u32, keep_at: u32, keep: u32) -> u32 {
    let f = g.alloc(4);
    if f != 0 {
        g.set_word(f, vtable);
    }
    if keep & 0xFF != 0 {
        g.set_word(keep_at, f);
    }
    f
}

pub fn new_shader_factory<G: ModelLoad>(g: &mut G, keep: u32) -> u32 {
    new_factory(g, VT_SHADER_FACTORY, SHADER_FACTORY, keep)
}

pub fn new_model_factory<G: ModelLoad>(g: &mut G, keep: u32) -> u32 {
    new_factory(g, VT_MODEL_FACTORY, MODEL_FACTORY, keep)
}

use crate::geometry::Geometry;
use crate::resourcemap::{Links, Location, Placed, ResourceBytes, ResourceMap};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundSphere {
    pub centre: [f32; 3],
    pub radius: f32,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub geometries: Vec<Option<Geometry>>,
    pub bounds: Vec<BoundSphere>,
    pub shader_indices: Vec<u16>,
    pub matrix_index: u8,
}

mod placed_model {
    pub const GEOMETRIES: u32 = 4;
    pub const GEOMETRY_COUNT: u32 = 8;
    pub const BOUNDS: u32 = 0xC;
    pub const SHADER_INDICES: u32 = 0x10;
    pub const MATRIX_INDEX: u32 = 0x17;
    pub const SHADER_INDEX_COUNT: u32 = 0x1A;
}

impl Model {
    pub fn read(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_model::*;
        let count = src.u16(at.plus(GEOMETRY_COUNT))? as u32;
        let mut geometries = Vec::with_capacity(count as usize);
        if let Some(list) = map.follow(src, at.plus(GEOMETRIES), links)? {
            for i in 0..count {
                let geometry = match map.follow(src, list.plus(4 * i), links)? {
                    Some(g) => Some(Geometry::read(src, map, g, links)?),
                    None => None,
                };
                geometries.push(geometry);
            }
        }
        let shader_count = src.u16(at.plus(SHADER_INDEX_COUNT))? as u32;
        let shader_indices = match map.follow(src, at.plus(SHADER_INDICES), links)? {
            Some(list) => (0..shader_count).map(|i| src.u16(list.plus(2 * i))).collect::<Result<_, _>>()?,
            None => vec![],
        };
        let spheres = if count > 1 { count + 1 } else { count };
        let bounds = match map.follow(src, at.plus(BOUNDS), links)? {
            Some(list) => (0..spheres)
                .map(|i| {
                    let f = |k: u32| src.u32(list.plus(16 * i + 4 * k)).map(f32::from_bits);
                    Ok(BoundSphere { centre: [f(0)?, f(1)?, f(2)?], radius: f(3)? })
                })
                .collect::<Result<_, String>>()?,
            None => vec![],
        };
        let matrix_index = src.u8(at.plus(MATRIX_INDEX))?;
        links.placed(Placed::Model, at);
        Ok(Model { geometries, bounds, shader_indices, matrix_index })
    }
}

impl Model {
    pub fn draw_buckets<S: crate::drawable::DrawBucket>(&self, shaders: &[Option<S>]) -> u32 {
        self.shader_indices
            .iter()
            .filter_map(|&i| shaders.get(i as usize)?.as_ref())
            .fold(0, |mask, s| mask | 1u32.checked_shl(s.draw_bucket() as u32).unwrap_or(0))
    }
}
