//! Ghost clipping, after `p2_body` in the Portal 2 Rust Engine: a `StandardMaterial` with up to
//! four world-space clip planes. A ghost copy of a body part is cut where it has not yet come out of the
//! exit portal, so the part of the mirrored body that falls behind the exit's wall (soles poking through a
//! floor, say) is never drawn. The plane sits `CLIP_OVERLAP` behind the opening, as in Portal 2.
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use super::Frame;

/// Portal 2 keeps a ghost drawing 2 Source units behind the exit plane so its two halves overlap. Here
/// the ghost's sole faces fall in that margin and show as flat slabs lying in the floor outside the
/// opening, so the plane sits a unit in front of the surface instead (the opening's surface hides the seam).
const CLIP_OVERLAP: f32 = -0.0254;
const SHADER: Handle<Shader> = bevy::asset::uuid_handle!("4c3d5f10-6b27-4e94-a1d8-2f7e9c4b6a13");
/// A plane that never clips (`dot(0, p) + 1 >= 0`).
const DISABLED: Vec4 = Vec4::new(0.0, 0.0, 0.0, 1.0);

/// Up to four world-space planes; a point `p` is discarded when `dot(plane.xyz, p) + plane.w < 0` for any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect, ShaderType)]
pub struct ClipPlanes {
    pub planes: [Vec4; 4],
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct ClipExt {
    #[uniform(100)]
    pub clip: ClipPlanes,
}

impl MaterialExtension for ClipExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub type GhostMaterial = ExtendedMaterial<StandardMaterial, ClipExt>;

/// The clip planes of a ghost that comes out of `exit`: keep only what is in front of it, give or take
/// the margin.
pub fn exit_planes(exit: &Frame) -> ClipPlanes {
    let n = exit.normal();
    ClipPlanes { planes: [n.extend(-n.dot(exit.at) + CLIP_OVERLAP), DISABLED, DISABLED, DISABLED] }
}

pub fn plugin(app: &mut App) {
    let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(&SHADER, Shader::from_wgsl(include_str!("../ghost_clip.wgsl"), "ghost_clip.wgsl"));
    app.add_plugins(MaterialPlugin::<GhostMaterial>::default());
}
