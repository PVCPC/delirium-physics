pub const BUFFERS: u32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Buffer {
    Vertex,
    Index,
}

pub trait Geometries {
    fn buffer(&mut self, geo: u32, kind: Buffer, index: u32) -> u32;
    fn set_buffer(&mut self, geo: u32, kind: Buffer, index: u32, buffer: u32);
    fn keeps_several(&mut self, geo: u32) -> bool;
    fn building_index(&mut self) -> u32;
    fn drawn_index(&mut self) -> u32;
    fn half_62(&mut self, geo: u32) -> u16;
    fn object_68(&mut self, geo: u32) -> u32;
    fn clear_object_68(&mut self, geo: u32);
    fn set_stride(&mut self, geo: u32, stride: u16);
    fn release_declaration(&mut self, geo: u32);
    fn make_declaration(&mut self, geo: u32, vertex_buffer: u32);
    fn vertex_count(&mut self, vb: u32) -> u16;
    fn vertex_stride(&mut self, vb: u32) -> u32;
    fn vertex_format(&mut self, vb: u32) -> u32;
    fn call_vf4(&mut self, obj: u32);
    fn delete_object(&mut self, obj: u32);
    fn drop_geometry(&mut self, geo: u32);
    fn free(&mut self, block: u32);
}

pub fn current_buffer<G: Geometries>(g: &mut G, geo: u32, kind: Buffer, building: u32) -> u32 {
    let index = if !g.keeps_several(geo) {
        0
    } else if building as u8 != 0 {
        g.building_index()
    } else {
        g.drawn_index()
    };
    g.buffer(geo, kind, index)
}

pub fn half_62<G: Geometries>(g: &mut G, geo: u32) -> u32 {
    g.half_62(geo) as u32
}

pub fn vertex_format<G: Geometries>(g: &mut G, geo: u32) -> u32 {
    let vb = g.buffer(geo, Buffer::Vertex, 0);
    g.vertex_format(vb)
}

pub fn total_vertices<G: Geometries>(g: &mut G, geo: u32) -> u32 {
    let mut total = 0u32;
    for index in 0..BUFFERS {
        let vb = g.buffer(geo, Buffer::Vertex, index);
        let count = if vb == 0 { 0 } else { g.vertex_count(vb) as u32 };
        total = total.wrapping_add(count);
    }
    total
}

pub fn release_resources<G: Geometries>(g: &mut G, geo: u32) {
    let obj = g.object_68(geo);
    if obj != 0 {
        g.call_vf4(obj);
        g.clear_object_68(geo);
    }
    for index in 0..BUFFERS {
        let vb = g.buffer(geo, Buffer::Vertex, index);
        if vb != 0 {
            g.call_vf4(vb);
            g.set_buffer(geo, Buffer::Vertex, index, 0);
        }
    }
}

pub fn set_vertex_buffer<G: Geometries>(g: &mut G, geo: u32, vb: u32) {
    let old = g.buffer(geo, Buffer::Vertex, 0);
    if old != 0 {
        g.delete_object(old);
    }
    g.set_buffer(geo, Buffer::Vertex, 0, vb);
    let stride = g.vertex_stride(vb);
    g.set_stride(geo, stride as u16);
    g.release_declaration(geo);
    g.make_declaration(geo, vb);
}

pub fn delete_geometry<G: Geometries>(g: &mut G, geo: u32, flags: u32) -> u32 {
    g.drop_geometry(geo);
    if flags & 1 != 0 {
        g.free(geo);
    }
    geo
}

use crate::resourcemap::{Links, Location, Placed, ResourceBytes, ResourceMap};
use crate::vertexbuffer::{CpuOnly, IndexBuffer, VertexBuffer};

#[derive(Clone, Debug)]
pub struct Geometry {
    pub vertex_buffers: [Option<VertexBuffer<CpuOnly>>; 4],
    pub index_buffers: [Option<IndexBuffer<CpuOnly>>; 4],
    pub index_count: u32,
    pub primitive_count: u32,
    pub vertex_count: u16,
    pub primitive_type: u16,
    pub stride: u16,
    pub bone_map: Vec<u16>,
}

mod placed_geometry {
    pub const VERTEX_BUFFERS: u32 = 0xC;
    pub const INDEX_BUFFERS: u32 = 0x1C;
    pub const INDEX_COUNT: u32 = 0x2C;
    pub const PRIMITIVE_COUNT: u32 = 0x30;
    pub const VERTEX_COUNT: u32 = 0x34;
    pub const PRIMITIVE_TYPE: u32 = 0x36;
    pub const BONE_MAP: u32 = 0x38;
    pub const STRIDE: u32 = 0x3C;
    pub const BONE_COUNT: u32 = 0x3E;
}

impl Geometry {
    pub fn read(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_geometry::*;
        let vertex_buffers = Self::read_vertex_slots(src, map, at.plus(VERTEX_BUFFERS), links)?;
        let index_buffers = Self::read_index_slots(src, map, at.plus(INDEX_BUFFERS), links)?;
        let bone_count = src.u16(at.plus(BONE_COUNT))?;
        let bone_map = match map.follow(src, at.plus(BONE_MAP), links)? {
            Some(bones) => (0..bone_count as u32).map(|i| src.u16(bones.plus(2 * i))).collect::<Result<_, _>>()?,
            None => vec![],
        };
        links.placed(Placed::Geometry, at);
        Ok(Geometry {
            vertex_buffers,
            index_buffers,
            index_count: src.u32(at.plus(INDEX_COUNT))?,
            primitive_count: src.u32(at.plus(PRIMITIVE_COUNT))?,
            vertex_count: src.u16(at.plus(VERTEX_COUNT))?,
            primitive_type: src.u16(at.plus(PRIMITIVE_TYPE))?,
            stride: src.u16(at.plus(STRIDE))?,
            bone_map,
        })
    }

    pub fn read_vertex_slots(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links)
        -> Result<[Option<VertexBuffer<CpuOnly>>; 4], String> {
        let mut slots: [Option<VertexBuffer<CpuOnly>>; 4] = Default::default();
        for (k, slot) in slots.iter_mut().enumerate() {
            if let Some(b) = map.follow(src, at.plus(4 * k as u32), links)? {
                *slot = Some(VertexBuffer::read_d3d(src, map, b, links)?);
            }
        }
        Ok(slots)
    }

    pub fn read_index_slots(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links)
        -> Result<[Option<IndexBuffer<CpuOnly>>; 4], String> {
        let mut slots: [Option<IndexBuffer<CpuOnly>>; 4] = Default::default();
        for (k, slot) in slots.iter_mut().enumerate() {
            if let Some(b) = map.follow(src, at.plus(4 * k as u32), links)? {
                *slot = Some(IndexBuffer::read_d3d(src, map, b, links)?);
            }
        }
        Ok(slots)
    }
}
