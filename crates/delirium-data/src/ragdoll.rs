use crate::{model::ptr, resource::Resource};

#[derive(Clone, Debug)]
pub struct Part {
    pub bone: usize,
    pub parent: Option<usize>,
    pub mass: f32,
    pub radius: f32,
    pub half: f32,
    pub frame: [[f32; 3]; 4],
    pub kind: u8,
    pub extent: [f32; 3],
    pub inertia: [f32; 3],
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
}
fn bef(d: &[u8], at: usize) -> Result<f32, String> {
    be32(d, at).map(f32::from_bits)
}
fn byte(d: &[u8], at: usize) -> Result<u8, String> {
    d.get(at).copied().ok_or("Read out of range".into())
}

pub fn read(r: &Resource) -> Result<Vec<Part>, String> {
    let d = &r.data;
    let p = |at: usize| be32(d, at).and_then(|v| ptr(r, v));
    let (children, groups, count) = (p(0xD4)?, p(0xD0)?, byte(d, 0x1F6)? as usize);
    let bound = p(p(0xE4)? + 0x0C)?;
    if byte(d, bound + 4)? != 12 {
        return Err("Fragment bound is not a composite".into());
    }
    let (kids, mats, inertia) = (p(bound + 0x80)?, p(bound + 0x84)?, p(0xF0)?);
    let mut raw = vec![];
    for i in 0..count {
        let c = p(children + i * 4)?;
        let group = byte(d, c + 0x0C)?;
        let bone = crate::u16_of([byte(d, c + 0x0E)?, byte(d, c + 0x0F)?]) as usize;
        let parent_group = byte(d, p(groups + group as usize * 4)? + 0x10D)?;
        let k = p(kids + i * 4)?;
        let ext = |a: usize| -> Result<f32, String> { Ok((bef(d, k + 0x10 + a)? - bef(d, k + 0x20 + a)?) / 2.0) };
        let (radius, along) = (ext(0)?.min(ext(8)?), ext(4)?);
        let row = |n: usize| -> Result<[f32; 3], String> { Ok([bef(d, mats + i * 64 + n * 16)?, bef(d, mats + i * 64 + n * 16 + 4)?, bef(d, mats + i * 64 + n * 16 + 8)?]) };
        raw.push((group, parent_group, Part { bone, parent: None, mass: bef(d, c + 4)?, radius, half: (along - radius).max(0.0),
                                               frame: [row(0)?, row(1)?, row(2)?, row(3)?], kind: byte(d, k + 4)?,
                                               extent: [ext(0)?, ext(4)?, ext(8)?],
                                               inertia: [bef(d, inertia + i * 16)?, bef(d, inertia + i * 16 + 4)?, bef(d, inertia + i * 16 + 8)?] }));
    }
    let group_of: Vec<u8> = raw.iter().map(|r| r.0).collect();
    Ok(raw.into_iter().map(|(_, pg, mut part)| {
        part.parent = group_of.iter().position(|&g| g == pg);
        part
    }).collect())
}
