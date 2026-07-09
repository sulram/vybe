//! THE TWEAK PANEL — the first Overlay front-end: one slider per knob the
//! sketch PICKED with `tune(..)`, and nothing else. It talks to the core
//! through exactly two seams — the `Overlay` trait (events + paint-over) and
//! the tune registry — and the core never learns what egui is.
//!
//! egui (pure math) and egui-winit (event translation) come from crates; the
//! RENDERER is ours, ~150 lines against our own wgpu — because egui-wgpu pins
//! wgpu 29 while the core rides 30. Sovereign base, literally: egui's paint
//! contract is just textured triangles + scissor rects. Swap this for
//! egui-wgpu if it ever catches up and earns its keep.
//!
//! Zero cost when the feature is off; never compiled into default builds.
//! When the `tekne` workspace exists, this module is the seed of the
//! `vybe-tweak` addon crate.

use std::collections::HashMap;

use winit::window::Window;

use crate::gpu::{Overlay, OverlayFrame};
use crate::tune;

// ---------------------------------------------------------------------------
// Panel — egui context + input translation + our renderer, behind Overlay.
// ---------------------------------------------------------------------------

pub(crate) struct Panel {
    ctx: egui::Context,
    input: egui_winit::State,
    renderer: PanelRenderer,
}

impl Panel {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat, window: &Window) -> Self {
        let ctx = egui::Context::default();
        let input = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        Self {
            ctx,
            input,
            renderer: PanelRenderer::new(device, format),
        }
    }
}

impl Overlay for Panel {
    fn event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.input.on_window_event(window, event).consumed
    }

    fn frame(&mut self, f: OverlayFrame<'_>) {
        let raw = self.input.take_egui_input(f.window);
        let output = self.ctx.run_ui(raw, tunes_ui);
        self.input
            .handle_platform_output(f.window, output.platform_output);

        let ppp = self.ctx.pixels_per_point();
        let primitives = self.ctx.tessellate(output.shapes, ppp);
        self.renderer.paint(
            f.device,
            f.queue,
            f.encoder,
            f.view,
            f.size_px,
            ppp,
            &primitives,
            &output.textures_delta,
        );
    }
}

/// One slider per picked knob — the whole UI. `root` is egui's transparent
/// full-viewport layer; we only float a small window on top of it.
fn tunes_ui(root: &mut egui::Ui) {
    egui::Window::new("tunes")
        .default_width(220.0)
        .show(root.ctx(), |ui| {
            let mut changed = false;
            tune::edit(|entries| {
                for e in entries {
                    changed |= ui
                        .add(egui::Slider::new(&mut e.value, e.min..=e.max).text(&e.name))
                        .changed();
                }
            });
            if changed {
                tune::mark_dirty(); // the shell re-describes the sketch next frame
            }
        });
}

// ---------------------------------------------------------------------------
// PanelRenderer — our egui backend: meshes -> one pipeline, scissored draws.
// ---------------------------------------------------------------------------

struct PanelRenderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: HashMap<egui::TextureId, (wgpu::Texture, wgpu::BindGroup)>,
    vertices: SizedBuffer,
    indices: SizedBuffer,
}

/// A growable buffer: recreated when a frame needs more room.
struct SizedBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    usage: wgpu::BufferUsages,
}

