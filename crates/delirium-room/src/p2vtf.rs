//! VTF textures with their full mip chain (from the Portal 2 Rust Engine).
//!
//! The `vtf` crate only decodes the top mip level to RGBA; world textures
//! need mipmaps and DXT data can go to the GPU untouched, so this reader
//! keeps every level and only converts formats the GPU cannot sample.

use anyhow::{bail, Context};
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub const FLAG_POINTSAMPLE: u32 = 0x1;
pub const FLAG_CLAMPS: u32 = 0x4;
pub const FLAG_CLAMPT: u32 = 0x8;
pub const FLAG_ONEBITALPHA: u32 = 0x1000;
pub const FLAG_EIGHTBITALPHA: u32 = 0x2000;
pub const FLAG_ENVMAP: u32 = 0x4000;

/// Pixel data as it will be uploaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8,
    /// DXT1 / BC1 (with or without 1-bit alpha)
    Bc1,
    /// DXT3 / BC2
    Bc2,
    /// DXT5 / BC3
    Bc3,
    Rgba16F,
}

#[derive(Clone, Debug)]
pub struct VtfTexture {
    pub width: u32,
    pub height: u32,
    pub flags: u32,
    pub format: PixelFormat,
    /// Mip levels, largest first. For cubemaps each level holds the six
    /// faces back to back (+X, -X, +Y, -Y, +Z, -Z).
    pub mips: Vec<Vec<u8>>,
    /// 1, or 6 for cubemaps.
    pub faces: u32,
    /// The texture has meaningful alpha (from flags or format).
    pub has_alpha: bool,
    pub reflectivity: [f32; 3],
}

