use bevy_app::{App, Plugin};
use bevy_color::{ColorToComponents, LinearRgba};
use bevy_ecs::prelude::*;
use bevy_asset::Handle;
use bevy_image::Image;
use bevy_math::{Vec2, Vec3, Vec4};
use bevy_render::{
    extract_component::ExtractComponentPlugin,
    extract_resource::{ExtractResource, ExtractResourcePlugin},
    render_asset::RenderAssets,
    render_resource::{DynamicUniformBuffer, ShaderType, UniformBuffer},
    texture::GpuImage,
    renderer::{RenderDevice, RenderQueue},
    view::ExtractedView,
    GpuResourceAppExt, Render, RenderApp, RenderSystems,
};
use bevy_shader::load_shader_library;

use crate::{DistanceFog, FogFalloff};

/// The GPU-side representation of the fog configuration that's sent as a uniform to the shader
#[derive(Copy, Clone, ShaderType, Default, Debug)]
pub struct GpuFog {
    /// Fog color
    base_color: Vec4,
    /// The color used for the fog where the view direction aligns with directional lights
    directional_light_color: Vec4,
    /// Allocated differently depending on fog mode.
    /// See `mesh_view_types.wesl` for a detailed explanation
    be: Vec3,
    /// The exponent applied to the directional light alignment calculation
    directional_light_exponent: f32,
    /// Allocated differently depending on fog mode.
    /// See `mesh_view_types.wesl` for a detailed explanation
    bi: Vec3,
    /// Unsigned int representation of the active fog falloff mode
    mode: u32,
    /// Delirium: `DistanceFog::surface`.
    surface: f32,
}

// Important: These must be kept in sync with `mesh_view_types.wesl`
#[expect(unused, reason = "Kept in sync with `mesh_view_types.wesl`")]
const GPU_FOG_MODE_OFF: u32 = 0;
const GPU_FOG_MODE_LINEAR: u32 = 1;
const GPU_FOG_MODE_EXPONENTIAL: u32 = 2;
const GPU_FOG_MODE_EXPONENTIAL_SQUARED: u32 = 3;
const GPU_FOG_MODE_ATMOSPHERIC: u32 = 4;

/// Metadata for fog
#[derive(Default, Resource)]
pub struct FogMeta {
    pub gpu_fogs: DynamicUniformBuffer<GpuFog>,
}

/// Prepares fog metadata and writes the fog-related uniform buffers to the GPU
pub fn prepare_fog(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut fog_meta: ResMut<FogMeta>,
    views: Query<(Entity, &DistanceFog), With<ExtractedView>>,
) {
    let views_iter = views.iter();
    let view_count = views_iter.len();
    let Some(mut writer) = fog_meta
        .gpu_fogs
        .get_writer(view_count, &render_device, &render_queue)
    else {
        return;
    };
    for (entity, fog) in views_iter {
        let gpu_fog = {
            match &fog.falloff {
                FogFalloff::Linear { start, end } => GpuFog {
                    mode: GPU_FOG_MODE_LINEAR,
                    surface: fog.surface,
                    base_color: LinearRgba::from(fog.color).to_vec4(),
                    directional_light_color: LinearRgba::from(fog.directional_light_color)
                        .to_vec4(),
                    directional_light_exponent: fog.directional_light_exponent,
                    be: Vec3::new(*start, *end, 0.0),
                    ..Default::default()
                },
                FogFalloff::Exponential { density } => GpuFog {
                    mode: GPU_FOG_MODE_EXPONENTIAL,
                    surface: fog.surface,
                    base_color: LinearRgba::from(fog.color).to_vec4(),
                    directional_light_color: LinearRgba::from(fog.directional_light_color)
                        .to_vec4(),
                    directional_light_exponent: fog.directional_light_exponent,
                    be: Vec3::new(*density, 0.0, 0.0),
                    ..Default::default()
                },
                FogFalloff::ExponentialSquared { density } => GpuFog {
                    mode: GPU_FOG_MODE_EXPONENTIAL_SQUARED,
                    surface: fog.surface,
                    base_color: LinearRgba::from(fog.color).to_vec4(),
                    directional_light_color: LinearRgba::from(fog.directional_light_color)
                        .to_vec4(),
                    directional_light_exponent: fog.directional_light_exponent,
                    be: Vec3::new(*density, 0.0, 0.0),
                    ..Default::default()
                },
                FogFalloff::Atmospheric {
                    extinction,
                    inscattering,
                } => GpuFog {
                    mode: GPU_FOG_MODE_ATMOSPHERIC,
                    base_color: LinearRgba::from(fog.color).to_vec4(),
                    directional_light_color: LinearRgba::from(fog.directional_light_color)
                        .to_vec4(),
                    directional_light_exponent: fog.directional_light_exponent,
                    be: *extinction,
                    bi: *inscattering,
                    surface: fog.surface,
                },
            }
        };

        // This is later read by `SetMeshViewBindGroup<I>`
        commands.entity(entity).insert(ViewFogUniformOffset {
            offset: writer.write(&gpu_fog),
        });
    }
}

/// Inserted on each `Entity` with an `ExtractedView` to keep track of its offset
/// in the `gpu_fogs` `DynamicUniformBuffer` within `FogMeta`
#[derive(Component)]
pub struct ViewFogUniformOffset {
    pub offset: u32,
}

/// A plugin that consolidates fog extraction, preparation and related resources/assets
pub struct FogPlugin;

/// Delirium: the caustics (the sun's light focused by a body of water's ripples) in flat layers down through it, on what's
/// in it (the standard material's directional lights under its surface).
#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct WaterCaustics {
    /// The layers stacked down one image, in red a quarter of the light over flat water's (none: off).
    pub image: Option<Handle<Image>>,
    /// The layers' corners (row 0 along `max.y`).
    pub min: Vec2,
    pub max: Vec2,
    /// The surface's height, the last layer's depth under it, how many layers.
    pub surface: f32,
    pub deep: f32,
    pub layers: u32,
}

#[derive(Copy, Clone, ShaderType, Default, Debug)]
pub struct GpuWaterCaustics {
    min: Vec2,
    max: Vec2,
    surface: f32,
    deep: f32,
    layers: f32,
    on: f32,
}

#[derive(Resource, Default)]
pub struct WaterCausticsBuffer(pub UniformBuffer<GpuWaterCaustics>);

fn prepare_water_caustics(
    c: Option<Res<WaterCaustics>>,
    images: Res<RenderAssets<GpuImage>>,
    mut buffer: ResMut<WaterCausticsBuffer>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let g = match c.as_deref() {
        Some(c) if c.image.as_ref().is_some_and(|h| images.get(h).is_some()) => GpuWaterCaustics {
            min: c.min,
            max: c.max,
            surface: c.surface,
            deep: c.deep,
            layers: c.layers as f32,
            on: 1.0,
        },
        _ => GpuWaterCaustics::default(),
    };
    buffer.0.set(g);
    buffer.0.write_buffer(&render_device, &render_queue);
}

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "fog.wesl");

        app.add_plugins((
            ExtractComponentPlugin::<DistanceFog>::default(),
            ExtractResourcePlugin::<WaterCaustics>::default(),
        ))
        .init_resource::<WaterCaustics>();

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_gpu_resource::<FogMeta>()
                .init_resource::<WaterCausticsBuffer>()
                .add_systems(
                    Render,
                    (prepare_fog, prepare_water_caustics).in_set(RenderSystems::PrepareResources),
                );
        }
    }
}
