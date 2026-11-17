pub trait Shaders {
    fn template(&mut self, s: u32) -> u32;
    fn template_word_0(&mut self, template: u32) -> u32;
    fn template_word_104(&mut self, template: u32) -> u32;
    fn template_half_100(&mut self, template: u32) -> u16;
    fn template_entry(&mut self, template: u32, index: u32) -> u32;
    fn use_template_entry(&mut self, with: u32, entry: u32) -> u32;

    fn first_item(&mut self, s: u32) -> u32;
    fn set_first_item(&mut self, s: u32, item: u32);
    fn next_item(&mut self, item: u32) -> u32;
    fn set_next_item(&mut self, item: u32, next: u32);

    fn set_class(&mut self, s: u32);
    fn block(&mut self, s: u32, which: u32) -> u32;
    fn clear_blocks(&mut self, s: u32);
    fn set_byte_9(&mut self, s: u32, value: u8);
    fn new_shader_byte(&mut self) -> u8;
    fn drop_item_list(&mut self, list: u32);
    fn new_item_list(&mut self, s: u32);
    fn free(&mut self, block: u32);

    fn set_base_class(&mut self, s: u32);
    fn pg_base_new(&mut self, s: u32, resource: u32);
    fn pg_base_drop(&mut self, s: u32);
    fn set_header(&mut self, s: u32, kind: u8);
    fn clear_words(&mut self, s: u32);
    fn block_64(&mut self, s: u32) -> u32;
    fn new_effect_instance(&mut self, s: u32);
    fn drop_effect_instance(&mut self, s: u32);
    fn release_vf17(&mut self, s: u32);

    fn alloc(&mut self, size: u32) -> u32;
    fn open_shader_group_file(&mut self, name: u32) -> u32;
    fn open_stream(&mut self, name: u32, file: u32) -> u32;
    fn end_stream(&mut self, stream: u32);
    fn close_file(&mut self, file: u32);
    fn factory_vf4(&mut self, factory: u32, stream: u32) -> u32;
    fn new_other_shader(&mut self, block: u32) -> u32;
    fn fixup_delta(&mut self, resource: u32, pointer: u32) -> u32;
    fn place_shader(&mut self, shader: u32, resource: u32);
    fn slot(&mut self, at: u32) -> u32;
    fn set_slot(&mut self, at: u32, value: u32);
}

pub const SHADER_FX_SIZE: u32 = 92;
pub const OTHER_SHADER_SIZE: u32 = 80;

pub fn new_base_shader<G: Shaders>(g: &mut G, s: u32, kind: u32) -> u32 {
    g.pg_base_new(s, kind);
    g.set_header(s, kind as u8);
    g.set_base_class(s);
    g.new_effect_instance(s);
    g.clear_words(s);
    s
}

pub fn drop_base_shader<G: Shaders>(g: &mut G, s: u32) {
    g.set_base_class(s);
    let block = g.block_64(s);
    g.free(block);
    g.drop_effect_instance(s);
    g.pg_base_drop(s);
}

pub fn delete_base_shader<G: Shaders>(g: &mut G, s: u32, flags: u32) -> u32 {
    drop_base_shader(g, s);
    if flags & 1 != 0 {
        g.free(s);
    }
    s
}

pub fn factory_new_shader_fx<G: Shaders>(g: &mut G) -> u32 {
    let block = g.alloc(SHADER_FX_SIZE);
    if block == 0 { 0 } else { new_shader(g, block) }
}

pub fn factory_new_other_shader<G: Shaders>(g: &mut G) -> u32 {
    let block = g.alloc(OTHER_SHADER_SIZE);
    if block == 0 { 0 } else { g.new_other_shader(block) }
}

pub fn factory_read_shader_group_file<G: Shaders>(g: &mut G, factory: u32, name: u32) -> u32 {
    let file = g.open_shader_group_file(name);
    if file == 0 {
        return 0;
    }
    let stream = g.open_stream(name, file);
    let group = g.factory_vf4(factory, stream);
    g.end_stream(stream);
    g.close_file(file);
    group
}

pub fn factory_place_shader<G: Shaders>(g: &mut G, resource: u32, slot: u32) {
    let pointer = g.slot(slot);
    if pointer == 0 {
        return;
    }
    let delta = g.fixup_delta(resource, pointer);
    let shader = delta.wrapping_add(g.slot(slot));
    g.set_slot(slot, shader);
    if shader != 0 {
        g.place_shader(shader, resource);
    }
}

