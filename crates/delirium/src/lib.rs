pub mod prelude {
    pub use bevy_math::prelude::*;
    pub use bevy_math::EulerRot;
    pub use bevy_transform::components::Transform;
    pub use tracing::{debug, info, warn};

    pub fn default<T: Default>() -> T {
        T::default()
    }
}

pub(crate) trait Fma {
    fn fma(self, b: Self, c: Self) -> Self;
}
impl Fma for f32 {
    fn fma(self, b: f32, c: f32) -> f32 {
        self.mul_add(b, c)
    }
}

pub(crate) trait Vd {
    fn vd(self, b: Self) -> f32;
}
impl Vd for prelude::Vec3 {
    fn vd(self, b: Self) -> f32 {
        let p = |i: usize| self[i] as f64 * b[i] as f64;
        ((p(0) + p(2)) + p(1)) as f32
    }
}

pub(crate) fn vm4(a: prelude::Vec3, b: prelude::Vec3, c: prelude::Vec3, d: prelude::Vec3, v: prelude::Vec3) -> prelude::Vec3 {
    let k = |i: usize| (((a[i] as f64 * v.x as f64) + (c[i] as f64 * v.z as f64)) + ((b[i] as f64 * v.y as f64) + d[i] as f64)) as f32;
    prelude::Vec3::new(k(0), k(1), k(2))
}

pub(crate) trait Vx {
    fn vx(self, b: Self) -> Self;
}
impl Vx for prelude::Vec3 {
    fn vx(self, b: Self) -> Self {
        Self::new((-self.z).mul_add(b.y, self.y * b.z), (-self.x).mul_add(b.z, self.z * b.x), (-self.y).mul_add(b.x, self.x * b.y))
    }
}

mod artic;
pub mod brace;
pub mod bvh;
pub mod bump;
pub mod climbgrab;
pub mod collision;
pub mod explosion;
pub mod falldown;
pub mod highfall;
pub mod jumproll;
pub mod onfire;
pub mod taskctor;
pub mod gjk;
pub mod curved;
pub mod grabhelper;
pub mod nm;
pub mod nmaw;
pub mod nmbb;
pub mod nmbf;
pub mod nmbr;
pub mod extras;
pub mod extras_fall;
pub mod extras_held;
pub mod nmbw;
pub mod nmcf;
pub mod nmdbm;
pub mod nmfl;
pub mod nmfow;
pub mod nmgr;
pub mod nmhf;
pub mod nmhl;
pub mod nmpa;
pub mod nmpose;
pub mod nmpd;
pub mod nmrds;
pub mod nmrelax;
pub mod nmrollrelax;
pub mod nmrs;
pub mod nmru;
pub mod nmscript;
pub mod nmshot;
pub mod nmsit;
pub mod nmst;
pub mod playerctl;
pub mod shottask;
pub mod meleetask;
pub mod rigid;
pub mod ragdoll;
pub mod ragstart;
pub mod sphere;
pub mod msg;
pub use delirium_data::estimate;