impl SizedBuffer {
    fn new(device: &wgpu::Device, usage: wgpu::BufferUsages, capacity: u64) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tweak panel buffer"),
            size: capacity,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            capacity,
            usage,
        }
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) {
        let needed = data.len() as u64;
        if needed > self.capacity {
            *self = Self::new(device, self.usage, needed.next_power_of_two());
        }
        queue.write_buffer(&self.buffer, 0, data);
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PanelUniforms {
    screen_size_points: [f32; 2],
    _pad: [f32; 2],
}

impl PanelRenderer {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tweak_panel.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/tweak_panel.wgsl").into()),
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("panel uniform layout"),
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
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("panel texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("panel pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("panel pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 20, // pos (2×f32) + uv (2×f32) + color (4×u8)
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 16,
                            shader_location: 2,
                        },
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // egui colors are premultiplied.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("panel uniforms"),
            size: std::mem::size_of::<PanelUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("panel uniform bind group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("panel sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            uniforms,
            uniform_bg,
            texture_layout,
            sampler,
            textures: HashMap::new(),
            vertices: SizedBuffer::new(device, wgpu::BufferUsages::VERTEX, 1 << 16),
            indices: SizedBuffer::new(device, wgpu::BufferUsages::INDEX, 1 << 16),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        size_px: [u32; 2],
        ppp: f32,
        primitives: &[egui::ClippedPrimitive],
        deltas: &egui::TexturesDelta,
    ) {
        for (id, delta) in &deltas.set {
            self.update_texture(device, queue, *id, delta);
        }

        // Concatenate every mesh into one vertex/index buffer.
        let mut vertex_bytes: Vec<u8> = Vec::new();
        let mut index_bytes: Vec<u8> = Vec::new();
        let mut draws = Vec::new(); // (texture, index range, base vertex, clip)
        let mut base_vertex = 0i32;
        let mut index_start = 0u32;
        for prim in primitives {
            let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive else {
                continue;
            };
            vertex_bytes.extend_from_slice(bytemuck::cast_slice(&mesh.vertices));
            index_bytes.extend_from_slice(bytemuck::cast_slice(&mesh.indices));
            let count = mesh.indices.len() as u32;
            draws.push((
                mesh.texture_id,
                index_start..index_start + count,
                base_vertex,
                prim.clip_rect,
            ));
            index_start += count;
            base_vertex += mesh.vertices.len() as i32;
        }

        if !draws.is_empty() {
            let u = PanelUniforms {
                screen_size_points: [size_px[0] as f32 / ppp, size_px[1] as f32 / ppp],
                _pad: [0.0; 2],
            };
            queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
            self.vertices.write(device, queue, &vertex_bytes);
            self.indices.write(device, queue, &index_bytes);

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tweak panel pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // over the scene, not instead of it
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.uniform_bg, &[]);
            pass.set_vertex_buffer(0, self.vertices.buffer.slice(..));
            pass.set_index_buffer(self.indices.buffer.slice(..), wgpu::IndexFormat::Uint32);

            for (tex_id, range, base, clip) in draws {
                let Some((_, bind_group)) = self.textures.get(&tex_id) else {
                    continue;
                };
                // Clip rect: points -> pixels, clamped to the frame.
                let x = ((clip.min.x * ppp).round() as u32).min(size_px[0]);
                let y = ((clip.min.y * ppp).round() as u32).min(size_px[1]);
                let w = ((clip.width() * ppp).round() as u32).min(size_px[0] - x);
                let h = ((clip.height() * ppp).round() as u32).min(size_px[1] - y);
                if w == 0 || h == 0 {
                    continue;
                }
                pass.set_scissor_rect(x, y, w, h);
                pass.set_bind_group(1, bind_group, &[]);
                pass.draw_indexed(range, base, 0..1);
            }
        }

        for id in &deltas.free {
            self.textures.remove(id);
        }
    }

    fn update_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: egui::TextureId,
        delta: &egui::epaint::ImageDelta,
    ) {
        let egui::ImageData::Color(image) = &delta.image;
        let size = wgpu::Extent3d {
            width: image.width() as u32,
            height: image.height() as u32,
            depth_or_array_layers: 1,
        };

        // A full update (re)creates the texture; a partial one writes into it.
        if delta.pos.is_none() || !self.textures.contains_key(&id) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("panel texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("panel texture bind group"),
                layout: &self.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.textures.insert(id, (texture, bind_group));
        }

        let (texture, _) = &self.textures[&id];
        let origin = match delta.pos {
            Some([x, y]) => wgpu::Origin3d {
                x: x as u32,
                y: y as u32,
                z: 0,
            },
            None => wgpu::Origin3d::ZERO,
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(image.pixels.as_slice()),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * image.width() as u32),
                rows_per_image: None,
            },
            size,
        );
    }
}
