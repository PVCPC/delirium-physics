use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use super::Frame;

const CLIP_OVERLAP: f32 = -0.0254;
const SHADER: Handle<Shader> = bevy::asset::uuid_handle!("4c3d5f10-6b27-4e94-a1d8-2f7e9c4b6a13");
const DISABLED: Vec4 = Vec4::new(0.0, 0.0, 0.0, 1.0);

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

pub fn exit_planes(exit: &Frame) -> ClipPlanes {
    let n = exit.normal();
    ClipPlanes { planes: [n.extend(-n.dot(exit.at) + CLIP_OVERLAP), DISABLED, DISABLED, DISABLED] }
}

pub fn plugin(app: &mut App) {
    let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(&SHADER, Shader::from_wgsl(include_str!("../ghost_clip.wgsl"), "ghost_clip.wgsl"));
    app.add_plugins(MaterialPlugin::<GhostMaterial>::default());
}
