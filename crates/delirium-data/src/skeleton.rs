use crate::resource::Resource;

#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub id: u16,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub flags: u32,
    pub rot_min: [f32; 3],
    pub rot_max: [f32; 3],
    pub block: [f32; 4],
}

const BONE_SIZE: usize = 0xE0;

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
}
fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2).map(|b| crate::u16_of([b[0], b[1]])).ok_or("Read out of range".into())
}
fn floats<const N: usize>(d: &[u8], at: usize) -> Result<[f32; N], String> {
    let mut out = [0.0; N];
    for (k, v) in out.iter_mut().enumerate() {
        *v = f32::from_bits(be32(d, at + k * 4)?);
    }
    Ok(out)
}
fn sys(p: u32) -> Result<usize, String> {
    if p >> 28 == 5 { Ok((p & 0x0FFF_FFFF) as usize) } else { Err(format!("Bad skeleton pointer {p:08X}")) }
}

pub fn read(r: &Resource, drawable: usize) -> Result<Vec<Bone>, String> {
    let d = &r.data;
    let skeleton = sys(be32(d, drawable + 0x0C)?)?;
    let (bones, count) = (sys(be32(d, skeleton)?)?, be16(d, skeleton + 0x14)? as usize);
    (0..count)
        .map(|i| {
            let b = bones + i * BONE_SIZE;
            let parent = match be32(d, b + 0x10)? {
                0 => None,
                p => match sys(p)?.checked_sub(bones).map(|o| o / BONE_SIZE) {
                    Some(parent) if parent < i => Some(parent),
                    _ => return Err(format!("Bone {i} has a bad parent")),
                },
            };
            let name = sys(be32(d, b)?).map(|at| d[at..].split(|&c| c == 0).next().unwrap_or_default()).unwrap_or_default();
            Ok(Bone {
                name: String::from_utf8_lossy(name).into_owned(),
                parent,
                id: be16(d, b + 0x16)?,
                translation: floats(d, b + 0x20)?,
                rotation: floats(d, b + 0x40)?,
                flags: be32(d, b + 0x04)?,
                rot_min: floats(d, b + 0xB0)?,
                rot_max: floats(d, b + 0xC0)?,
                block: floats(d, b + 0x30)?,
            })
        })
        .collect()
}
