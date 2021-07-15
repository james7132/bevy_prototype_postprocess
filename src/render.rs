use crate::components::*;
use bevy::{
    asset::{AssetServer, Handle},
    ecs::{prelude::*, system::SystemState},
    math::*,
    pbr2::{ExtractedMeshes, PbrShaders},
    render2::{
        camera::{ActiveCameras, CameraPlugin},
        color::Color,
        render_asset::RenderAssets,
        render_graph::{Node, NodeRunError, RenderGraphContext, SlotInfo, SlotType},
        render_phase::{Draw, DrawFunctions, Drawable, RenderPhase, TrackedRenderPass},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        shader::Shader,
        texture::*,
        view::{ExtractedView, ViewMeta, ViewUniformOffset},
    },
};
use crevice::std140::AsStd140;
use std::num::NonZeroU32;

bitflags::bitflags!{
    #[derive(AsStd140)]
    pub struct UberFlags: u32 {
        const BLOOM = 1 << 0;
        const NORMAL_TONEMAPPING = 1 << 1;
        const ACES_TONEMAPPING = 1 << 2;
        const CHANNEL_MIXING = 1 << 3;
    }
}

#[derive(Debug, Clone, AsStd140)]
pub struct UberUniform {
    flags: UberFlags,
    bloom: UberBloom,
    channel_mixing: UberChannelMixing,
}

#[derive(Debug, Clone, Default, AsStd140)]
struct UberBloom {
    threshold: f32,
    intensity: f32,
    scatter: f32,
    tint: Vec4,
    clamp: f32,
}

impl From<Bloom> for UberBloom {
    fn from(value: Bloom) -> Self {
        Self {
            threshold: value.threshold,
            intensity: value.intensity,
            scatter: value.scatter,
            tint: Vec4::from(value.tint),
            clamp: value.clamp,
        }
    }
}

#[derive(Debug, Clone, Default, AsStd140)]
struct UberChannelMixing {
    matrix: Mat3,
}

impl From<ChannelMixing> for UberChannelMixing {
    fn from(value: ChannelMixing) -> Self {
        Self {
            matrix: value.into(),
        }
    }
}

pub struct UberEffectShaders {
    pipeline: RenderPipeline,
    view_layout: BindGroupLayout,
    sampler: Sampler,
}

// TODO: this pattern for initializing the shaders / pipeline isn't ideal. this should be handled by the asset system
impl FromWorld for UberEffectShaders {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let uber_shader = Shader::from_wgsl(include_str!("uber.wgsl"));
        let uber_shader_module = render_device.create_shader_module(&uber_shader);

        let view_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStage::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(std::mem::size_of::<UberUniform>() as u64),
                    },
                    count: None,
                },
            ],
            label: None,
        });

        let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            push_constant_ranges: &[],
            bind_group_layouts: &[&view_layout],
        });

        let pipeline = render_device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            vertex: VertexState {
                buffers: &[],
                module: &uber_shader_module,
                entry_point: "vertex",
            },
            fragment: Some(FragmentState {
                module: &uber_shader_module,
                entry_point: "fragment",
                targets: &[ColorTargetState {
                    format: TextureFormat::R8Unorm,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::Src,
                            dst_factor: BlendFactor::OneMinusSrc,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrite::ALL,
                }],
            }),
            depth_stencil: None,
            layout: Some(&pipeline_layout),
            multisample: MultisampleState::default(),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                clamp_depth: false,
                conservative: false,
            },
        });

        UberEffectShaders {
            pipeline,
            view_layout,
            sampler: render_device.create_sampler(&SamplerDescriptor::default()),
        }
    }
}

type ExtractedUberUniform = UberUniform;

pub fn extract_uber(
    mut commands: Commands,
    active_cameras: Res<ActiveCameras>,
    uber_config: Res<UberUniform>,
) {
    if let Some(camera_3d) = active_cameras.get(CameraPlugin::CAMERA_3D) {
        if let Some(entity) = camera_3d.entity {
            commands
                .get_or_spawn(entity)
                .insert(RenderPhase::<UberPhase>::default());
        }
    }
    commands.insert_resource::<ExtractedUberUniform>(uber_config.clone());
}

#[derive(Default)]
pub struct UberMeta {
    pub uniform: UniformVec<UberUniform>,
}

pub struct ViewUber {
    pub view_uber_texture: Texture,
    pub view_uber_texture_view: TextureView,
}

pub fn prepare_uber(
    mut commands: Commands,
    extracted_uber_config: Res<ExtractedUberUniform>,
    mut uber_meta: ResMut<UberMeta>,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ExtractedView), With<RenderPhase<UberPhase>>>,
) {
    uber_meta.uniform.reserve_and_clear(1, &render_device);
    uber_meta.uniform.push(extracted_uber_config.clone().into());

    // set up uber for each view
    for (entity, view) in views.iter() {
        let view_uber_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: view.width,
                    height: view.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R8Unorm,
                usage: TextureUsage::RENDER_ATTACHMENT | TextureUsage::SAMPLED,
            },
        );
        let view_uber_texture_view =
            view_uber_texture
                .texture
                .create_view(&TextureViewDescriptor {
                    label: None,
                    format: None,
                    dimension: Some(TextureViewDimension::D2),
                    aspect: TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: NonZeroU32::new(1),
                });
        commands.entity(entity).insert(ViewUber {
            view_uber_texture: view_uber_texture.texture,
            view_uber_texture_view,
        });
    }

    uber_meta.uniform.write_to_staging_buffer(&render_device);
}

