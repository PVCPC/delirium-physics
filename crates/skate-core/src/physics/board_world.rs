mod broadphase;
mod query_index;
pub mod query_metadata;
use crate::math::Vector3;
use broadphase::{conservative_bounds, primitive_bounds};
use query_metadata::{Bounds, QueryMetadata};

use super::{
    board::BodyId,
    board_runtime::BoardRuntime,
    board_step::{BoardCollision, CollisionBody},
    collision::{Sphere, Triangle, WorldContactSettings},
    contact::{RetailContactInput, RetailContactMaterial, combine_contact_materials},
    triangle_query::{TriangleLineHit, triangle_segment},
    world_contact::{
        ContactBuffer, ContactPrimitive, ContactRecord, primitive_triangle_world_contacts,
        triangle_from_volume,
    },
};

#[derive(Clone, Copy, Debug)]
pub struct WorldTriangle {
    pub triangle: Triangle,
    pub material: RetailContactMaterial,
    pub tag: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldLineHit {
    pub geometry: TriangleLineHit,
    pub tag: u32,
}

impl WorldTriangle {
    pub fn from_vertices(
        vertices: [Vector3; 3],
        material: RetailContactMaterial,
        tag: u32,
        flags: u32,
        edge_cosines: [f32; 3],
        fatness: f32,
    ) -> Option<Self> {
        if vertices
            .iter()
            .any(|v| !v.x.is_finite() || !v.y.is_finite() || !v.z.is_finite())
        {
            return None;
        }
        let triangle = triangle_from_volume(vertices, fatness, edge_cosines, flags);
        let normal = triangle.feature.normal;
        if triangle
            .edge_lengths
            .iter()
            .any(|&v| !v.is_finite() || v <= 0.0)
            || !normal.x.is_finite()
            || !normal.y.is_finite()
            || !normal.z.is_finite()
            || normal == Vector3::ZERO
        {
            return None;
        }
        Some(Self {
            triangle,
            material,
            tag,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ContactRetentionSettings {
    pub capacity: u32,
    pub duplicate_distance_squared: f32,
    pub deferred_reduction: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct WheelWorldSettings {
    pub radius: f32,
    pub material: RetailContactMaterial,
    pub query: WorldContactSettings,
    pub retention: ContactRetentionSettings,
}

#[derive(Clone, Copy, Debug)]
pub struct BoardWorldVolume {
    pub body: CollisionBody,
    pub primitive: ContactPrimitive,
    pub linear_velocity: Vector3,
    pub material: RetailContactMaterial,
}

pub struct BoardWorld {
    triangles: Vec<WorldTriangle>,
    triangle_bounds: Vec<Bounds>,
    query_metadata: Option<QueryMetadata>,
    query_index: query_index::QueryIndex,
    maximum_fatness: f32,
    maximum_triangle_margin: f32,
    imported_floor_seams: bool,
    contacts: Vec<BoardCollision>,
    buffer: ContactBuffer,
}

impl BoardWorld {
    pub fn new(triangles: Vec<WorldTriangle>) -> Self {
        let triangle_bounds: Vec<_> = triangles
            .iter()
            .map(|t| {
                Bounds::from_points(t.triangle.vertices).unwrap_or(Bounds {
                    min: Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
                    max: Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
                })
            })
            .collect();
        let maximum_fatness = triangles
            .iter()
            .map(|t| {
                if t.triangle.fatness.is_finite() && t.triangle.fatness >= 0. {
                    t.triangle.fatness
                } else {
                    f32::INFINITY
                }
            })
            .fold(0., f32::max);
        let maximum_triangle_margin = triangle_bounds
            .iter()
            .map(|b| {
                let span = (b.max.x - b.min.x)
                    .max(b.max.y - b.min.y)
                    .max(b.max.z - b.min.z);
                span * (2. * broadphase::THIN_MARGIN)
            })
            .fold(0., f32::max);
        Self {
            triangles,
            triangle_bounds,
            query_metadata: None,
            query_index: query_index::QueryIndex::default(),
            maximum_fatness,
            maximum_triangle_margin,
            imported_floor_seams: false,
            contacts: Vec::new(),
            buffer: ContactBuffer {
                count: 0,
                flushed: 0,
                capacity: 0,
                dropped: 0,
                distance_squared_threshold: 0.0,
                allow_flush: 1,
                deferred_reduction: 0,
                full: 0,
                records: [[0; 64]; 50],
            },
        }
    }

    pub fn with_query_metadata(
        triangles: Vec<WorldTriangle>,
        metadata: QueryMetadata,
    ) -> Result<Self, &'static str> {
        metadata.validate(&triangles)?;
        let mut world = Self::new(triangles);
        world.query_index = query_index::QueryIndex::new(&metadata.meshes);
        world.query_metadata = Some(metadata);
        Ok(world)
    }

    pub fn query_metadata(&self) -> Result<&QueryMetadata, &'static str> {
        self.query_metadata
            .as_ref()
            .ok_or("Canonical world has no authored query metadata")
    }

    pub fn triangles(&self) -> &[WorldTriangle] {
        &self.triangles
    }

    pub fn enable_imported_floor_seams(&mut self) {
        self.imported_floor_seams = true;
    }

    pub fn query_thin_line(
        &self,
        start: Vector3,
        end: Vector3,
    ) -> Result<Option<WorldLineHit>, &'static str> {
        self.query_swept_line(start, end, 0.0)
    }

    pub fn query_swept_line(
        &self,
        start: Vector3,
        end: Vector3,
        radius: f32,
    ) -> Result<Option<WorldLineHit>, &'static str> {
        if !radius.is_finite() || radius < 0.0 {
            return Err("line query radius must be finite and nonnegative");
        }
        let direction = Vector3::new(end.x - start.x, end.y - start.y, end.z - start.z);
        let mut nearest: Option<WorldLineHit> = None;
        for (_, entry) in self.line_candidates(start, end, radius) {
            let mut geometry = TriangleLineHit {
                position: Vector3::ZERO,
                normal: Vector3::ZERO,
                fraction: 0.0,
                volume_parameter: [0.0; 3],
            };
            if triangle_segment(
                &mut geometry,
                start,
                direction,
                entry.triangle.vertices,
                radius,
                entry.triangle.fatness,
            ) {
                if nearest
                    .as_ref()
                    .is_none_or(|hit| geometry.fraction < hit.geometry.fraction)
                {
                    nearest = Some(WorldLineHit {
                        geometry,
                        tag: entry.tag,
                    });
                }
            }
        }
        Ok(nearest)
    }

    pub fn query(
        &mut self,
        board: &BoardRuntime,
        settings: WheelWorldSettings,
    ) -> &[BoardCollision] {
        let poses = board.part_transforms();
        let volumes: Vec<_> = BodyId::ORDER[..4]
            .iter()
            .filter_map(|&id| {
                let body = &board.bodies()[id.index()];
                (body.state_flags != 1).then_some(BoardWorldVolume {
                    body: CollisionBody::Board(id),
                    primitive: ContactPrimitive::Sphere(Sphere {
                        center: poses[id.index()].translation,
                        radius: settings.radius,
                    }),
                    linear_velocity: body.rates.linear_velocity,
                    material: settings.material,
                })
            })
            .collect();
        self.query_primitives(&volumes, settings.query, settings.retention)
    }

    pub fn query_primitives(
        &mut self,
        volumes: &[BoardWorldVolume],
        query: WorldContactSettings,
        retention: ContactRetentionSettings,
    ) -> &[BoardCollision] {
        self.contacts.clear();
        self.buffer.count = 0;
        self.buffer.flushed = 0;
        self.buffer.dropped = 0;
        self.buffer.full = 0;
        self.buffer.capacity = retention.capacity;
        self.buffer.distance_squared_threshold = retention.duplicate_distance_squared;
        self.buffer.deferred_reduction = u8::from(retention.deferred_reduction);
        let padding =
            if query.volume_padding.is_finite() && query.maximum_separating_distance.is_finite() {
                query.volume_padding.max(0.) + query.maximum_separating_distance.max(0.)
            } else {
                f32::INFINITY
            };
        let volume_bounds: Vec<_> = volumes
            .iter()
            .map(|v| {
                primitive_bounds(v.primitive)
                    .map(|b| conservative_bounds(b, padding + self.maximum_fatness))
            })
            .collect();
        let bounds: Option<Vec<_>> = volume_bounds.iter().copied().collect();
        let bounds = bounds
            .and_then(|b| Bounds::from_points(b.iter().flat_map(|b| [b.min, b.max])))
            .map(|b| b.expanded(padding));
        let candidates: Vec<_> = self
            .candidate_ranges(bounds)
            .into_iter()
            .flatten()
            .collect();
        let triangles = &self.triangles;
        let triangle_bounds = &self.triangle_bounds;
        let imported_floor_seams = self.imported_floor_seams;
        let output = &mut self.contacts;
        let mut publish = |records: &[ContactRecord]| {
            output.extend(records.iter().map(collision_from_record));
        };
        for &index in &candidates {
            let entry = &triangles[index];
            for (volume, volume_bounds) in volumes.iter().zip(&volume_bounds) {
                if self.query_metadata.is_some()
                    && volume_bounds.is_some_and(|b| !self.triangle_bounds[index].overlaps(b))
                {
                    continue;
                }
                let Some(manifold) = primitive_triangle_world_contacts(
                    volume.primitive,
                    entry.triangle,
                    volume.linear_velocity,
                    query,
                ) else {
                    continue;
                };
                let material = combine_contact_materials(volume.material, entry.material);
                for pair in &manifold.points[..manifold.count] {
                    if imported_floor_seams
                        && imported_internal_floor_edge(
                            index,
                            entry,
                            pair.b,
                            manifold.normal,
                            &candidates,
                            triangles,
                            triangle_bounds,
                        )
                    {
                        continue;
                    }
                    let contact = RetailContactInput {
                        position_on_a: pair.a,
                        position_on_b: pair.b,
                        normal: manifold.normal,
                        restitution: material.restitution,
                        static_friction: material.static_friction,
                        dynamic_friction: material.dynamic_friction,
                        tag: entry.tag,
                    };
                    let Some(slot) = self.buffer.allocate(&mut publish) else {
                        self.buffer.flush(&mut publish);
                        return &self.contacts;
                    };
                    self.buffer.records[slot] = retention_record(volume.body, contact);
                    if self.buffer.last_is_duplicate() {
                        self.buffer.count -= 1;
                    }
                }
            }
        }
        self.buffer.flush(&mut publish);
        &self.contacts
    }

    pub fn contacts(&self) -> &[BoardCollision] {
        &self.contacts
    }

    pub fn dropped_contacts(&self) -> u32 {
        self.buffer.dropped
    }
}

fn imported_internal_floor_edge(
    source_index: usize,
    source: &WorldTriangle,
    contact: Vector3,
    contact_normal: Vector3,
    candidates: &[usize],
    triangles: &[WorldTriangle],
    bounds: &[Bounds],
) -> bool {
    let face = source.triangle.feature.normal;
    let lateral = contact_normal.x.hypot(contact_normal.z);
    if lateral < 0.05 {
        return false;
    }
    let probe = |sign: f32| {
        Vector3::new(
            contact.x + sign * 0.005 * contact_normal.x / lateral,
            contact.y,
            contact.z + sign * 0.005 * contact_normal.z / lateral,
        )
    };
    if face.y >= 0.98 && contact_normal.y > 0.05 {
        let mut beyond = probe(1.);
        let origin = source.triangle.vertices[0];
        beyond.y =
            origin.y - (face.x * (beyond.x - origin.x) + face.z * (beyond.z - origin.z)) / face.y;
        return !point_in_floor_triangle(beyond, source.triangle.vertices)
            && floor_at(
                beyond,
                Some(source_index),
                Some(face),
                candidates,
                triangles,
                bounds,
            );
    }
    if face.y.abs() > 0.2
        || source
            .triangle
            .vertices
            .iter()
            .any(|p| p.y > contact.y + 0.01)
    {
        return false;
    }
    floor_at(probe(-1.), None, None, candidates, triangles, bounds)
        && floor_at(probe(1.), None, None, candidates, triangles, bounds)
}

fn floor_at(
    probe: Vector3,
    excluded: Option<usize>,
    reference: Option<Vector3>,
    candidates: &[usize],
    triangles: &[WorldTriangle],
    bounds: &[Bounds],
) -> bool {
    candidates.iter().any(|&other_index| {
        if excluded == Some(other_index) {
            return false;
        }
        let other = &triangles[other_index];
        let normal = other.triangle.feature.normal;
        if normal.y < 0.98
            || reference.is_some_and(|face| {
                face.x * normal.x + face.y * normal.y + face.z * normal.z < 0.999
            })
        {
            return false;
        }
        let b = bounds[other_index];
        if probe.x < b.min.x - 0.001
            || probe.x > b.max.x + 0.001
            || probe.z < b.min.z - 0.001
            || probe.z > b.max.z + 0.001
        {
            return false;
        }
        let p = other.triangle.vertices;
        let plane_gap = (other.triangle.feature.normal.x * (probe.x - p[0].x)
            + other.triangle.feature.normal.y * (probe.y - p[0].y)
            + other.triangle.feature.normal.z * (probe.z - p[0].z))
            .abs();
        plane_gap <= 0.002 && point_in_floor_triangle(probe, p)
    })
}

fn point_in_floor_triangle(point: Vector3, vertices: [Vector3; 3]) -> bool {
    let cross =
        |a: Vector3, b: Vector3, p: Vector3| (b.x - a.x) * (p.z - a.z) - (b.z - a.z) * (p.x - a.x);
    let winding = cross(vertices[0], vertices[1], vertices[2]).signum();
    (0..3).all(|i| {
        let a = vertices[i];
        let b = vertices[(i + 1) % 3];
        winding * cross(a, b, point) >= -0.001 * (b.x - a.x).hypot(b.z - a.z)
    })
}

fn retention_record(id: CollisionBody, contact: RetailContactInput) -> ContactRecord {
    let mut record = [0; 64];
    for (offset, v) in [
        (0, contact.position_on_a),
        (4, contact.position_on_b),
        (8, contact.normal),
    ] {
        record[offset..offset + 3].copy_from_slice(&[v.x, v.y, v.z].map(f32::to_bits));
    }
    record[3] = id.contact_id();
    record[7] = u32::MAX;
    record[11] = contact.restitution.to_bits();
    record[15] = contact.static_friction.to_bits();
    record[19] = contact.dynamic_friction.to_bits();
    record[23] = contact.tag;
    record
}

fn collision_from_record(record: &ContactRecord) -> BoardCollision {
    let f = |i| f32::from_bits(record[i]);
    let vector = |i| Vector3::new(f(i), f(i + 1), f(i + 2));
    BoardCollision {
        body_a: CollisionBody::from_contact_id(record[3]),
        body_b: CollisionBody::StaticWorld,
        contact: RetailContactInput {
            position_on_a: vector(0),
            position_on_b: vector(4),
            normal: vector(8),
            restitution: f(11),
            static_friction: f(15),
            dynamic_friction: f(19),
            tag: record[23],
        },
    }
}

