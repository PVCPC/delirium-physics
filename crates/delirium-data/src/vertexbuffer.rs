use crate::resourcemap::{Links, Location, Placed, ResourceBytes, ResourceMap, ResourcePtr};
use crate::vertexformat::{Channel, ElementType, VertexFormat};

pub trait BufferLock {
    fn lock(&mut self, read_only: bool) -> bool;
    fn unlock(&mut self) -> bool;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuOnly;

impl BufferLock for CpuOnly {
    fn lock(&mut self, _read_only: bool) -> bool {
        true
    }
    fn unlock(&mut self) -> bool {
        false
    }
}

#[derive(Clone, Debug)]
pub struct VertexBuffer<L> {
    pub format: VertexFormat,
    pub count: u16,
    pub stride: u32,
    pub vertices: Vec<u8>,
    pub locked: bool,
    pub unpacked: bool,
    pub gpu: L,
}

const HALF_TO_FLOAT_BIAS: u32 = 127 - 15;

pub fn widen_half(h: u16) -> f32 {
    if h == 0 {
        return 0.0;
    }
    let h = h as u32;
    let sign_and_mantissa = ((h << 3) & 0xFFFC_0000) | (h & 0x3FF);
    let exponent = ((h >> 10) & 0x1F) + HALF_TO_FLOAT_BIAS;
    f32::from_bits((sign_and_mantissa << 13) | (exponent << 23))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlendWeights {
    Row([u8; 16]),
    Floats([f32; 4]),
}

pub const WEIGHT_BYTE_SCALE: f32 = 0.003_921_568_393_707_275;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditLock {
    pub was_locked: bool,
}

impl<L: BufferLock> VertexBuffer<L> {
    pub fn begin_edit(&mut self, read_only: bool) -> EditLock {
        let was_locked = self.locked;
        if !was_locked {
            self.locked = self.gpu.lock(read_only);
        }
        EditLock { was_locked }
    }

    pub fn end_edit(&mut self, edit: EditLock) {
        if self.locked && !edit.was_locked {
            self.locked = self.gpu.unlock();
        }
    }

    fn element_at(&self, index: u32, channel: Channel) -> usize {
        (self.stride * index + self.format.element_offset(channel)) as usize
    }

    pub fn element_word(&mut self, index: u32, channel: Channel) -> u32 {
        let edit = self.begin_edit(false);
        let at = self.element_at(index, channel);
        let word = u32::from_be_bytes(self.vertices[at..at + 4].try_into().unwrap());
        self.end_edit(edit);
        word
    }

    pub fn element_bytes(&self, index: u32, channel: Channel, len: usize) -> &[u8] {
        let at = self.element_at(index, channel);
        &self.vertices[at..at + len]
    }

    pub fn vertex_row(&mut self, index: u32) -> [u8; 16] {
        let edit = self.begin_edit(false);
        let at = (self.stride * index) as usize & !15;
        let row = self.vertices[at..at + 16].try_into().unwrap();
        self.end_edit(edit);
        row
    }

    pub fn tex_coord(&mut self, index: u32, set: u8) -> [f32; 2] {
        let edit = self.begin_edit(false);
        let at = self.element_at(index, Channel::TexCoord(set));
        let b = &self.vertices[at..];
        let [u, v] = if self.format.types[Channel::TexCoord(0).number() as usize] == ElementType::Half2 {
            [widen_half(u16::from_be_bytes([b[0], b[1]])), widen_half(u16::from_be_bytes([b[2], b[3]]))]
        } else {
            [0, 4].map(|k| f32::from_bits(u32::from_be_bytes(b[k..k + 4].try_into().unwrap())))
        };
        self.end_edit(edit);
        [u, 1.0 - v]
    }

    pub fn blend_weights(&mut self, index: u32) -> BlendWeights {
        let edit = self.begin_edit(false);
        let at = self.element_at(index, Channel::BlendWeight);
        let weights = if self.unpacked {
            BlendWeights::Row(self.vertices[at & !15..(at & !15) + 16].try_into().unwrap())
        } else {
            let w = u32::from_be_bytes(self.vertices[at..at + 4].try_into().unwrap());
            BlendWeights::Floats([16, 8, 0, 24].map(|s| ((w >> s) & 0xFF) as f32 * WEIGHT_BYTE_SCALE))
        };
        self.end_edit(edit);
        weights
    }

    pub fn copy_vertices(&mut self, from: &[u8]) {
        let edit = self.begin_edit(false);
        self.vertices[..from.len()].copy_from_slice(from);
        self.end_edit(edit);
    }

    pub fn clear_vertices(&mut self) {
        let edit = self.begin_edit(false);
        let len = self.count as usize * self.stride as usize;
        self.vertices[..len].fill(0);
        self.end_edit(edit);
    }

    pub fn copy_vertices_from<M: BufferLock>(&mut self, from: &mut VertexBuffer<M>) {
        let own = self.begin_edit(false);
        let other = from.begin_edit(true);
        let len = self.count as usize * self.stride as usize;
        self.copy_vertices(&from.vertices[..len]);
        from.end_edit(other);
        self.end_edit(own);
    }
}

mod placed_vertex {
    pub const COUNT: u32 = 4;
    pub const UNPACKED: u32 = 7;
    pub const STRIDE: u32 = 0xC;
    pub const FORMAT: u32 = 0x10;
    pub const DATA: u32 = 0x18;
    pub const D3D: u32 = 0x1C;
    pub const D3D_DATA: u32 = 0x18;
}

impl VertexBuffer<CpuOnly> {
    pub fn read(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_vertex::*;
        let count = src.u16(at.plus(COUNT))?;
        let unpacked = src.u8(at.plus(UNPACKED))? != 0;
        let stride = src.u32(at.plus(STRIDE))?;
        let format_at = map.follow(src, at.plus(FORMAT), links)?.ok_or("A vertex buffer without a format")?;
        let mut record = [0; 16];
        src.read(format_at, &mut record)?;
        let data = map.follow(src, at.plus(DATA), links)?.ok_or("A vertex buffer without data")?;
        let vertices = src.bytes(data, count as usize * stride as usize)?;
        links.placed(Placed::VertexBuffer, at);
        Ok(VertexBuffer {
            format: crate::model::format_from_record(&record),
            count,
            stride,
            vertices,
            locked: false,
            unpacked,
            gpu: CpuOnly,
        })
    }

    pub fn read_d3d(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_vertex::*;
        let buffer = Self::read(src, map, at, links)?;
        let header = map.follow(src, at.plus(D3D), links)?.ok_or("A D3D vertex buffer without its header")?;
        let gpu = src.u32(header.plus(D3D_DATA))?;
        let target = map.resolve(ResourcePtr(gpu & !3)).ok_or_else(|| format!("Bad GPU address {gpu:08X}"))?;
        links.gpu_link(header.plus(D3D_DATA), target);
        links.placed(Placed::VertexBufferD3d, at);
        Ok(buffer)
    }
}

pub struct VertexBufferEditor<'a, L: BufferLock> {
    pub buffer: &'a mut VertexBuffer<L>,
    edit: Option<EditLock>,
}

impl<'a, L: BufferLock> VertexBufferEditor<'a, L> {
    pub fn new(buffer: &'a mut VertexBuffer<L>, lock: bool, read_only: bool) -> Self {
        let edit = if lock { Some(buffer.begin_edit(read_only)) } else { None };
        VertexBufferEditor { buffer, edit }
    }

