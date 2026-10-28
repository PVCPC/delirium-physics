use bevy::prelude::*;
use skate_core::{
    math::Vector3,
    physics::{
        board_world::{
            BoardWorld, WorldTriangle,
            query_metadata::{Bounds, QueryMesh, QueryMetadata, QueryPool},
        },
        collision::TriangleFeature,
        contact::RetailContactMaterial,
        drive_frames::RetailAffineTransform,
    },
};
use skate_data::skate_map::SkateMap;
use std::collections::HashMap;

pub(crate) fn validate_runtime(map: &SkateMap) -> Result<(), String> {
    let _span = info_span!("validate_map").entered();
    let archive = retail_archive(map)?;
    if map.geometry.collision.is_empty() && archive.is_none() {
        return Err("SKATE map has no collision geometry".into());
    }
    for triangle in map.geometry.collision.iter().filter(|_| archive.is_none()) {
        if let Some(edges) = triangle.native_edges {
            decode_native_edges(edges)?;
        }
    }
    if !map.doors.is_empty() {
        return Err(format!(
            "Map '{}' contains {} hinged doors. This imported game has no door body/controller adapter yet; refusing to drop their geometry or turn them into static walls.",
            map.name,
            map.doors.len()
        ));
    }
    for extension in &map.extensions {
        let tag = String::from_utf8_lossy(&extension.tag);
        if extension.tag == *b"RWCM" {
            continue;
        }
        if extension.tag == *b"MOBJ" {
            skate_data::skate_map::validate_static_objects(map, extension)?;
            continue;
        }
        if extension.tag == *b"SKYB" && extension.schema == 1 {
            eprintln!(
                "SKATE LIMITATION: SKYB retail sky retained; using the map horizon until its shader adapter is available."
            );
            continue;
        }
        if extension.tag != *b"WMET" && extension.tag != *b"WCFG" && extension.tag != *b"BMAT" {
            return Err(format!(
                "SKATE extension {tag} schema {} is decoded but its runtime adapter is not implemented. Refusing to silently omit potentially required world geometry.",
                extension.schema
            ));
        }
        eprintln!(
            "SKATE LIMITATION: {tag} extension retained; its runtime behavior is not connected."
        );
    }
    if map.materials.iter().any(|m| m.retail_definition.is_some()) {
        eprintln!(
            "SKATE_RENDER: retail world material adapter enabled; unsupported families retain portable PBR rendering."
        );
    }
    if map.textures.iter().any(|t| t.width == 0) {
        return Err(
            "SKATE contains external texture placeholders; supply a package with embedded textures"
                .into(),
        );
    }
    if !map.routes.is_empty() {
        eprintln!(
            "SKATE LIMITATION: {} NPC routes parsed; supplied game has no NPC controller.",
            map.routes.len()
        );
    }
    if map.lights.iter().any(|l| l.kind == 2) {
        eprintln!(
            "SKATE LIMITATION: area-light records retained; Bevy adapter currently renders point and spot lights only."
        );
    }
    eprintln!(
        "SKATE LIMITATION: native frame-lighting/day-night controller is not connected; using package lighting and retail district sky where available."
    );
    eprintln!(
        "SKATE_MAP_LOADED name={:?} version={} render_triangles={} collision_triangles={} textures={} spawn={:?}",
        map.name,
        map.version,
        map.geometry.indices.len() / 3,
        map.geometry.collision.len(),
        map.textures.len(),
        map.spawn
    );
    Ok(())
}

fn decode_native_edges(edges: [u8; 3]) -> Result<(u32, [f32; 3]), String> {
    let mut flags = 1 | TriangleFeature::ONE_SIDED | TriangleFeature::USE_EDGE_COSINES;
    let mut cosines = [0.; 3];
    for (i, code) in edges.into_iter().enumerate() {
        let exponent = code & 0x1f;
        if exponent >= 28 {
            return Err(format!(
                "Invalid SKATE native edge angle code {exponent} at corner {i}"
            ));
        }
        cosines[i] = 1.0 - f32::from_bits(0x411d_e9e7) / ((8_u32 << exponent) as f32);
        flags |= u32::from(code & 0x20) << i;
        flags |= u32::from(code & 0x40) << (i + 3);
    }
    Ok((flags, cosines))
}

