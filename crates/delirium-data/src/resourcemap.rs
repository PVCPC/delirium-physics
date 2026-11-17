use crate::resource::{graphics_size, system_size, Resource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourcePtr(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location(pub u32);

impl Location {
    pub fn plus(self, bytes: u32) -> Location {
        Location(self.0 + bytes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub base: u32,
    pub size: u32,
    pub start: Location,
}

impl Page {
    fn holds(&self, p: u32) -> bool {
        p >= self.base && p - self.base < self.size
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceMap {
    pub pages: Vec<Page>,
}

pub const SYSTEM_BASE: u32 = 0x5000_0000;
pub const GRAPHICS_BASE: u32 = 0x6000_0000;

impl ResourceMap {
    pub fn of(r: &Resource) -> ResourceMap {
        let system = system_size(r.flags) as u32;
        ResourceMap {
            pages: vec![
                Page { base: SYSTEM_BASE, size: system, start: Location(0) },
                Page { base: GRAPHICS_BASE, size: graphics_size(r.flags) as u32, start: Location(system) },
            ],
        }
    }

    pub fn resolve(&self, p: ResourcePtr) -> Option<Location> {
        let page = self.pages.iter().find(|page| page.holds(p.0))?;
        Some(Location(page.start.0 + (p.0 - page.base)))
    }

    pub fn follow(&self, src: &impl ResourceBytes, field: Location, links: &mut impl Links) -> Result<Option<Location>, String> {
        let p = src.u32(field)?;
        if p == 0 {
            return Ok(None);
        }
        let target = self.resolve(ResourcePtr(p)).ok_or_else(|| format!("Bad resource pointer {p:08X}"))?;
        links.link(field, target);
        Ok(Some(target))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placed {
    VertexBuffer,
    VertexBufferD3d,
    IndexBuffer,
    IndexBufferD3d,
    Geometry,
    Model,
    InstanceData,
    TextureReference,
    BaseShader,
    Shader,
    ShaderGroup,
    TextureDictionary,
    ShaderSlot,
    SkeletonData,
    DrawableBase,
    Drawable,
}

pub trait Links {
    fn link(&mut self, field: Location, target: Location);
    fn gpu_link(&mut self, field: Location, target: Location);
    fn placed(&mut self, what: Placed, at: Location);
}

impl Links for () {
    fn link(&mut self, _: Location, _: Location) {}
    fn gpu_link(&mut self, _: Location, _: Location) {}
    fn placed(&mut self, _: Placed, _: Location) {}
}

pub trait ResourceBytes {
    fn read(&self, at: Location, out: &mut [u8]) -> Result<(), String>;

    fn u8(&self, at: Location) -> Result<u8, String> {
        let mut b = [0; 1];
        self.read(at, &mut b)?;
        Ok(b[0])
    }
    fn u16(&self, at: Location) -> Result<u16, String> {
        let mut b = [0; 2];
        self.read(at, &mut b)?;
        Ok(u16::from_be_bytes(b))
    }
    fn u32(&self, at: Location) -> Result<u32, String> {
        let mut b = [0; 4];
        self.read(at, &mut b)?;
        Ok(u32::from_be_bytes(b))
    }
    fn bytes(&self, at: Location, len: usize) -> Result<Vec<u8>, String> {
        let mut b = vec![0; len];
        self.read(at, &mut b)?;
        Ok(b)
    }
}

impl ResourceBytes for Resource {
    fn read(&self, at: Location, out: &mut [u8]) -> Result<(), String> {
        let at = at.0 as usize;
        let b = self.data.get(at..at + out.len()).ok_or("Read out of range")?;
        out.copy_from_slice(b);
        Ok(())
    }
}
