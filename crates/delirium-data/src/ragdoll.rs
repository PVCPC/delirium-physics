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
    pub length: f32,
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + crate::kv!(1 int)).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
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
    let (children, groups, count) = (p(crate::kv!(393 int))?, p(crate::kv!(365 int))?, byte(d, crate::kv!(631 int))? as usize);
    let bound = p(p(crate::kv!(88 int))? + crate::kv!(52 int))?;
    if byte(d, bound + crate::kv!(1 int))? != crate::kv!(52 int) {
        return Err("Fragment bound is not a composite".into());
    }
    let (kids, mats, inertia) = (p(bound + crate::kv!(252 int))?, p(bound + crate::kv!(363 int))?, p(crate::kv!(297 int))?);
    let mut raw = vec![];
    for i in 0..count {
        let c = p(children + i * crate::kv!(1 int))?;
        let group = byte(d, c + crate::kv!(52 int))?;
        let bone = crate::u16_of([byte(d, c + crate::kv!(53 int))?, byte(d, c + crate::kv!(194 int))?]) as usize;
        let parent_group = byte(d, p(groups + group as usize * crate::kv!(1 int))? + crate::kv!(877 int))?;
        let k = p(kids + i * crate::kv!(1 int))?;
        let ext = |a: usize| -> Result<f32, String> { Ok((bef(d, k + crate::kv!(51 int) + a)? - bef(d, k + crate::kv!(7 int) + a)?) / 2.0) };
        let (radius, along) = (ext(0)?.min(ext(crate::kv!(23 int))?), ext(crate::kv!(1 int))?);
        let kind = byte(d, k + crate::kv!(1 int))?;
        let (radius, half) = if kind == 1 { (bef(d, k + crate::kv!(252 int))?, bef(d, k + crate::kv!(83 int))? * 0.5) } else { (radius, (along - radius).max(0.0)) };
        let row = |n: usize| -> Result<[f32; 3], String> { Ok([bef(d, mats + i * crate::kv!(254 int) + n * crate::kv!(51 int))?, bef(d, mats + i * crate::kv!(254 int) + n * crate::kv!(51 int) + crate::kv!(1 int))?, bef(d, mats + i * crate::kv!(254 int) + n * crate::kv!(51 int) + crate::kv!(23 int))?]) };
        raw.push((group, parent_group, Part { bone, parent: None, mass: bef(d, c + crate::kv!(1 int))?, radius, half,
                                               frame: [row(0)?, row(1)?, row(2)?, row(crate::kv!(0 int))?], kind,
                                               extent: [ext(0)?, ext(crate::kv!(1 int))?, ext(crate::kv!(23 int))?],
                                               inertia: [bef(d, inertia + i * crate::kv!(51 int))?, bef(d, inertia + i * crate::kv!(51 int) + crate::kv!(1 int))?, bef(d, inertia + i * crate::kv!(51 int) + crate::kv!(23 int))?],
                                               length: if byte(d, k + crate::kv!(1 int))? == 1 { bef(d, k + crate::kv!(83 int))? } else { 0.0 } }));
    }
    let group_of: Vec<u8> = raw.iter().map(|r| r.0).collect();
    Ok(raw.into_iter().map(|(_, pg, mut part)| {
        part.parent = group_of.iter().position(|&g| g == pg);
        part
    }).collect())
}
