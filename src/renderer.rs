use crate::app::App;

use std::{
    borrow::{BorrowMut, Cow},
    collections::HashMap,
    sync::{Arc, Mutex},
};

use color_eyre::eyre::{eyre, Result};
use dashmap::DashMap;
use glam::{vec3, Vec3};
use wgpu::{util::DeviceExt, RenderPipeline};
use winit::{
    dpi::PhysicalSize,
    window::{Window, WindowId},
};

const MSAA_SAMPLES: u32 = 1;
const FORMAT_INDEX: usize = 0;
const ALPHA_MODES_INDEX: usize = 0;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: Vec3,
    // color: [f32; 4],
}

impl Vertex {
    fn new(position: Vec3) -> Self {
        Self { position }
    }
}

impl Vertex {
    const ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![
        0 => Float32x3,
        // 1 => Float32x3,
    ];

    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}

struct ViewportDesc {
    window: WindowId,
    background: wgpu::Color,
    surface: wgpu::Surface,
}

struct Viewport {
    desc: ViewportDesc,
    config: wgpu::SurfaceConfiguration,
    render_target: Option<wgpu::Texture>,
}

impl ViewportDesc {
    fn new(window: &Window, background: wgpu::Color, instance: &wgpu::Instance) -> Self {
        let surface = unsafe { instance.create_surface(window) }.unwrap();
        Self {
            window: window.id(),
            background,
            surface,
        }
    }

    fn create_target_texture(
        &self,
        device: &wgpu::Device,
        size: PhysicalSize<u32>,
        format: wgpu::TextureFormat,
    ) -> Option<wgpu::Texture> {
        if MSAA_SAMPLES == 1 {
            return None;
        }
        let limits = wgpu::Limits::default();
        let max_dim = limits.max_texture_dimension_3d;
        if size.width > max_dim || size.height > max_dim {
            return None;
        }
        Some(device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render target"),
            size: wgpu::Extent3d {
                width: size.width.min(limits.max_texture_dimension_3d),
                height: size.height.min(limits.max_texture_dimension_3d),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: MSAA_SAMPLES,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        }))
    }

    fn build(
        self,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        size: PhysicalSize<u32>,
    ) -> Viewport {
        let caps = self.surface.get_capabilities(adapter);
        let format = caps.formats[FORMAT_INDEX];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[ALPHA_MODES_INDEX],
            view_formats: vec![],
        };
        let render_target = self.create_target_texture(device, size, format);

        self.surface.configure(device, &config);

        Viewport {
            desc: self,
            config,
            render_target,
        }
    }
}

impl Viewport {
    fn resize(&mut self, device: &wgpu::Device, size: winit::dpi::PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.desc.surface.configure(device, &self.config);
        let next_target = self
            .desc
            .create_target_texture(device, size, self.config.format);
        let old = std::mem::replace(&mut self.render_target, next_target);
        if let Some(old) = old {
            old.destroy();
        }
    }

