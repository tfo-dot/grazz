use grazz::game::GameState;

use bytemuck::{Pod, Zeroable};
use grazz::ipc;
use rand::RngExt;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use wgpu::util::DeviceExt;

use layershellev::id::Id;
use layershellev::reexport::{Anchor, Layer};
use layershellev::{DispatchMessage, LayerShellEvent, RefreshRequest, ReturnData, WindowState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Instance {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub sway_phase: f32,
}

impl Instance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Uniforms {
    pub time: f32,
    pub mower_x: f32,
    pub grow_factor: f32,
    pub _padding: f32,
}

const GRASS_VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.02, 0.0],
    },
    Vertex {
        position: [0.02, 0.0],
    },
    Vertex {
        position: [0.00, 1.0],
    },
];

pub struct State<'a> {
    pub surface: wgpu::Surface<'a>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: (u32, u32),
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    num_instances: u32,
    start_time: std::time::Instant,
    pub mower_x: f32,
    pub growth_factor: f32,
    pub is_mowing: bool,
    pub trigger_mower: Arc<AtomicU32>,
    pub local_gen: u32,
    pub game_state: Arc<Mutex<GameState>>,
}

impl<'a> State<'a> {
    pub async fn new(
        display_handle: raw_window_handle::RawDisplayHandle,
        window_handle: raw_window_handle::RawWindowHandle,
        size: (u32, u32),
        mower_running: Arc<AtomicU32>,
        game_state: Arc<Mutex<GameState>>,
    ) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: display_handle,
                    raw_window_handle: window_handle,
                })
                .expect("Failed to create surface")
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mut rng = rand::rng();
        let num_instances = 2_000;
        let instances: Vec<Instance> = (0..num_instances)
            .map(|_| {
                let x = rng.random_range(-1.05..1.05);
                let y_offset = rng.random_range(-1.1..-0.95);
                let scale_x = rng.random_range(0.5..1.2);
                let scale_y = rng.random_range(0.4..1.0);

                Instance {
                    offset: [x, y_offset],
                    scale: [scale_x, scale_y],
                    sway_phase: rng.random_range(0.0..std::f32::consts::TAU), // 0 to 2*PI
                }
            })
            .collect();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(GRASS_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms {
                time: 0.0,
                mower_x: -2.0,
                grow_factor: 0.0,
                _padding: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Grass Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("grass.wgsl").into()),
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc(), Instance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            instance_buffer,
            uniform_buffer,
            uniform_bind_group,
            num_instances: num_instances as u32,
            start_time: std::time::Instant::now(),
            mower_x: -2.0,
            growth_factor: 0.2,
            is_mowing: false,
            local_gen: 0,
            trigger_mower: mower_running,
            game_state,
        }
    }

    pub fn configure_surface(&mut self, size: (u32, u32)) {
        self.size = size;
        self.config.width = size.0;
        self.config.height = size.1;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn update(&mut self) {
        let elapsed = self.start_time.elapsed().as_secs_f32();

        let global_gen = self.trigger_mower.load(Ordering::Relaxed);
        if global_gen > self.local_gen {
            self.is_mowing = true;
            self.mower_x = -1.5;
            self.local_gen = global_gen;
        }

        if let Ok(mut gs) = self.game_state.lock() {
            if self.is_mowing {
                self.mower_x += 0.03 * gs.mower_level as f32;

                if self.mower_x > 1.5 {
                    self.is_mowing = false;
                    self.mower_x = -2.0;

                    let cut_amount = (self.growth_factor - 0.2) * self.num_instances as f32;

                    gs.total_grass_cut += cut_amount;
                    gs.money += cut_amount * 0.01 * gs.money_level as f32;
                    gs.save();
                    println!("Screen Mowed! Total Money: ${:.2}", gs.money);

                    self.growth_factor = 0.2;
                }
            } else {
                if self.growth_factor < 1.0 {
                    self.growth_factor += 0.0002 * gs.fertilizer_level as f32;
                }
            }
        }

        let uniforms = Uniforms {
            time: elapsed,
            mower_x: self.mower_x,
            grow_factor: self.growth_factor,
            _padding: 0.0,
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

            render_pass.draw(0..3, 0..self.num_instances);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn main() {
    let game_state = Arc::new(Mutex::new(GameState::load()));
    let start_mowing = Arc::new(AtomicU32::new(0));

    let ipc_mowing_flag = start_mowing.clone();
    let ipc_game_state = game_state.clone();

    ipc::spawn_ipc(ipc_mowing_flag, ipc_game_state);

    let mut states: HashMap<Id, State> = HashMap::new();
    let mut last_update: HashMap<Id, std::time::Instant> = HashMap::new();

    let target_fps = 60.0;
    let frame_duration = std::time::Duration::from_secs_f32(1.0 / target_fps);

    let ev: WindowState<()> = WindowState::new("grazz")
        .with_allscreens()
        .with_anchor(Anchor::Bottom | Anchor::Left | Anchor::Right)
        .with_layer(Layer::Bottom)
        .with_size((0, 100))
        .with_use_display_handle(true)
        .with_exclusive_zone(50)
        .build()
        .unwrap();

    ev.running(move |event, ws, idx| match event {
        LayerShellEvent::InitRequest => ReturnData::RequestBind,
        LayerShellEvent::BindProvide(_globals, _qh) => {
            let display_handle = ws.display_handle().unwrap().clone().as_raw();

            for unit in ws.get_unit_iter() {
                let window_handle = unit.window_handle().unwrap().as_raw();
                let size = unit.get_size();

                if size.0 == 0 || size.1 == 0 {
                    continue;
                }

                let state_mowing_flag = start_mowing.clone();
                let state_game = game_state.clone();

                let state = pollster::block_on(State::new(
                    display_handle,
                    window_handle,
                    size,
                    state_mowing_flag,
                    state_game,
                ));
                states.insert(unit.id(), state);
            }

            ReturnData::RequestCompositor
        }
        LayerShellEvent::RequestMessages(DispatchMessage::RequestRefresh {
            width,
            height,
            scale_float: _,
            is_created: _,
        }) => {
            if let Some(id) = idx {
                if !states.contains_key(&id) && *width > 0 && *height > 0 {
                    let display_handle = ws.display_handle().unwrap().clone().as_raw();
                    let unit = ws.get_unit_with_id(id).unwrap();
                    let window_handle = unit.window_handle().unwrap().as_raw();

                    let state_mowing_flag = start_mowing.clone();
                    let state_game = game_state.clone();

                    let state = pollster::block_on(State::new(
                        display_handle,
                        window_handle,
                        (*width, *height),
                        state_mowing_flag,
                        state_game,
                    ));
                    states.insert(id, state);
                    last_update.insert(id, std::time::Instant::now());
                }

                if let Some(state) = states.get_mut(&id) {
                    if state.size != (*width, *height) {
                        state.configure_surface((*width, *height));
                    }

                    state.update();

                    if last_update.get(&id).expect("Timer missing").elapsed() >= frame_duration {
                        if let Ok(_) = state.render() {
                            last_update.insert(id, std::time::Instant::now());
                        }
                    }

                    ws.request_refresh(id, RefreshRequest::NextFrame);
                }
            }
            ReturnData::None
        }
        _ => ReturnData::None,
    })
    .unwrap();
}
