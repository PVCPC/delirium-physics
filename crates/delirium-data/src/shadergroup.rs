use crate::resourcemap::{Links, Location, Placed, ResourceBytes, ResourceMap};
use crate::vertexformat::ChannelSet;

#[derive(Clone, Debug)]
pub struct ShaderGroup<T, S> {
    pub textures: Option<T>,
    pub shaders: Vec<Option<S>>,
    pub vertex_channels: Vec<ChannelSet>,
    pub other_vertex_channels: Vec<ChannelSet>,
}

pub trait GroupParts {
    type Textures;
    type Shader;
    fn textures(&mut self, src: &impl ResourceBytes, map: &ResourceMap, field: Location, links: &mut impl Links)
        -> Result<Option<Self::Textures>, String>;
    fn shader(&mut self, src: &impl ResourceBytes, map: &ResourceMap, slot: Location, links: &mut impl Links)
        -> Result<Option<Self::Shader>, String>;
}

mod placed_group {
    pub const TEXTURES: u32 = 4;
    pub const SHADERS: u32 = 8;
    pub const SHADER_COUNT: u32 = 0xC;
    pub const EMPTY: [u32; 12] = [0x10, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C];
    pub const VERTEX_FORMATS: u32 = 0x40;
    pub const OTHER_VERTEX_FORMATS: u32 = 0x48;
    pub const FORMAT_COUNT: u32 = 4;
    pub const FORMAT_SIZE: u32 = 16;
}

impl<T, S> ShaderGroup<T, S> {
    pub fn read<P: GroupParts<Textures = T, Shader = S>>(src: &impl ResourceBytes, map: &ResourceMap, at: Location,
                                                           parts: &mut P, links: &mut impl Links) -> Result<Self, String> {
        use placed_group::*;
        for field in EMPTY {
            if src.u32(at.plus(field))? != 0 {
                return Err("A shader group with arrays the game's files leave empty".into());
            }
        }
        let textures = parts.textures(src, map, at.plus(TEXTURES), links)?;
        let count = src.u16(at.plus(SHADER_COUNT))? as u32;
        let mut shaders = Vec::with_capacity(count as usize);
        if let Some(list) = map.follow(src, at.plus(SHADERS), links)? {
            for i in 0..count {
                shaders.push(parts.shader(src, map, list.plus(4 * i), links)?);
            }
        }
        let formats = |field: u32, links: &mut _| -> Result<Vec<ChannelSet>, String> {
            let n = src.u16(at.plus(field + FORMAT_COUNT))? as u32;
            match map.follow(src, at.plus(field), links)? {
                Some(list) => (0..n).map(|i| Ok(ChannelSet(src.u32(list.plus(FORMAT_SIZE * i))? as u16))).collect(),
                None => Ok(vec![]),
            }
        };
        let vertex_channels = formats(VERTEX_FORMATS, links)?;
        let other_vertex_channels = formats(OTHER_VERTEX_FORMATS, links)?;
        links.placed(Placed::ShaderGroup, at);
        Ok(ShaderGroup { textures, shaders, vertex_channels, other_vertex_channels })
    }
}
