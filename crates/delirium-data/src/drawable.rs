use crate::grmmodel::Model;
use crate::resourcemap::{Links, Location, Placed, ResourceBytes, ResourceMap};
use crate::shadergroup::{GroupParts, ShaderGroup};
use crate::trackerrooms::EntityType;

pub const LODS: usize = 4;
const CHOSEN_LODS: usize = 3;

#[derive(Clone, Debug)]
pub struct Drawable<T, S, K> {
    pub skeleton: Option<K>,
    pub shader_group: Option<ShaderGroup<T, S>>,
    pub lod_group: LodGroup,
}

#[derive(Clone, Debug)]
pub struct LodGroup {
    pub centre: [f32; 3],
    pub box_min: [f32; 3],
    pub box_max: [f32; 3],
    pub radius: f32,
    pub lods: [Option<Vec<Option<Model>>>; LODS],
    pub distances: [f32; LODS],
    pub bucket_masks: [u32; LODS],
}

pub trait DrawBucket {
    fn draw_bucket(&self) -> u8;
}

impl DrawBucket for crate::shaderfx::Shader {
    fn draw_bucket(&self) -> u8 {
        self.draw_bucket
    }
}

pub trait DrawableParts: GroupParts {
    type Skeleton;
    fn skeleton(&mut self, src: &impl ResourceBytes, map: &ResourceMap, field: Location, links: &mut impl Links)
        -> Result<Option<Self::Skeleton>, String>;
}

mod placed {
    pub const SHADER_GROUP: u32 = 8;
    pub const SKELETON: u32 = 0xC;
    pub const LOD_GROUP: u32 = 0x10;
    pub mod lod_group {
        pub const CENTRE: u32 = 0;
        pub const BOX_MIN: u32 = 0x10;
        pub const BOX_MAX: u32 = 0x20;
        pub const MODELS: u32 = 0x30;
        pub const DISTANCES: u32 = 0x40;
        pub const BUCKET_MASKS: u32 = 0x50;
        pub const RADIUS: u32 = 0x60;
    }
    pub const LIST_COUNT: u32 = 4;
}

impl<T, S, K> Drawable<T, S, K> {
    pub fn new() -> Self {
        Drawable { shader_group: None, skeleton: None, lod_group: LodGroup::new() }
    }

    pub fn read<P>(src: &impl ResourceBytes, map: &ResourceMap, at: Location, parts: &mut P, links: &mut impl Links)
        -> Result<Self, String>
    where
        P: DrawableParts<Textures = T, Shader = S, Skeleton = K>,
        S: DrawBucket,
    {
        links.placed(Placed::DrawableBase, at);
        let shader_group = match map.follow(src, at.plus(placed::SHADER_GROUP), links)? {
            Some(g) => Some(ShaderGroup::read(src, map, g, parts, links)?),
            None => None,
        };
        let (skeleton, mut lod_group) = Self::read_rest(src, map, at, parts, links)?;
        if let Some(group) = &shader_group {
            lod_group.refresh_bucket_masks(&group.shaders);
        }
        links.placed(Placed::Drawable, at);
        Ok(Drawable { shader_group, skeleton, lod_group })
    }

    pub fn read_sharing<P>(src: &impl ResourceBytes, map: &ResourceMap, at: Location, shared: Option<&ShaderGroup<T, S>>,
                           parts: &mut P, links: &mut impl Links) -> Result<Self, String>
    where
        P: DrawableParts<Textures = T, Shader = S, Skeleton = K>,
        S: DrawBucket,
    {
        links.placed(Placed::DrawableBase, at);
        let (skeleton, mut lod_group) = Self::read_rest(src, map, at, parts, links)?;
        if let Some(group) = shared {
            lod_group.refresh_bucket_masks(&group.shaders);
        }
        links.placed(Placed::Drawable, at);
        Ok(Drawable { shader_group: None, skeleton, lod_group })
    }

    fn read_rest<P: DrawableParts<Skeleton = K>>(src: &impl ResourceBytes, map: &ResourceMap, at: Location, parts: &mut P,
                                                  links: &mut impl Links) -> Result<(Option<K>, LodGroup), String> {
        let skeleton = parts.skeleton(src, map, at.plus(placed::SKELETON), links)?;
        let lod_group = LodGroup::read(src, map, at.plus(placed::LOD_GROUP), links)?;
        Ok((skeleton, lod_group))
    }
}

impl<T, S, K> Default for Drawable<T, S, K> {
    fn default() -> Self {
        Self::new()
    }
}

impl LodGroup {
    pub fn new() -> LodGroup {
        LodGroup {
            centre: [0.0; 3],
            box_min: [0.0; 3],
            box_max: [0.0; 3],
            radius: 0.0,
            lods: [None, None, None, None],
            distances: [0.0; LODS],
            bucket_masks: [u32::MAX; LODS],
        }
    }

    pub fn read(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links) -> Result<Self, String> {
        use placed::lod_group::*;
        let f = |off: u32| src.u32(at.plus(off)).map(f32::from_bits);
        let v3 = |off: u32| -> Result<[f32; 3], String> { Ok([f(off)?, f(off + 4)?, f(off + 8)?]) };
        let lods = Self::read_model_lists(src, map, at.plus(MODELS), links)?;
        let mut distances = [0.0; LODS];
        let mut bucket_masks = [0; LODS];
        for k in 0..LODS as u32 {
            distances[k as usize] = f(DISTANCES + 4 * k)?;
            bucket_masks[k as usize] = src.u32(at.plus(BUCKET_MASKS + 4 * k))?;
        }
        Ok(LodGroup {
            centre: v3(CENTRE)?,
            box_min: v3(BOX_MIN)?,
            box_max: v3(BOX_MAX)?,
            radius: f(RADIUS)?,
            lods,
            distances,
            bucket_masks,
        })
    }

