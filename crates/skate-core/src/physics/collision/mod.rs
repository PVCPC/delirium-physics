mod arithmetic;
mod edge;
mod fixup;
mod sphere_triangle;
mod world_contact;

pub use fixup::{ContactPair, TriangleFeature, TriangleFixup, TriangleRegion, fix_up_triangle};
pub use sphere_triangle::{Sphere, SphereTrianglePair, Triangle, intersect_sphere_triangle};
pub use world_contact::{
    WorldContact, WorldContactSettings, sphere_triangle_world_contact, world_separation_limit,
};
