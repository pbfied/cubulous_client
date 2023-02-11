use std::env;
use std::ffi::{c_char, CStr, CString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::mem;
use std::path::Path;

use ash::{vk, Device, Entry, Instance};
use ash::extensions::khr::{Surface, Swapchain};
use ash::vk::Sampler;
use num::clamp;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle}; // Entry holds Vulkan functions
// vk holds Vulkan structs with no methods along with Vulkan macros
// Instance wraps Entry functions with a winit surface and some under the hood initialization parameters
// Device is a logical Vulkan device

use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Icon, Window, WindowBuilder, WindowId},
};

use cubulous_client::renderer::{
    color::Color,
    core::Core,
    depth::{Depth, find_depth_format},
    descriptor::{create_descriptor_set_layout, Descriptor},
    frame_buffers::{destroy_frame_buffers, setup_frame_buffers},
    logical_layer::LogicalLayer,
    physical_layer::PhysicalLayer,
    raster_pipeline::RasterPipeline,
    render_pass::{destroy_render_pass, setup_render_pass},
    render_target::RenderTarget,
    vertex::{VertexBuffer, Vertex},
    index::{Index, IndexBuffer},
    model::load_model,
    sampler::{create_sampler, destroy_sampler},
    texture::Texture,
    ubo::UniformBuffer
};

const MODEL_PATH: &str = "models/viking_room.obj";
const TEXTURE_PATH: &str = "textures/viking_room.png";
const MAX_FRAMES_IN_FLIGHT: usize = 2;
const VERTICES: [Vertex; 8] = [
    Vertex {
        pos: [-0.5, -0.5, 0.0],
        color: [1.0, 0.0, 0.0],
        texCoord: [1.0, 0.0]
    },
    Vertex {
        pos: [0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0],
        texCoord: [0.0, 0.0]
    },
    Vertex {
        pos: [0.5, 0.5, 0.0],
        color: [0.0, 0.0, 1.0],
        texCoord: [0.0, 1.0]
    },
    Vertex {
        pos: [-0.5, 0.5, 0.0],
        color: [1.0, 1.0, 1.0],
        texCoord: [1.0, 1.0]
    },

    Vertex {
        pos: [-0.5, -0.5, -0.5],
        color: [1.0, 0.0, 0.0],
        texCoord: [1.0, 0.0]
    },
    Vertex {
        pos: [0.5, -0.5, -0.5],
        color: [0.0, 1.0, 0.0],
        texCoord: [0.0, 0.0]
    },
    Vertex {
        pos: [0.5, 0.5, -0.5],
        color: [0.0, 0.0, 1.0],
        texCoord: [0.0, 1.0]
    },
    Vertex {
        pos: [-0.5, 0.5, -0.5],
        color: [1.0, 1.0, 1.0],
        texCoord: [1.0, 1.0]
    },
];

const INDICES: [u32; 12] =  [0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4];

pub struct RasterRenderer {
    core: Core, // Windowing handles and Vk instance
    physical_layer: PhysicalLayer, // Physical device handle and derived properties
    logical_layer: LogicalLayer, // Logical device and logical queue
    raster_pipeline: RasterPipeline,
    render_pass: vk::RenderPass,
    render_target: RenderTarget,
    frame_buffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available_sems: Vec<vk::Semaphore>,
    render_finished_sems: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer,
    uniform_buffer: UniformBuffer,
    descriptor: Descriptor,
    texture: Texture,
    sampler: Sampler,
    depth: Depth,
    color: Color,
    vertices: Vec<Vertex>,
    indices: Vec<u32>
}

impl RasterRenderer {
    pub fn new(ev_loop: &EventLoop<()>) -> RasterRenderer {
        fn setup_sync_objects(logical_layer: &LogicalLayer) -> (Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
            let sem_create_info = vk::SemaphoreCreateInfo::default();
            let fence_create_info = vk::FenceCreateInfo::default()
                .flags(vk::FenceCreateFlags::SIGNALED);

            let mut image_avail_vec: Vec<vk::Semaphore> = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
            let mut render_finished_vec: Vec<vk::Semaphore> = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);
            let mut fences_vec: Vec<vk::Fence> = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT as usize);