pub struct UberViewBindGroup {
    view_bind_group: BindGroup,
}

pub struct UberConfigBindGroup {
    uber_config_bind_group: BindGroup,
}

pub fn queue_meshes(
    mut commands: Commands,
    draw_functions: Res<DrawFunctions>,
    render_device: Res<RenderDevice>,
    uber_shaders: Res<UberEffectShaders>,
    _pbr_shaders: Res<PbrShaders>,
    view_meta: Res<ViewMeta>,
    uber_meta: Res<UberMeta>,
    _extracted_uber_config: Res<ExtractedUberUniform>,
    _gpu_images: Res<RenderAssets<Image>>,
    mut views: Query<(Entity, &mut RenderPhase<UberPhase>)>,
) {
    if view_meta.uniforms.len() < 1 {
        return;
    }

    let uber_config_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
        entries: &[BindGroupEntry {
            binding: 0,
            resource: uber_meta.uniform.binding(),
        }],
        label: None,
        layout: &uber_shaders.view_layout,
    });

    commands.insert_resource(UberConfigBindGroup {
        uber_config_bind_group,
    });

    for (i, (entity, mut uber_phase)) in views.iter_mut().enumerate() {
        // TODO: cache this?
        let view_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: view_meta.uniforms.binding(),
                },
            ],
            label: None,
            layout: &uber_shaders.view_layout,
        });

        commands
            .entity(entity)
            .insert(UberViewBindGroup { view_bind_group });

        let draw_uber = draw_functions.read().get_id::<DrawUber>().unwrap();
        uber_phase.add(Drawable {
            draw_function: draw_uber,
            draw_key: i,
            sort_key: 0,
        });
    }
}

pub struct UberPhase;

pub struct UberPassNode {
    main_view_query: QueryState<(&'static ViewUber, &'static RenderPhase<UberPhase>)>,
}

impl UberPassNode {
    pub const IN_VIEW: &'static str = "view";

    pub fn new(world: &mut World) -> Self {
        Self {
            main_view_query: QueryState::new(world),
        }
    }
}

impl Node for UberPassNode {
    fn input(&self) -> Vec<SlotInfo> {
        vec![SlotInfo::new(Self::IN_VIEW, SlotType::Entity)]
    }

    fn update(&mut self, world: &mut World) {
        self.main_view_query.update_archetypes(world);
    }

    fn run(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let uber_meta = world.get_resource::<UberMeta>().unwrap();
        uber_meta
            .uniform
            .write_to_uniform_buffer(&mut render_context.command_encoder);

        let view_entity = graph.get_input_entity(Self::IN_VIEW)?;
        if let Some((view_uber, uber_phase)) =
            self.main_view_query.get_manual(world, view_entity).ok()
        {
            let pass_descriptor = RenderPassDescriptor {
                label: Some("uber"),
                color_attachments: &[RenderPassColorAttachment {
                    view: &view_uber.view_uber_texture_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK.into()),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            };

            let draw_functions = world.get_resource::<DrawFunctions>().unwrap();

            let render_pass = render_context
                .command_encoder
                .begin_render_pass(&pass_descriptor);
            let mut draw_functions = draw_functions.write();
            let mut tracked_pass = TrackedRenderPass::new(render_pass);
            for drawable in uber_phase.drawn_things.iter() {
                let draw_function = draw_functions.get_mut(drawable.draw_function).unwrap();
                draw_function.draw(
                    world,
                    &mut tracked_pass,
                    view_entity,
                    drawable.draw_key,
                    drawable.sort_key,
                );
            }
        }

        Ok(())
    }
}

type DrawUberParams<'s, 'w> = (
    Res<'w, UberEffectShaders>,
    Res<'w, UberConfigBindGroup>,
    Query<'w, 's, (&'w ViewUniformOffset, &'w UberViewBindGroup)>,
);

pub struct DrawUber {
    params: SystemState<DrawUberParams<'static, 'static>>,
}

impl DrawUber {
    pub fn new(world: &mut World) -> Self {
        Self {
            params: SystemState::new(world),
        }
    }
}

impl Draw for DrawUber {
    fn draw<'w>(
        &mut self,
        world: &'w World,
        pass: &mut TrackedRenderPass<'w>,
        view: Entity,
        _draw_key: usize,
        _sort_key: usize,
    ) {
        let (uber_shaders, uber_config_bind_group, views) = self.params.get(world);
        let (view_uniform_offset, uber_view_bind_group) = views.get(view).unwrap();
        pass.set_render_pipeline(&uber_shaders.into_inner().pipeline);
        pass.set_bind_group(
            0,
            &uber_view_bind_group.view_bind_group,
            &[view_uniform_offset.offset],
        );

        pass.set_bind_group(
            1,
            &uber_config_bind_group.into_inner().uber_config_bind_group,
            &[],
        );

        pass.draw(0..3, 0..1);
    }
}