pub fn template_name<G: Shaders>(g: &mut G, s: u32) -> u32 {
    let t = g.template(s);
    g.template_word_0(t)
}

pub fn template_word_104<G: Shaders>(g: &mut G, s: u32) -> u32 {
    let t = g.template(s);
    g.template_word_104(t)
}

pub fn template_half_100<G: Shaders>(g: &mut G, s: u32) -> u32 {
    let t = g.template(s);
    g.template_half_100(t) as u32
}

pub fn use_template_entry<G: Shaders>(g: &mut G, s: u32, index: u32, with: u32) -> u32 {
    let t = g.template(s);
    let entry = g.template_entry(t, index);
    g.use_template_entry(with, entry)
}

pub fn append_item<G: Shaders>(g: &mut G, s: u32, item: u32) {
    let mut last = g.first_item(s);
    if last == 0 {
        g.set_first_item(s, item);
        return;
    }
    loop {
        let next = g.next_item(last);
        if next == 0 {
            break;
        }
        last = next;
    }
    g.set_next_item(last, item);
}

pub fn new_shader<G: Shaders>(g: &mut G, s: u32) -> u32 {
    new_base_shader(g, s, 2);
    g.set_class(s);
    g.new_item_list(s);
    g.clear_blocks(s);
    let byte = g.new_shader_byte();
    g.set_byte_9(s, byte);
    s
}

pub fn drop_shader<G: Shaders>(g: &mut G, s: u32) {
    g.set_class(s);
    for which in 0..4 {
        let block = g.block(s, which);
        g.free(block);
    }
    let list = g.first_item(s);
    if list != 0 {
        g.drop_item_list(list);
        g.free(list);
    }
    drop_base_shader(g, s);
}

pub fn delete_shader<G: Shaders>(g: &mut G, s: u32, flags: u32) -> u32 {
    drop_shader(g, s);
    if flags & 1 != 0 {
        g.free(s);
    }
    s
}

pub fn release_shader<G: Shaders>(g: &mut G, s: u32) {
    g.release_vf17(s);
}

use crate::instancedata::InstanceData;
use crate::resourcemap::{Links, Location, Placed, ResourceBytes, ResourceMap};

#[derive(Clone, Debug, PartialEq)]
pub struct Shader {
    pub effect_name: Option<String>,
    pub preset: Option<String>,
    pub draw_bucket: u8,
    pub instance: InstanceData,
}

mod placed_shader {
    pub const DRAW_BUCKET: u32 = 9;
    pub const INSTANCE: u32 = 0x14;
    pub const UNUSED: [u32; 2] = [0x4C, 0x50];
    pub const EFFECT_NAME: u32 = 0x44;
    pub const PRESET: u32 = 0x48;
    pub const DESCRIPTORS: u32 = 0x58;
}

impl Shader {
    pub fn read(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_shader::*;
        links.placed(Placed::BaseShader, at);
        let instance = InstanceData::read(src, map, at.plus(INSTANCE), links)?;
        for field in UNUSED.into_iter().chain([DESCRIPTORS]) {
            if src.u32(at.plus(field))? != 0 {
                return Err("A shader with fields the game's files leave empty".into());
            }
        }
        let preset = map.follow(src, at.plus(PRESET), links)?.map(|t| c_string(src, t)).transpose()?;
        let effect_name = map.follow(src, at.plus(EFFECT_NAME), links)?.map(|t| c_string(src, t)).transpose()?;
        let draw_bucket = src.u8(at.plus(DRAW_BUCKET))?;
        links.placed(Placed::Shader, at);
        Ok(Shader { effect_name, preset, draw_bucket, instance })
    }

    pub fn preset_name(&self) -> Option<String> {
        self.preset.clone().or_else(|| self.effect_name.as_ref().map(|name| format!("{name}{PRESET_EXTENSION}")))
    }
}

pub fn read_shader_slots(src: &impl ResourceBytes, map: &ResourceMap, slots: Location, count: u32, links: &mut impl Links)
    -> Result<Vec<Option<Shader>>, String> {
    (0..count)
        .map(|i| map.follow(src, slots.plus(4 * i), links)?.map(|s| Shader::read(src, map, s, links)).transpose())
        .collect()
}

pub const PRESET_EXTENSION: &str = ".sps";

fn c_string(src: &impl ResourceBytes, at: Location) -> Result<String, String> {
    let mut bytes = vec![];
    loop {
        let b = src.u8(at.plus(bytes.len() as u32))?;
        if b == 0 {
            return Ok(String::from_utf8_lossy(&bytes).into_owned());
        }
        bytes.push(b);
    }
}