    fn get_current_texture(&mut self) -> wgpu::SurfaceTexture {
        self.desc
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture")
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    viewports: HashMap<WindowId, Viewport>,
    render_pipeline: RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    egui_renderers: HashMap<WindowId, egui_wgpu::Renderer>,
    egui_painters: HashMap<WindowId, egui_wgpu::winit::Painter>,
    // pub egui_contexts: HashMap<WindowId, egui::Context>,
    egui_contexts: Arc<DashMap<WindowId, egui::Context>>,
}

impl Renderer {
    pub async fn new(
        viewports: &mut [(&Window, wgpu::Color)],
        egui_contexts: Arc<DashMap<WindowId, egui::Context>>,
    ) -> Result<Self> {
        let instance = wgpu::Instance::default();
        let viewport_descriptions: Vec<_> = viewports
            .iter()
            .map(|(window, color)| ViewportDesc::new(window, *color, &instance))
            .collect();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                // Request an adapter which can render to our surface
                compatible_surface: viewport_descriptions.first().map(|desc| &desc.surface),
                ..Default::default()
            })
            .await
            .ok_or_else(|| eyre!("Failed to find an appropriate adapter"))?;

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::downlevel_defaults(),
                },
                None,
            )
            .await?;

        let viewport_map: HashMap<WindowId, Viewport> = viewports
            .iter()
            .zip(viewport_descriptions)
            .map(|((window, _color), desc)| {
                (
                    window.id(),
                    desc.build(&adapter, &device, window.inner_size()),
                )
            })
            .collect();

        let primary_viewport = viewport_map
            .values()
            .next()
            .expect("could not get first viewport");

        let mut egui_painters: HashMap<WindowId, egui_wgpu::winit::Painter> =
            HashMap::with_capacity(viewports.len());
        // for (window, _) in viewports.iter_mut() {
        //     let mut painter =
        //         egui_wgpu::winit::Painter::new(Default::default(), MSAA_SAMPLES, 0, false);
        //     unsafe { painter.set_window(Some(*window)) }
        //         .await
        //         .expect("could not set window for painter");
        //     egui_painters.insert(window.id(), painter);
        // }

        let egui_renderers = viewports
            .iter()
            .map(|(window, _)| {
                let egui_renderer = egui_wgpu::Renderer::new(
                    &device,
                    primary_viewport.config.format,
                    None,
                    1, // MSAA_SAMPLES,
                );
                (window.id(), egui_renderer)
            })
            .collect();

        let (render_pipeline, bind_group_layout, vertex_buffer, index_buffer, num_indices) =
            Self::create_pipeline_and_buffers(&device, &primary_viewport.config.format)?;

        Ok(Self {
            device,
            queue,
            viewports: viewport_map,
            render_pipeline,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
            num_indices,
            // platform,
            egui_renderers,
            egui_painters,
            egui_contexts,
        })
    }
    // async fn run(event_loop: EventLoop<()>, viewports: Vec<(Window, wgpu::Color)>) {

    pub fn reload(&mut self) -> Result<()> {
        let (_, viewport) = self
            .viewports
            .iter()
            .next()
            .ok_or_else(|| eyre!("failed to get viewport"))?;
        let (render_pipeline, bind_group_layout, vertex_buffer, index_buffer, num_indices) =
            Self::create_pipeline_and_buffers(&self.device, &viewport.config.format)?;
        self.render_pipeline = render_pipeline;
        self.bind_group_layout = bind_group_layout;
        self.vertex_buffer = vertex_buffer;
        self.index_buffer = index_buffer;
        self.num_indices = num_indices;
        Ok(())
    }

    pub fn resize(&mut self, window: &Window, size: PhysicalSize<u32>) {
        if let Some(viewport) = self.viewports.get_mut(&window.id()) {
            viewport.resize(&self.device, size);
            // On macos the window needs to be redrawn manually after resizing
            window.request_redraw();
        }
    }

    pub fn render(
        &mut self,
        app: &mut App,
        window: &Window,
        egui_state: Arc<Mutex<egui_winit::State>>,
    ) {
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        // Render the frame.
        if let Some(viewport) = self.viewports.get_mut(&window.id()) {
            let frame = viewport.get_current_texture();
            let target = &viewport.render_target;
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let target_view = target
                .as_ref()
                .map(|x| x.create_view(&wgpu::TextureViewDescriptor::default()));
            // let target_view = match target {
            //     Some(x) => Some(x.create_view(&wgpu::TextureViewDescriptor::default())),
            //     None => None,
            // };

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

            self.render_background(app, encoder, &view, target_view.as_ref());
            // frame.present();

            self.render_ui(window, egui_state, app, view);
            frame.present();
        }
    }

    fn render_ui(
        &mut self,
        window: &Window,
        egui_state: Arc<Mutex<egui_winit::State>>,
        app: &mut App,
        view: wgpu::TextureView,
    ) {
        // Update egui.
        let size = window.inner_size();
        let screen = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: 1.0,
        };
        let renderer = self.egui_renderers.get_mut(&window.id()).unwrap();
        // let painter = self
        //     .egui_painters
        //     .get_mut(&window.id())
        //     .expect("could not get painter for id");
        let egui_ctx = self
            .egui_contexts
            .get(&window.id())
            .expect("could not get egui context for id")
            .clone();

        let raw_input = egui_state.lock().unwrap().take_egui_input(window);
        let full_output = egui_ctx.run(raw_input, |ctx| app.ui(ctx));
        let clipped_primitives = egui_ctx.tessellate(full_output.shapes);
        // painter.paint_and_update_textures(
        //     1.0,
        //     [0.5, 0.5, 0.5, 0.5],
        //     &clipped_primitives,
        //     &full_output.textures_delta,
        // );
        for (id, image_delta) in &full_output.textures_delta.set {
            renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("egui Render Encoder"),
            });
        let egui_command_buffers = renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &clipped_primitives,
            &screen,
        );
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            renderer.render(&mut render_pass, &clipped_primitives, &screen);
        }
        for id in &full_output.textures_delta.free {
            renderer.free_texture(id);
        }
        self.queue.submit(
            egui_command_buffers
                .into_iter()
                .chain(Some(encoder.finish())),
        );
    }

    fn render_background(
        &mut self,
        app: &mut App,
        mut encoder: wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        target_view: Option<&wgpu::TextureView>,
    ) {
        // Render the triangle.
        let color = app.triangle_color;
        let color_array: [f32; 4] = [
            color.r() as f32 / 255.0,
            color.g() as f32 / 255.0,
            color.b() as f32 / 255.0,
            color.a() as f32 / 255.0,
        ];
        let bind_group = create_bind_group(&self.device, &self.bind_group_layout, &color_array);
        {
            let (view, resolve_target) = match target_view {
                Some(target) => (target, Some(view)),
                None => (view, None),
            };
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
    }

    fn create_pipeline_and_buffers(
        device: &wgpu::Device,
        swapchain_format: &wgpu::TextureFormat,
    ) -> Result<(
        RenderPipeline,
        wgpu::BindGroupLayout,
        wgpu::Buffer,
        wgpu::Buffer,
        u32,
    )> {
        // Create shaders.
        let shader = std::fs::read_to_string("src/shaders/triangle.wgsl")?;
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&shader)),
        });

        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&shader)),
        });

        // Create vertex and index buffers.
        let vertices: &[Vertex] = &[
            Vertex::new(vec3(-0.5, -0.5, 0.0)),
            Vertex::new(vec3(0.5, -0.5, 0.0)),
            Vertex::new(vec3(0.0, 0.5, 0.0)),
        ];
        let indices: &[u16] = &[0, 1, 2];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Color Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            }],
        });

        // Create render pipeline.
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let mut color_target = wgpu::ColorTargetState::from(*swapchain_format);
        color_target.blend = Some(wgpu::BlendState::ALPHA_BLENDING);

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader,
                entry_point: "fs_main",
                targets: &[Some(color_target)],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLES,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Ok((
            render_pipeline,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
            indices.len() as u32,
        ))
    }
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    color: &[f32; 4],
) -> wgpu::BindGroup {
    let color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Color Buffer"),
        contents: bytemuck::cast_slice(color),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Color Bind Group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: color_buffer.as_entire_binding(),
        }],
    })
}