    pub fn holding(buffer: &'a mut VertexBuffer<L>, edit: Option<EditLock>) -> Self {
        VertexBufferEditor { buffer, edit }
    }

    pub fn edit(&self) -> Option<EditLock> {
        self.edit
    }
}

impl<L: BufferLock> Drop for VertexBufferEditor<'_, L> {
    fn drop(&mut self) {
        if let Some(edit) = self.edit.take() {
            self.buffer.end_edit(edit);
        }
    }
}

#[derive(Clone, Debug)]
pub struct IndexBuffer<L> {
    pub count: u32,
    pub indices: Vec<u16>,
    pub gpu: L,
}

mod placed_index {
    pub const COUNT: u32 = 4;
    pub const DATA: u32 = 8;
    pub const D3D: u32 = 0xC;
    pub const D3D_DATA: u32 = 0x18;
}

impl IndexBuffer<CpuOnly> {
    pub fn read(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_index::*;
        let count = src.u32(at.plus(COUNT))?;
        let data = map.follow(src, at.plus(DATA), links)?.ok_or("An index buffer without data")?;
        let indices = src.bytes(data, 2 * count as usize)?.chunks_exact(2).map(|b| u16::from_be_bytes([b[0], b[1]])).collect();
        links.placed(Placed::IndexBuffer, at);
        Ok(IndexBuffer { count, indices, gpu: CpuOnly })
    }

    pub fn read_d3d(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed_index::*;
        let buffer = Self::read(src, map, at, links)?;
        let header = map.follow(src, at.plus(D3D), links)?.ok_or("A D3D index buffer without its header")?;
        map.follow(src, header.plus(D3D_DATA), links)?;
        links.placed(Placed::IndexBufferD3d, at);
        Ok(buffer)
    }
}

impl<L: BufferLock> IndexBuffer<L> {
    pub fn copy_indices<M: BufferLock>(&mut self, from: &mut IndexBuffer<M>) {
        from.gpu.lock(true);
        self.gpu.lock(false);
        let n = self.count as usize;
        self.indices[..n].copy_from_slice(&from.indices[..n]);
        self.gpu.unlock();
        from.gpu.unlock();
    }
}
