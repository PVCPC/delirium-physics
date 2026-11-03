//! Red Dead Redemption 2's ragdoll for a creature, read from the player's own export of its fragment (OpenIV: the
//! fragment's "Save content", which writes the decrypted resource as `<name>.yft.sys`; for the horse,
//! packs_0.rpf/packs/base/models/metaped_fragments.rpf/meta_base_horse_01.yft). Only the physics is read, from the
//! detailed physics LOD: each link's capsules (size, place, mass, inertia, centre of mass), the joints between the
//! links (their frames in both links and their limits) and which links collide with each other. Nothing of the
//! file is kept or written anywhere.
//!
//! The layout (RAGE resource version 186, as RDR2 ships it), found by reading the file: the root's +0x90 points to
//! the LOD group, whose +0x10 is the detailed LOD. That LOD: +0x20 the articulated body (its +0x78: the joints), +0xc8
//! the groups (links), +0xe0 the composite bound, +0xe8 each child's inertia, +0x100/+0x108 the self-collision pairs
//! (u16 link indices, the count at +0x138), +0x128 each child's mass, +0x140 the group count, +0x146 the child count.
//! A group: +0x06 its parent group (-1: none; the joints say the same), +0x08 its first child, +0x0a how many, +0x4f its name. The composite:
//! +0x70 the child bounds, +0x78 their matrices (3 rows of axes, then the place; 0x40 each). A capsule bound: +0x10
//! its type (1), +0x20 its box's far corner (x, y, z, then the radius), +0x50 its centre of mass. A joint: +0x18 its
//! parent and child link, +0x20 its axes in the parent link (rows), +0x50 its place there, +0x60 and +0x90 the same in
//! the child link, +0xa0 its half-ranges (lean 1, lean 2, twist), already less RAGE's margin; its axes are the columns there, where a
//! bound's matrix has them as rows. A link's frame is its
//! first capsule's turn, at the centre of mass of all its capsules.
use bevy::prelude::*;

const BASE: u64 = 0x5000_0000;

pub struct Capsule {
    pub mass: f32,
    /// about its centre of mass, along its own axes
    pub inertia: Vec3,
    pub radius: f32,
    /// half its length, end cap to end cap, along its own y
    pub half: f32,
    /// its axes (columns) and centre, in the model's frame (x left, y back, z up)
    pub turn: Mat3,
    pub at: Vec3,
    /// its centre of mass, in its own frame
    pub cg: Vec3,
}

pub struct Link {
    pub name: String,
    pub capsules: Vec<Capsule>,
}

pub struct Joint {
    pub parent: usize,
    pub child: usize,
    /// its axes (columns: lean 1, lean 2, twist) in each link's frame, and its place there
    pub in_parent: Mat3,
    pub at_parent: Vec3,
    pub in_child: Mat3,
    pub at_child: Vec3,
    /// half-ranges, rad
    pub half: Vec3,
}

pub struct Frag {
    pub links: Vec<Link>,
    pub joints: Vec<Joint>,
    /// links that collide with each other
    pub pairs: Vec<(usize, usize)>,
}

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
    /// A pointer into this resource, as an offset.
    fn p(&self, o: usize) -> Result<usize, String> {
        let v = u64::from_le_bytes(self.bytes(o)?);
        v.checked_sub(BASE).map(|x| x as usize).filter(|&x| x < self.0.len()).ok_or_else(|| format!("not a fragment I can read (no pointer at {o:#x})"))
    }
    /// Three rows on disk as the columns of a matrix.
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
        if r.u8(b + 0x10)? != 1 {
            return Err(format!("part {k} isn't a capsule"));
        }
        let (far, radius) = (r.v(b + 0x20)?, r.f(b + 0x2c)?);
        let m = mats + 0x40 * k;
        Ok(Capsule { mass: r.f(masses + 4 * k)?, inertia: r.v(inertias + 16 * k)?, radius, half: far.y, turn: r.rows(m)?,
                     at: r.v(m + 48)?, cg: r.v(b + 0x50)? })
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
        // on disk its axes are the columns (a bound's are the rows)
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
    /// Its frame in the model: its first capsule's turn, at the centre of mass of all its capsules.
    pub fn frame(&self) -> (Mat3, Vec3) {
        let m: f32 = self.capsules.iter().map(|c| c.mass).sum();
        let at = self.capsules.iter().map(|c| (c.at + c.turn * c.cg) * c.mass).sum::<Vec3>() / m.max(1e-6);
        (self.capsules[0].turn, at)
    }
}
