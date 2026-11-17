use crate::resourcemap::{Links, Location, Placed, ResourceBytes, ResourceMap, ResourcePtr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NameHash(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct InstanceData {
    pub effect: NameHash,
    pub variables: Vec<Variable>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub name: NameHash,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Texture(std::rc::Rc<TextureRef>),
    Registers(Vec<[f32; 4]>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureRef {
    pub name: Option<String>,
    pub reference: bool,
}

mod placed_instance {
    pub const VALUES: u32 = 0;
    pub const COUNT: u32 = 8;
    pub const KINDS: u32 = 0x10;
    pub const EFFECT: u32 = 0x14;
    pub const NAMES: u32 = 0x20;
}

mod placed_texture {
    pub const KIND: u32 = 8;
    pub const NAME: u32 = 0x14;
    pub const REFERENCE: u8 = 2;
}

const TEXTURE_KIND: u8 = 0;

impl InstanceData {
    pub fn read(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_instance::*;
        let values = map.follow(src, at.plus(VALUES), links)?;
        let kinds = map.follow(src, at.plus(KINDS), links)?;
        let names = map.follow(src, at.plus(NAMES), links)?;
        let count = src.u32(at.plus(COUNT))?;
        let mut variables = Vec::with_capacity(count as usize);
        for i in 0..count {
            let (Some(values), Some(kinds), Some(names)) = (values, kinds, names) else {
                return Err("Instance data without its arrays".into());
            };
            let kind = src.u8(kinds.plus(i))?;
            let name = NameHash(src.u32(names.plus(4 * i))?);
            let value = match map.follow(src, values.plus(4 * i), links)? {
                None => None,
                Some(v) if kind == TEXTURE_KIND => Some(Value::Texture(std::rc::Rc::new(TextureRef::read(src, map, v, links)?))),
                Some(v) => Some(Value::Registers(
                    (0..kind as u32)
                        .map(|row| {
                            let f = |k: u32| src.u32(v.plus(16 * row + 4 * k)).map(f32::from_bits);
                            Ok([f(0)?, f(1)?, f(2)?, f(3)?])
                        })
                        .collect::<Result<_, String>>()?,
                )),
            };
            variables.push(Variable { name, value });
        }
        links.placed(Placed::InstanceData, at);
        Ok(InstanceData { effect: NameHash(src.u32(at.plus(EFFECT))?), variables })
    }
}

impl TextureRef {
    fn read(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_texture::*;
        let reference = src.u8(at.plus(KIND))? == REFERENCE;
        let name = match src.u32(at.plus(NAME))? {
            0 => None,
            p => map.resolve(ResourcePtr(p)).map(|n| c_string(src, n)).transpose()?,
        };
        if reference {
            links.placed(Placed::TextureReference, at);
        }
        Ok(TextureRef { name, reference })
    }
}

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