fn retail_archive(map: &SkateMap) -> Result<Option<&[u8]>, String> {
    let mut archives = map.extensions.iter().filter(|e| e.tag == *b"RWCM");
    let Some(archive) = archives.next() else {
        return Ok(None);
    };
    if archive.schema != 1 || archives.next().is_some() {
        return Err("SKATE requires one RWCM extension with schema 1".into());
    }
    Ok(Some(&archive.payload))
}

fn retail_collision_world(
    archive: &[u8],
    material: RetailContactMaterial,
) -> Result<BoardWorld, String> {
    let mut triangles = Vec::new();
    let mut packed_surfaces = Vec::new();
    let mut meshes = Vec::new();
    let count = skate_data::retail_collision::visit_clusters(archive, |_, cluster| {
        let mut cursor = 0;
        while cursor < cluster.len() {
            let group = cluster[cursor].group;
            let start = triangles.len();
            while cursor < cluster.len() && cluster[cursor].group == group {
                let source = cluster[cursor];
                let (flags, cosines) = match source.edges {
                    Some(edges) => {
                        let (mut flags, cosines) = decode_native_edges(edges)?;
                        flags &= !TriangleFeature::ONE_SIDED;
                        if source.one_sided {
                            flags |= TriangleFeature::ONE_SIDED;
                        }
                        (flags, cosines)
                    }
                    None => (0x1e1, [-1.; 3]),
                };
                triangles.push(
                    WorldTriangle::from_vertices(
                        source.points.map(|p| Vector3::new(p[0], p[1], p[2])),
                        material,
                        u32::from(source.surface),
                        flags,
                        cosines,
                        0.,
                    )
                    .ok_or("Invalid RWCM collision triangle")?,
                );
                packed_surfaces.push(source.surface);
                cursor += 1;
            }
            let range = start..triangles.len();
            let bounds = Bounds::from_points(
                triangles[range.clone()]
                    .iter()
                    .flat_map(|t| t.triangle.vertices),
            )
            .ok_or("Invalid RWCM cluster bounds")?;
            meshes.push(QueryMesh {
                geometry: 0,
                rejection_flags: 0,
                triangle_range: range,
                local_to_world: RetailAffineTransform::IDENTITY,
                world_to_local: RetailAffineTransform::IDENTITY,
                local_bounds: bounds,
                matching_group: i32::from(group),
                pool: QueryPool::Ground,
            });
        }
        Ok(())
    })?;
    eprintln!(
        "SKATE_RWCM_READY triangles={count} query_clusters={} source=embedded",
        meshes.len()
    );
    BoardWorld::with_query_metadata(
        triangles,
        QueryMetadata {
            packed_surfaces,
            meshes,
            static_edges: vec![],
            island_flags: 0,
        },
    )
    .map_err(str::to_owned)
}

