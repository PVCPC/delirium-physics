use bevy::prelude::*;

const BASE: u64 = 0x5000_0000;

pub struct Capsule {
    pub mass: f32,
    pub inertia: Vec3,
    pub radius: f32,
    pub half: f32,
    pub turn: Mat3,
    pub at: Vec3,
    pub cg: Vec3,
    pub boxed: Option<Vec3>,
}

pub struct Link {
    pub name: String,
    pub capsules: Vec<Capsule>,
}

pub struct Joint {
    pub parent: usize,
    pub child: usize,
    pub in_parent: Mat3,
    pub at_parent: Vec3,
    pub in_child: Mat3,
    pub at_child: Vec3,
    pub half: Vec3,
}

pub struct Frag {
    pub links: Vec<Link>,
    pub joints: Vec<Joint>,
    pub pairs: Vec<(usize, usize)>,
}

impl Frag {
    pub fn to_text(&self) -> String {
        let f = |v: &[f32]| v.iter().map(|x| format!(" {x:?}")).collect::<String>();
        let mut s = String::new();
        for l in &self.links {
            s += &format!("link {} {}\n", l.name, l.capsules.len());
            for c in &l.capsules {
                let b = c.boxed.map_or(vec![0.0], |b| vec![1.0, b.x, b.y, b.z]);
                s += &format!("cap{}{}{}{}{}{}{}\n", f(&[c.mass]), f(&c.inertia.to_array()), f(&[c.radius, c.half]), f(&c.turn.to_cols_array()), f(&c.at.to_array()), f(&c.cg.to_array()), f(&b));
            }
        }
        for j in &self.joints {
            s += &format!("joint {} {}{}{}{}{}{}\n", j.parent, j.child, f(&j.in_parent.to_cols_array()), f(&j.at_parent.to_array()), f(&j.in_child.to_cols_array()), f(&j.at_child.to_array()), f(&j.half.to_array()));
        }
        for (a, b) in &self.pairs {
            s += &format!("pair {a} {b}\n");
        }
        s
    }

    pub fn from_text(text: &str) -> Result<Frag, String> {
        let mut fr = Frag { links: vec![], joints: vec![], pairs: vec![] };
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            let n = |i: usize| w.get(i).and_then(|x| x.parse::<f32>().ok()).ok_or_else(|| format!("horse body: bad line {line:?}"));
            let v3 = |i: usize| Ok::<_, String>(Vec3::new(n(i)?, n(i + 1)?, n(i + 2)?));
            let m3 = |i: usize| Ok::<_, String>(Mat3::from_cols(v3(i)?, v3(i + 3)?, v3(i + 6)?));
            match w.first().copied() {
                Some("link") => fr.links.push(Link { name: w[1].into(), capsules: vec![] }),
                Some("cap") => {
                    let boxed = if n(22)? != 0.0 { Some(v3(23)?) } else { None };
                    let c = Capsule { mass: n(1)?, inertia: v3(2)?, radius: n(5)?, half: n(6)?, turn: m3(7)?, at: v3(16)?, cg: v3(19)?, boxed };
                    fr.links.last_mut().ok_or("horse body: a capsule before its link")?.capsules.push(c);
                }
                Some("joint") => fr.joints.push(Joint { parent: n(1)? as usize, child: n(2)? as usize, in_parent: m3(3)?, at_parent: v3(12)?, in_child: m3(15)?, at_child: v3(24)?, half: v3(27)? }),
                Some("pair") => fr.pairs.push((n(1)? as usize, n(2)? as usize)),
                _ => {}
            }
        }
        Ok(fr)
    }
}

pub const BUILT_IN_XML: &str = include_str!("horse_body/horse.xml");
pub const BUILT_IN_FRAG: &str = include_str!("horse_body/horse_frag.txt");
pub const FARM_FRAG: &str = include_str!("horse_body/farm_frag.txt");
pub const ANIMALS: [(&str, &str); 7] = [("Horse", FARM_FRAG), ("Cow", include_str!("horse_body/farm_cow.txt")),
    ("Zebra", include_str!("horse_body/farm_zebra.txt")), ("Llama", include_str!("horse_body/farm_llama.txt")),
    ("Pig", include_str!("horse_body/farm_pig.txt")), ("Sheep", include_str!("horse_body/farm_sheep.txt")), ("Pug", include_str!("horse_body/farm_pug.txt"))];

struct R<'a>(&'a [u8]);