fn rd_u16(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn rd_u32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn rd_f32(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

/// Bytes for one image of a VTF format, `None` for unknown formats.
fn image_size(fmt: i32, w: u32, h: u32) -> Option<usize> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let blocks = w.div_ceil(4) * h.div_ceil(4);
    Some(match fmt {
        13 | 20 => blocks * 8,
        14 | 15 => blocks * 16,
        0 | 1 | 11 | 12 | 16 | 23 | 26 | 27 => w * h * 4,
        2 | 3 | 9 | 10 => w * h * 3,
        4 | 6 | 17 | 18 | 19 | 21 | 22 => w * h * 2,
        5 | 7 | 8 => w * h,
        24 | 25 => w * h * 8,
        28 => w * h * 12,
        29 => w * h * 16,
        _ => return None,
    })
}

fn expand5(v: u16) -> u8 {
    ((v as u32 * 255 + 15) / 31) as u8
}
fn expand6(v: u16) -> u8 {
    ((v as u32 * 255 + 31) / 63) as u8
}
fn expand4(v: u16) -> u8 {
    (v as u8 & 0xf) * 17
}

/// Convert one uncompressed image to RGBA8.
fn to_rgba8(fmt: i32, src: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let n = (w.max(1) * h.max(1)) as usize;
    let mut out = vec![255u8; n * 4];
    for i in 0..n {
        let o = &mut out[i * 4..i * 4 + 4];
        match fmt {
            0 => o.copy_from_slice(&src[i * 4..i * 4 + 4]),
            1 => {
                let s = &src[i * 4..];
                o.copy_from_slice(&[s[3], s[2], s[1], s[0]]);
            }
            11 => {
                let s = &src[i * 4..];
                o.copy_from_slice(&[s[1], s[2], s[3], s[0]]);
            }
            12 => {
                let s = &src[i * 4..];
                o.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
            }
            16 => {
                let s = &src[i * 4..];
                o.copy_from_slice(&[s[2], s[1], s[0], 255]);
            }
            2 | 9 => {
                let s = &src[i * 3..];
                o[..3].copy_from_slice(&s[..3]);
            }
            3 | 10 => {
                let s = &src[i * 3..];
                o[..3].copy_from_slice(&[s[2], s[1], s[0]]);
            }
            4 | 17 => {
                let v = rd_u16(src, i * 2);
                let (a, b) = (expand5(v >> 11), expand5(v & 0x1f));
                let g = expand6((v >> 5) & 0x3f);
                // RGB565: r in high bits; BGR565: b in high bits
                let (r, bb) = if fmt == 4 { (b, a) } else { (a, b) };
                // Valve's RGB565 is actually stored with red in the low bits.
                o[..3].copy_from_slice(&[r, g, bb]);
            }
            18 | 21 => {
                let v = rd_u16(src, i * 2);
                o[0] = expand5((v >> 10) & 0x1f);
                o[1] = expand5((v >> 5) & 0x1f);
                o[2] = expand5(v & 0x1f);
                o[3] = if fmt == 21 && v & 0x8000 == 0 { 0 } else { 255 };
            }
            19 => {
                let v = rd_u16(src, i * 2);
                o[0] = expand4(v >> 8);
                o[1] = expand4(v >> 4);
                o[2] = expand4(v);
                o[3] = expand4(v >> 12);
            }
            5 | 7 => {
                let l = src[i];
                o[..3].copy_from_slice(&[l, l, l]);
            }
            6 => {
                let l = src[i * 2];
                o.copy_from_slice(&[l, l, l, src[i * 2 + 1]]);
            }
            8 => {
                o.copy_from_slice(&[255, 255, 255, src[i]]);
            }
            22 => {
                o[0] = src[i * 2];
                o[1] = src[i * 2 + 1];
                o[2] = 0;
            }
            23 | 26 => o.copy_from_slice(&src[i * 4..i * 4 + 4]),
            25 => {
                for c in 0..4 {
                    o[c] = (rd_u16(src, i * 8 + c * 2) >> 8) as u8;
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

impl VtfTexture {
    /// Parse a VTF (versions 7.0–7.5). Only frame 0, face 0, slice 0 is kept.
    pub fn parse(data: &[u8]) -> anyhow::Result<Self> {
        if data.len() < 64 || &data[0..4] != b"VTF\0" {
            bail!("not a VTF");
        }
        let minor = rd_u32(data, 8);
        let header_size = rd_u32(data, 12) as usize;
        let width = rd_u16(data, 16) as u32;
        let height = rd_u16(data, 18) as u32;
        let flags = rd_u32(data, 20);
        let frames = rd_u16(data, 24).max(1) as usize;
        let first_frame = rd_u16(data, 26);
        let reflectivity = [rd_f32(data, 32), rd_f32(data, 36), rd_f32(data, 40)];
        let hi_fmt = rd_u32(data, 52) as i32;
        let mip_count = (data[56] as u32).max(1);
        let lo_fmt = rd_u32(data, 57) as i32;
        let lo_w = data[61] as u32;
        let lo_h = data[62] as u32;
        let depth = if minor >= 2 { rd_u16(data, 63).max(1) as usize } else { 1 };

        let faces = if flags & FLAG_ENVMAP != 0 {
            if minor < 5 && first_frame != 0xffff {
                7
            } else {
                6
            }
        } else {
            1
        };

        // Locate high-res image data.
        let mut hi_offset = None;
        if minor >= 3 && data.len() >= 80 {
            let num_res = rd_u32(data, 68) as usize;
            for r in 0..num_res {
                let o = 80 + r * 8;
                if o + 8 > data.len() {
                    break;
                }
                if data[o..o + 3] == [0x30, 0, 0] {
                    hi_offset = Some(rd_u32(data, o + 4) as usize);
                }
            }
        }
        let hi_offset = match hi_offset {
            Some(o) => o,
            None => {
                let lo_size = if lo_fmt >= 0 && lo_w > 0 && lo_h > 0 {
                    image_size(lo_fmt, lo_w, lo_h).unwrap_or(0)
                } else {
                    0
                };
                header_size + lo_size
            }
        };

        // Clamp the mip count to what the dimensions allow.
        let max_mips = 32 - width.max(height).max(1).leading_zeros();
        let mip_count = mip_count.min(max_mips);

        let size_of = |level: u32| {
            image_size(hi_fmt, (width >> level).max(1), (height >> level).max(1))
        };
        if size_of(0).is_none() {
            bail!("unsupported VTF format {hi_fmt}");
        }
        // Data is stored smallest mip first; within a mip: frames, faces, slices.
        let mut offsets = vec![0usize; mip_count as usize];
        let mut cursor = hi_offset;
        for level in (0..mip_count).rev() {
            offsets[level as usize] = cursor;
            cursor += size_of(level).unwrap() * frames * faces * depth;
        }
        if cursor > data.len() {
            bail!("VTF truncated ({} > {})", cursor, data.len());
        }

        let (format, has_alpha_fmt) = match hi_fmt {
            13 => (PixelFormat::Bc1, false),
            20 => (PixelFormat::Bc1, true),
            14 => (PixelFormat::Bc2, true),
            15 => (PixelFormat::Bc3, true),
            24 => (PixelFormat::Rgba16F, true),
            0 | 1 | 11 | 12 | 6 | 8 | 19 | 21 | 23 | 25 => (PixelFormat::Rgba8, true),
            _ => (PixelFormat::Rgba8, false),
        };
        let out_faces = if faces >= 6 { 6 } else { 1 };
        let mut mips = Vec::with_capacity(mip_count as usize);
        for level in 0..mip_count {
            let (w, h) = ((width >> level).max(1), (height >> level).max(1));
            let size = size_of(level).unwrap();
            let mut img = Vec::new();
            for face in 0..out_faces {
                let start = offsets[level as usize] + face * size * depth;
                let src = &data[start..start + size];
                match format {
                    PixelFormat::Rgba8 => img.extend(to_rgba8(hi_fmt, src, w, h).with_context(|| format!("format {hi_fmt}"))?),
                    _ => img.extend_from_slice(src),
                }
            }
            mips.push(img);
        }
        let has_alpha = has_alpha_fmt || flags & (FLAG_ONEBITALPHA | FLAG_EIGHTBITALPHA) != 0;
        Ok(VtfTexture {
            width,
            height,
            flags,
            format,
            mips,
            faces: out_faces as u32,
            has_alpha,
            reflectivity,
        })
    }

    /// Decompress BC data to RGBA8 in place (for GPUs without BC support or
    /// textures whose size is not a multiple of the 4x4 block).
    pub fn decompress(&mut self) {
        let fmt = match self.format {
            PixelFormat::Bc1 => texpresso::Format::Bc1,
            PixelFormat::Bc2 => texpresso::Format::Bc2,
            PixelFormat::Bc3 => texpresso::Format::Bc3,
            _ => return,
        };
        let faces = self.faces.max(1) as usize;
        for (level, mip) in self.mips.iter_mut().enumerate() {
            let w = (self.width >> level).max(1) as usize;
            let h = (self.height >> level).max(1) as usize;
            let face_len = mip.len() / faces;
            let mut out = vec![0u8; w * h * 4 * faces];
            for f in 0..faces {
                fmt.decompress(&mip[f * face_len..(f + 1) * face_len], w, h, &mut out[f * w * h * 4..(f + 1) * w * h * 4]);
            }
            *mip = out;
        }
        self.format = PixelFormat::Rgba8;
    }

    /// Top-level RGBA8 pixels (decompressing if needed). For tests and tools.
    pub fn rgba8_level0(&self) -> Option<Vec<u8>> {
        let mut t = self.clone();
        t.mips.truncate(1);
        t.decompress();
        (t.format == PixelFormat::Rgba8).then(|| t.mips.swap_remove(0))
    }

    /// Average colour of the smallest mip, linear-ish sRGB 0..1.
    pub fn average_color(&self) -> [f32; 4] {
        let mut t = self.clone();
        let last = t.mips.len() - 1;
        let lw = (t.width >> last).max(1);
        let lh = (t.height >> last).max(1);
        t.mips = vec![t.mips.pop().unwrap()];
        t.width = lw;
        t.height = lh;
        t.decompress();
        if t.format != PixelFormat::Rgba8 {
            return [0.5, 0.5, 0.5, 1.0];
        }
        let px = &t.mips[0];
        let n = (px.len() / 4).max(1) as f32;
        let mut acc = [0f32; 4];
        for p in px.chunks_exact(4) {
            for c in 0..4 {
                acc[c] += p[c] as f32 / 255.0;
            }
        }
        acc.map(|v| v / n)
    }

    /// Build a Bevy image. `bc_supported`: upload DXT as-is. `srgb`: colour data.
    pub fn into_image(mut self, bc_supported: bool, srgb: bool) -> Image {
        let is_bc = matches!(self.format, PixelFormat::Bc1 | PixelFormat::Bc2 | PixelFormat::Bc3);
        if is_bc && (!bc_supported || self.width % 4 != 0 || self.height % 4 != 0) {
            self.decompress();
        }
        let format = match (self.format, srgb) {
            (PixelFormat::Rgba8, true) => TextureFormat::Rgba8UnormSrgb,
            (PixelFormat::Rgba8, false) => TextureFormat::Rgba8Unorm,
            (PixelFormat::Bc1, true) => TextureFormat::Bc1RgbaUnormSrgb,
            (PixelFormat::Bc1, false) => TextureFormat::Bc1RgbaUnorm,
            (PixelFormat::Bc2, true) => TextureFormat::Bc2RgbaUnormSrgb,
            (PixelFormat::Bc2, false) => TextureFormat::Bc2RgbaUnorm,
            (PixelFormat::Bc3, true) => TextureFormat::Bc3RgbaUnormSrgb,
            (PixelFormat::Bc3, false) => TextureFormat::Bc3RgbaUnorm,
            (PixelFormat::Rgba16F, _) => TextureFormat::Rgba16Float,
        };
        let mip_count = self.mips.len() as u32;
        let faces = self.faces.max(1) as usize;
        // Bevy/wgpu want layer-major data: every mip of face 0, then face 1...
        let data: Vec<u8> = if faces == 1 {
            self.mips.concat()
        } else {
            let mut d = Vec::new();
            for f in 0..faces {
                for mip in &self.mips {
                    let n = mip.len() / faces;
                    d.extend_from_slice(&mip[f * n..(f + 1) * n]);
                }
            }
            d
        };
        let mut image = Image::new_uninit(
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: faces as u32,
            },
            TextureDimension::D2,
            format,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.data = Some(data);
        image.texture_descriptor.mip_level_count = mip_count;
        if faces == 6 {
            image.texture_view_descriptor = Some(bevy::render::render_resource::TextureViewDescriptor {
                dimension: Some(bevy::render::render_resource::TextureViewDimension::Cube),
                ..Default::default()
            });
        }
        let address = |clamp: bool| {
            if clamp {
                ImageAddressMode::ClampToEdge
            } else {
                ImageAddressMode::Repeat
            }
        };
        let point = self.flags & FLAG_POINTSAMPLE != 0;
        let filter = if point {
            ImageFilterMode::Nearest
        } else {
            ImageFilterMode::Linear
        };
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: address(self.flags & FLAG_CLAMPS != 0),
            address_mode_v: address(self.flags & FLAG_CLAMPT != 0),
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter: filter,
            anisotropy_clamp: if point { 1 } else { 16 },
            ..Default::default()
        });
        image
    }
}