            for _ in 0..MAX_FRAMES_IN_FLIGHT {
                unsafe {
                    image_avail_vec.push(logical_layer.logical_device.create_semaphore(&sem_create_info, None).unwrap());
                    render_finished_vec.push(logical_layer.logical_device.create_semaphore(&sem_create_info, None).unwrap());
                    fences_vec.push(logical_layer.logical_device.create_fence(&fence_create_info, None).unwrap());
                }
            }

            (image_avail_vec, render_finished_vec, fences_vec)
        }

        let required_extensions: Vec<CString> = Vec::from([
            CString::from(vk::KhrSwapchainFn::name()), // Equivalent to the Vulkan VK_KHR_SWAPCHAIN_EXTENSION_NAME
        ]);
        let required_layers: Vec<String> = Vec::from([String::from("VK_LAYER_KHRONOS_validation")]);

        let core = Core::new(&ev_loop, &required_layers);
        let physical_layer = PhysicalLayer::new(&core, &required_extensions).unwrap();
        let logical_layer = LogicalLayer::new(&core, &physical_layer, &required_extensions);
        let render_target = RenderTarget::new(&core, &physical_layer, &logical_layer);
        let render_pass = setup_render_pass(&logical_layer, &render_target,
                                            find_depth_format(&core, &physical_layer),
                                            physical_layer.max_msaa_samples);
        let descriptor_layout = create_descriptor_set_layout(&logical_layer);
        let raster_pipeline = RasterPipeline::new(&logical_layer, render_pass,
                                                  descriptor_layout, physical_layer.max_msaa_samples);
        let pool_create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(physical_layer.graphics_family_index);
        let command_pool = unsafe {
            logical_layer.logical_device.create_command_pool(&pool_create_info, None).unwrap()
        };

        let depth = Depth::new(&core, &physical_layer, &logical_layer, &render_target, command_pool);
        let color = Color::new(&core, &physical_layer, &logical_layer, &render_target);
        let frame_buffers = setup_frame_buffers(&logical_layer, render_pass,
                                                &render_target, depth.view,
                                                color.view);

        let buf_create_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32);
        let command_buffers = unsafe { logical_layer.logical_device.allocate_command_buffers(&buf_create_info).unwrap() };
        let (vertices, indices) = load_model(MODEL_PATH);
        // let (vertices, indices) = (Vec::from(VERTICES), Vec::from(INDICES));
        let vertex_buffer = VertexBuffer::new(&core, &physical_layer, &logical_layer, command_pool, vertices.as_slice());
        let index_buffer = IndexBuffer::new(&core, &physical_layer, &logical_layer, command_pool, &indices);
        let uniform_buffer = UniformBuffer::new(&core, &physical_layer, &logical_layer, MAX_FRAMES_IN_FLIGHT);
        let texture = Texture::new(&core, &physical_layer, &logical_layer, command_pool, TEXTURE_PATH);
        // let texture = Texture::new(&core, &physical_layer, &logical_layer, command_pool, "textures/texture.jpg");

        let sampler = create_sampler(&core, &physical_layer, &logical_layer, texture.mip_levels);
        let descriptor = Descriptor::new(&logical_layer, &uniform_buffer, sampler, &texture, descriptor_layout, MAX_FRAMES_IN_FLIGHT);

        let (image_available_sems, render_finished_sems, in_flight_fences) =
        setup_sync_objects(&logical_layer);

        let current_frame = 0;

        RasterRenderer {
            core,
            physical_layer,
            logical_layer,
            raster_pipeline,
            render_pass,
            render_target,
            frame_buffers,
            command_pool,
            command_buffers,
            image_available_sems,
            render_finished_sems,
            in_flight_fences,
            current_frame,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            descriptor,
            texture,
            sampler,
            depth,
            color,
            vertices,
            indices,
        }
    }

    fn destroy_command_pool(&self) {
        unsafe { self.logical_layer.logical_device.destroy_command_pool(self.command_pool, None) };
    }

    fn destroy_sync_objects(&self) {
        unsafe {
            for i in self.image_available_sems.iter() {
                self.logical_layer.logical_device.destroy_semaphore(*i, None);
            }
            for r in self.render_finished_sems.iter() {
                self.logical_layer.logical_device.destroy_semaphore(*r, None);
            }
            for f in self.in_flight_fences.iter() {
                self.logical_layer.logical_device.destroy_fence(*f, None);
            }
        }
    }

    fn record_command_buffer(&self, image_index: u32) {
        // Defines a transformation from a VK image to the framebuffer
        fn setup_viewport(swap_extent: &vk::Extent2D) -> vk::Viewport {
            vk::Viewport::default()
                .x(0.0) // Origin
                .y(0.0)
                .width(swap_extent.width as f32) // Max range from origin
                .height(swap_extent.height as f32)
                .min_depth(0.0) // ??
                .max_depth(1.0)
        }

        fn setup_scissor(swap_extent: &vk::Extent2D) -> vk::Rect2D {
            vk::Rect2D::default()
                .offset(vk::Offset2D::default()
                    .x(0)
                    .y(0))
                .extent(*swap_extent)
        }

        let begin_info = vk::CommandBufferBeginInfo::default();

        let render_offset = vk::Offset2D::default()
            .x(0)
            .y(0);
        let render_extent = vk::Extent2D::default()
            .height(self.render_target.extent.height)
            .width(self.render_target.extent.width);
        let render_area = vk::Rect2D::default() // Area where shader loads and stores occur
            .offset(render_offset)
            .extent(render_extent);

        let clear_color_value = vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 1.0]
        };
        let clear_depth_stencil = vk::ClearDepthStencilValue::default()
            .depth(1.0)
            .stencil(0);
        let clear_values = [
            vk::ClearValue {
                color: clear_color_value
            },
            vk::ClearValue {
                depth_stencil: clear_depth_stencil
            }
        ];

        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(self.render_pass)
            .framebuffer(self.frame_buffers[image_index as usize])
            .render_area(render_area)
            .clear_values(&clear_values);

        let viewports = [setup_viewport(&self.render_target.extent)];

        let scissors = [setup_scissor(&self.render_target.extent)];

        let command_buffer = *self.command_buffers.get(self.current_frame).unwrap();

        let vertex_buffers = [self.vertex_buffer.buf];

        let offsets: [vk::DeviceSize; 1] = [0];

        unsafe {
            self.logical_layer.logical_device.begin_command_buffer(command_buffer, &begin_info).unwrap();
            self.logical_layer.logical_device.cmd_begin_render_pass(command_buffer,
                                                      &render_pass_info,
                                                      vk::SubpassContents::INLINE); // Execute commands in primary buffer
            self.logical_layer.logical_device.cmd_bind_pipeline(command_buffer,
                                                  vk::PipelineBindPoint::GRAPHICS,
                                                  *self.raster_pipeline.pipelines.get(0).unwrap());
            self.logical_layer.logical_device.cmd_bind_vertex_buffers(command_buffer, 0, &vertex_buffers, &offsets);
            self.logical_layer.logical_device.cmd_bind_index_buffer(command_buffer, self.index_buffer.buf, 0, vk::IndexType::UINT32);
            self.logical_layer.logical_device.cmd_set_viewport(command_buffer, 0, &viewports);
            self.logical_layer.logical_device.cmd_set_scissor(command_buffer, 0, &scissors);
            // self.logical_layer.logical_device.cmd_draw(command_buffer,
            //                              self.vertex_buffer.vertex_count,
            //                              1,
            //                              0, // Vertex buffer offset, lowest value of gl_VertexIndex
            //                              0); // lowest value of gl_InstanceIndex
            self.logical_layer.logical_device.cmd_bind_descriptor_sets(command_buffer,
                                                                       vk::PipelineBindPoint::GRAPHICS,
                                                                       self.raster_pipeline.pipeline_layout,
                                                                       0,
                                                                       &[*self.descriptor.sets.get(self.current_frame).unwrap()],
                                                                       &[]);
            self.logical_layer.logical_device.cmd_draw_indexed(command_buffer, self.index_buffer.index_count, 1, 0, 0, 0);
            self.logical_layer.logical_device.cmd_end_render_pass(command_buffer);
            self.logical_layer.logical_device.end_command_buffer(command_buffer).unwrap();
        }
    }

    fn draw_frame(&mut self) {
        let fences = [*self.in_flight_fences.get(self.current_frame).unwrap()];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let wait_sems = [*self.image_available_sems.get(self.current_frame).unwrap()];
        let command_buffers = [*self.command_buffers.get(self.current_frame).unwrap()];
        let sig_sems = [*self.render_finished_sems.get(self.current_frame).unwrap()];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_sems)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&sig_sems);
        let submit_array = [submit_info];
        let swap_chains = [self.render_target.swap_chain];

        self.uniform_buffer.build_transforms(&self.render_target, self.current_frame);

        unsafe {
            self.logical_layer.logical_device.wait_for_fences(&fences, true, u64::MAX).unwrap();

            let (next_image_idx, _) = match self.render_target.swap_loader.acquire_next_image(self.render_target.swap_chain,
                                    u64::MAX,
                                    *self.image_available_sems.get(self.current_frame).unwrap(),
                                    vk::Fence::null()) {
                Ok(img_idx) => img_idx,
                Err(result) => match result {
                    vk::Result::ERROR_OUT_OF_DATE_KHR => { self.recreate_swap_chain(); return },
                    _ => panic!("Unknown error at acquire_next_image")
                }
            };

            self.logical_layer.logical_device.reset_fences(&fences).unwrap();

            let image_indices = [next_image_idx];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&sig_sems)
                .swapchains(&swap_chains)
                .image_indices(&image_indices);
            self.logical_layer.logical_device.reset_command_buffer(*self.command_buffers.get(self.current_frame).unwrap(),
                                                     vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.record_command_buffer(next_image_idx);
            self.logical_layer.logical_device.queue_submit(self.logical_layer.graphics_queue, &submit_array, *self.in_flight_fences.get(self.current_frame).unwrap()).unwrap();

            match self.render_target.swap_loader.queue_present(self.logical_layer.present_queue, &present_info)
            {
                Err(r) => match r {
                    vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR => { self.recreate_swap_chain() },
                    _ => panic!("Unknown error")
                }
                Ok(_) => { }
            }
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
    }

    fn cleanup_swap_chain(&self) {
        self.logical_layer.wait_idle();
        self.color.destroy(&self.logical_layer);
        self.depth.destroy(&self.logical_layer);
        destroy_frame_buffers(&self.logical_layer, &self.frame_buffers);
        self.render_target.destroy(&self.logical_layer);
    }

    fn recreate_swap_chain(&mut self) {
        self.cleanup_swap_chain();
        self.color = Color::new(&self.core, &self.physical_layer, &self.logical_layer, &self.render_target);
        self.depth = Depth::new(&self.core, &self.physical_layer, &self.logical_layer,
                                &self.render_target, self.command_pool);
        self.render_target = RenderTarget::new(&self.core, &self.physical_layer, &self.logical_layer);
        self.frame_buffers = setup_frame_buffers(&self.logical_layer, self.render_pass,
                                                 &self.render_target, self.depth.view,
                                                 self.color.view);
    }

    fn window_id(&self) -> WindowId {
        self.core.window.id()
    }

    pub fn run_blocking(mut self, event_loop: EventLoop<()>) {
        event_loop.run(move |event, _, control_flow| {
            control_flow.set_poll();

            match event {
                Event::WindowEvent {
                    // If event has Event::WindowEvent type and event: WindowEvent::CloseRequested member and if window_id == window.id()
                    event: WindowEvent::CloseRequested,
                    window_id,
                } if window_id == self.window_id() => *control_flow = ControlFlow::Exit,
                Event::MainEventsCleared => self.core.window.request_redraw(), // Emits a RedrawRequested event after input events end
                                                                        // Needed when a redraw is needed after the user resizes for example
                Event::RedrawRequested(window_id) if window_id == self.window_id() => self.draw_frame(),
                Event::LoopDestroyed => unsafe { self.logical_layer.logical_device.device_wait_idle().unwrap() },
                _ => (), // Similar to the "default" case of a switch statement: return void which is essentially () in Rust
            }
        });
    }
}

impl Drop for RasterRenderer {
    fn drop(&mut self) {
        self.cleanup_swap_chain();
        destroy_sampler(&self.logical_layer, self.sampler);
        self.texture.destroy(&self.logical_layer);
        self.descriptor.destroy(&self.logical_layer);
        self.index_buffer.destroy(&self.logical_layer);
        self.vertex_buffer.destroy(&self.logical_layer);
        self.destroy_sync_objects();
        self.destroy_command_pool();
        self.raster_pipeline.destroy(&self.logical_layer);
        self.uniform_buffer.destroy(&self.logical_layer);
        destroy_render_pass(&self.logical_layer, self.render_pass);
        self.logical_layer.destroy();
        self.core.destroy();
    }
}

fn hello_triangle() {
    // Generic window setup
    let event_loop = EventLoop::new();

    let renderer = RasterRenderer::new(&event_loop);

    renderer.run_blocking(event_loop);
}

fn main() {
    hello_triangle();
}