impl R<'_> {
    fn bytes<const N: usize>(&self, o: usize) -> Result<[u8; N], String> {
        self.0.get(o..o + N).and_then(|b| b.try_into().ok()).ok_or_else(|| format!("the file ends early (at {o:#x})"))
    }
    fn u8(&self, o: usize) -> Result<usize, String> { Ok(self.bytes::<1>(o)?[0] as usize) }
    fn u16(&self, o: usize) -> Result<usize, String> { Ok(u16::from_le_bytes(self.bytes(o)?) as usize) }
    fn u32(&self, o: usize) -> Result<usize, String> { Ok(u32::from_le_bytes(self.bytes(o)?) as usize) }
    fn f(&self, o: usize) -> Result<f32, String> { Ok(f32::from_le_bytes(self.bytes(o)?)) }
    fn v(&self, o: usize) -> Result<Vec3, String> { Ok(Vec3::new(self.f(o)?, self.f(o + 4)?, self.f(o + 8)?)) }
    fn p(&self, o: usize) -> Result<usize, String> {
        let v = u64::from_le_bytes(self.bytes(o)?);
        v.checked_sub(BASE).map(|x| x as usize).filter(|&x| x < self.0.len()).ok_or_else(|| format!("not a fragment I can read (no pointer at {o:#x})"))
    }
    fn rows(&self, o: usize) -> Result<Mat3, String> {
        Ok(Mat3::from_cols(self.v(o)?, self.v(o + 16)?, self.v(o + 32)?))
    }
}

pub fn read(bytes: &[u8]) -> Result<Frag, String> {
    let r = R(bytes);
    let lod = r.p(r.p(0x90)? + 0x10)?;
    let (groups, kids) = (r.u8(lod + 0x140)?, r.u8(lod + 0x146)?);
    if groups == 0 || kids < groups {
        return Err("not a ragdoll fragment (no links)".into());
    }
    let (comp, masses, inertias) = (r.p(lod + 0xe0)?, r.p(lod + 0x128)?, r.p(lod + 0xe8)?);
    let (bounds, mats) = (r.p(comp + 0x70)?, r.p(comp + 0x78)?);
    let capsule = |k: usize| -> Result<Capsule, String> {
        let b = r.p(bounds + 8 * k)?;
        let boxed = match r.u8(b + 0x10)? {
            1 => false,
            3 => true,
            _ => return Err(format!("part {k} isn't a capsule or a box")),
        };
        let (far, radius) = (r.v(b + 0x20)?, r.f(b + 0x2c)?);
        let m = mats + 0x40 * k;
        Ok(Capsule { mass: r.f(masses + 4 * k)?, inertia: r.v(inertias + 16 * k)?, radius, half: far.y, turn: r.rows(m)?,
                     at: r.v(m + 48)?, cg: r.v(b + 0x50)?, boxed: boxed.then_some(far) })
    };
    let group_at = r.p(lod + 0xc8)?;
    let mut links = vec![];
    for g in 0..groups {
        let o = r.p(group_at + 8 * g)?;
        let (first, n) = (r.u16(o + 8)?, r.u8(o + 0x0a)?);
        let name = String::from_utf8_lossy(&bytes[o + 0x4f..(o + 0x90).min(bytes.len())]).split('\0').next().unwrap_or("").to_string();
        let capsules = (first..first + n.max(1)).map(capsule).collect::<Result<Vec<_>, _>>()?;
        links.push(Link { name, capsules });
    }
    let art = r.p(lod + 0x20)?;
    let jat = r.p(art + 0x78)?;
    let mut joints = vec![];
    for i in 0..groups - 1 {
        let j = r.p(jat + 8 * i)?;
        let (parent, child) = (r.u32(j + 0x18)?, r.u32(j + 0x1c)?);
        if parent >= groups || child >= groups {
            return Err(format!("joint {i} joins links that aren't there"));
        }
        joints.push(Joint { parent, child, in_parent: r.rows(j + 0x20)?.transpose(), at_parent: r.v(j + 0x50)?, in_child: r.rows(j + 0x60)?.transpose(),
                            at_child: r.v(j + 0x90)?, half: r.v(j + 0xa0)? });
    }
    let n = r.u32(lod + 0x138)?;
    let (pa, pb) = (r.p(lod + 0x100)?, r.p(lod + 0x108)?);
    let pairs = (0..n).map(|i| Ok((r.u16(pa + 2 * i)?, r.u16(pb + 2 * i)?))).collect::<Result<Vec<_>, String>>()?
        .into_iter().filter(|&(a, b)| a < groups && b < groups).collect();
    Ok(Frag { links, joints, pairs })
}

impl Link {
    pub fn frame(&self) -> (Mat3, Vec3) {
        let m: f32 = self.capsules.iter().map(|c| c.mass).sum();
        let at = self.capsules.iter().map(|c| (c.at + c.turn * c.cg) * c.mass).sum::<Vec3>() / m.max(1e-6);
        (self.capsules[0].turn, at)
    }
}