pub(crate) fn collision_world(
    map: &SkateMap,
    material: RetailContactMaterial,
) -> Result<BoardWorld, String> {
    if let Some(archive) = retail_archive(map)? {
        return retail_collision_world(archive, material);
    }
    let mut welded = HashMap::<[i64; 3], usize>::new();
    let mut positions = Vec::<Vec3>::new();
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    for tri in &map.geometry.collision {
        let ids = tri.points.map(|p| {
            let inverse = 1.0 / f64::from(0.001_f32);
            let key = p.map(|v| (f64::from(v) * inverse).round() as i64);
            *welded.entry(key).or_insert_with(|| {
                let id = positions.len();
                positions.push(Vec3::from_array(p));
                id
            })
        });
        let [a, b, c] = tri.points.map(Vec3::from_array);
        let normal = (b - a)
            .cross(c - a)
            .try_normalize()
            .ok_or("Invalid SKATE collision triangle normal")?;
        vertices.push(ids);
        normals.push(normal);
    }
    let mut cosines = vec![[1.; 3]; vertices.len()];
    let mut flags =
        vec![TriangleFeature::ONE_SIDED | TriangleFeature::USE_EDGE_COSINES | 0xe0; vertices.len()];
    if map
        .geometry
        .collision
        .iter()
        .any(|t| t.native_edges.is_none())
    {
        let mut edge_groups = HashMap::<(usize, usize), Vec<(usize, usize, usize, usize)>>::new();
        for (i, ids) in vertices.iter().enumerate() {
            for edge in 0..3 {
                let (a, b) = (ids[edge], ids[(edge + 1) % 3]);
                edge_groups
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .push((i, edge, a, b));
            }
        }
        for group in edge_groups.values() {
            for &(i, edge, a, b) in group {
                let Some(&(other, _, _, _)) = group
                    .iter()
                    .filter(|&&(candidate, _, ca, cb)| candidate != i && ca == b && cb == a)
                    .max_by(|&&(left, _, _, _), &&(right, _, _, _)| {
                        normals[i]
                            .dot(normals[left])
                            .total_cmp(&normals[i].dot(normals[right]))
                    })
                else {
                    continue;
                };
                let cosine = normals[i].dot(normals[other]).clamp(-1., 1.);
                let orientation =
                    (positions[b] - positions[a]).dot(normals[i].cross(normals[other]));
                cosines[i][edge] = cosine;
                if orientation <= -1.0e-6 || cosine >= 1. - 1.0e-5 {
                    flags[i] &= !(0x20 << edge);
                }
            }
        }
        let mut adjacent = vec![Vec::new(); positions.len()];
        for (i, ids) in vertices.iter().enumerate() {
            for &v in ids {
                adjacent[v].push(i);
            }
        }
        for (v, faces) in adjacent.iter().enumerate() {
            let reference = normals[faces[0]];
            if faces
                .iter()
                .all(|&i| (reference.dot(normals[i]) - 1.).abs() <= 0.01)
            {
                for &i in faces {
                    for corner in 0..3 {
                        if vertices[i][corner] == v {
                            flags[i] |= 0x200 << corner;
                        }
                    }
                }
            }
        }
    }
    let mut triangles = Vec::with_capacity(vertices.len());
    let mut packed_surfaces = Vec::with_capacity(vertices.len());
    for (i, source) in map.geometry.collision.iter().enumerate() {
        if let Some(edges) = source.native_edges {
            (flags[i], cosines[i]) = decode_native_edges(edges)?;
        }
        let m = &map.materials[source.material as usize - 1];
        packed_surfaces.push((m.audio | (m.physics << 7) | (m.pattern << 12)) as u16);
        let points = source.points.map(|p| Vector3::new(p[0], p[1], p[2]));
        triangles.push(
            WorldTriangle::from_vertices(
                points,
                material,
                source.surface,
                flags[i],
                cosines[i],
                0.,
            )
            .ok_or_else(|| {
                format!(
                    "Invalid SKATE collision volume at triangle {i}: {:?}",
                    source.points
                )
            })?,
        );
    }
    let mut meshes = Vec::new();
    for start in (0..triangles.len()).step_by(64) {
        let end = (start + 64).min(triangles.len());
        let bounds = Bounds::from_points(
            triangles[start..end]
                .iter()
                .flat_map(|t| t.triangle.vertices),
        )
        .ok_or("SKATE collision bounds empty")?;
        meshes.push(QueryMesh {
            geometry: 0,
            rejection_flags: 0,
            triangle_range: start..end,
            local_to_world: RetailAffineTransform::IDENTITY,
            world_to_local: RetailAffineTransform::IDENTITY,
            local_bounds: bounds,
            matching_group: -1,
            pool: QueryPool::Ground,
        });
    }
    let metadata = QueryMetadata {
        packed_surfaces,
        meshes,
        static_edges: vec![],
        island_flags: 0,
    };
    let mut world = BoardWorld::with_query_metadata(triangles, metadata).map_err(str::to_owned)?;
    if map.name == "IW4L collision" {
        world.enable_imported_floor_seams();
    }
    Ok(world)
}