    pub fn read_model_lists(src: &impl ResourceBytes, map: &ResourceMap, at: Location, links: &mut impl Links)
        -> Result<[Option<Vec<Option<Model>>>; LODS], String> {
        let mut lods = [None, None, None, None];
        for (k, lod) in lods.iter_mut().enumerate() {
            if let Some(list) = map.follow(src, at.plus(4 * k as u32), links)? {
                *lod = Some(read_model_list(src, map, list, links)?);
            }
        }
        Ok(lods)
    }

    pub fn refresh_bucket_masks<S: DrawBucket>(&mut self, shaders: &[Option<S>]) {
        for (lod, mask) in self.lods.iter().zip(&mut self.bucket_masks) {
            if let Some(models) = lod {
                *mask = bucket_mask(models, shaders);
            }
        }
    }

    pub fn choose_lod(&self, matrix: &[[f32; 4]; 4], view: &mut impl SphereView) -> LodChoice {
        let [x, y, z] = self.centre;
        let centre: [f32; 3] = std::array::from_fn(|k| {
            x.mul_add(matrix[0][k], y.mul_add(matrix[1][k], z.mul_add(matrix[2][k], matrix[3][k])))
        });
        self.choose_lod_at(centre, view)
    }

    pub fn choose_lod_inline(&self, matrix: &[[f32; 4]; 4], view: &mut impl SphereView) -> LodChoice {
        let [x, y, z] = self.centre;
        let centre: [f32; 3] = std::array::from_fn(|k| x * matrix[0][k] + y * matrix[1][k] + z * matrix[2][k] + matrix[3][k]);
        self.choose_lod_at(centre, view)
    }

    fn choose_lod_at(&self, centre: [f32; 3], view: &mut impl SphereView) -> LodChoice {
        let (seen, distance) = view.sphere(centre, self.radius);
        let to_surface = distance - self.radius;
        let lod = if seen {
            (0..CHOSEN_LODS).find(|&k| {
                let farther = to_surface.partial_cmp(&self.distances[k]) == Some(std::cmp::Ordering::Greater);
                !farther && self.lods[k].is_some()
            })
        } else {
            None
        };
        let surface_distance = if to_surface <= 0.0 { 0.0 } else { to_surface };
        LodChoice { lod, seen, surface_distance }
    }

    pub fn draw_lod<S: DrawBucket>(&self, world: &[[f32; 4]; 4], lod: usize, bucket: Option<u8>,
                                   visible_models: Option<&ModelMask>, shaders: &[Option<S>], out: &mut impl GeometryDraw) {
        out.set_world(world);
        let Some(Some(models)) = self.lods.get(lod) else { return };
        for (m, model) in models.iter().enumerate().rev() {
            if visible_models.is_some_and(|mask| !mask.has(m)) {
                continue;
            }
            let Some(model) = model else { continue };
            for (g, &shader) in model.shader_indices.iter().enumerate().rev() {
                let in_bucket = match bucket {
                    None => true,
                    Some(b) => shaders.get(shader as usize).and_then(Option::as_ref).is_some_and(|s| s.draw_bucket() == b),
                };
                if in_bucket {
                    out.draw_geometry(model, g);
                }
            }
        }
    }
}

impl Default for LodGroup {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodChoice {
    pub lod: Option<usize>,
    pub seen: bool,
    pub surface_distance: f32,
}

pub trait SphereView {
    fn sphere(&mut self, centre: [f32; 3], radius: f32) -> (bool, f32);
}

pub struct ModelMask<'a>(pub &'a [u32]);

impl ModelMask<'_> {
    fn has(&self, m: usize) -> bool {
        self.0.get(m / 32).is_some_and(|w| w & (1 << (m % 32)) != 0)
    }
}

pub trait GeometryDraw {
    fn set_world(&mut self, world: &[[f32; 4]; 4]);
    fn draw_geometry(&mut self, model: &Model, geometry: usize);
}

pub fn bucket_mask<S: DrawBucket>(models: &[Option<Model>], shaders: &[Option<S>]) -> u32 {
    models.iter().flatten().fold(0, |mask, model| mask | model.draw_buckets(shaders))
}

pub fn read_model_list(src: &impl ResourceBytes, map: &ResourceMap, list: Location, links: &mut impl Links)
    -> Result<Vec<Option<Model>>, String> {
    let count = src.u16(list.plus(placed::LIST_COUNT))? as u32;
    let Some(models) = map.follow(src, list, links)? else { return Ok(vec![]) };
    (0..count)
        .map(|i| map.follow(src, models.plus(4 * i), links)?.map(|m| Model::read(src, map, m, links)).transpose())
        .collect()
}

impl LodGroup {
    pub const NO_MODELS_MASK: u32 = u32::MAX;

    pub fn first_bucket_mask(&self, from: usize) -> Option<u32> {
        self.bucket_masks.iter().skip(from).copied().find(|&m| m != Self::NO_MODELS_MASK)
    }
}

impl<T, S, K> Drawable<T, S, K> {
    pub fn first_bucket_mask(&self, from: usize) -> Option<u32> {
        self.lod_group.first_bucket_mask(from)
    }

    pub fn parts_light_alone(&self) -> bool {
        let Some(models) = &self.lod_group.lods[0] else { return true };
        models.iter().all(|m| match m {
            Some(m) => !m.bounds.is_empty() && m.matrix_index == 0,
            None => false,
        })
    }
}

pub fn building_parts_light_alone<T, S, K>(kind: EntityType, drawable: Option<&Drawable<T, S, K>>) -> bool {
    match (kind, drawable) {
        (EntityType::Building, Some(d)) => d.parts_light_alone(),
        _ => false,
    }
}
