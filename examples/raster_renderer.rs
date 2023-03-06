use std::ffi::CString;
use ash::vk;
use ash::vk::Sampler;

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowId,
};

use renderlib::{
    color::Color,
    depth::{Depth, find_depth_format},
    descriptor::{create_descriptor_set_layout, Descriptor},
    frame_buffers::{destroy_frame_buffers, setup_frame_buffers},
    raster_pipeline::RasterPipeline,
    render_pass::{destroy_render_pass, setup_render_pass},
    render_target::RenderTarget,
    model::load_model,
    sampler::{create_sampler, destroy_sampler},
    texture::Texture,
    ubo::UniformBuffer
};
use renderlib::core::Core;
use renderlib::gpu_buffer::GpuBuffer;
use renderlib::logical_layer::LogicalLayer;
use renderlib::physical_layer::PhysicalLayer;
use renderlib::renderutils::create_common_vulkan_objs;

pub const MAX_FRAMES_IN_FLIGHT: usize = 2;
const MODEL_PATH: &str = "graphics/models/viking_room.obj";
const TEXTURE_PATH: &str = "graphics/textures/viking_room.png";
// const VERTICES: [Vertex; 8] = [
//     Vertex {
//         pos: [-0.5, -0.5, 0.0],
//         color: [1.0, 0.0, 0.0],
//         tex_coord: [1.0, 0.0]
//     },
//     Vertex {
//         pos: [0.5, -0.5, 0.0],
//         color: [0.0, 1.0, 0.0],
//         tex_coord: [0.0, 0.0]
//     },
//     Vertex {
//         pos: [0.5, 0.5, 0.0],
//         color: [0.0, 0.0, 1.0],
//         tex_coord: [0.0, 1.0]
//     },
//     Vertex {
//         pos: [-0.5, 0.5, 0.0],
//         color: [1.0, 1.0, 1.0],
//         tex_coord: [1.0, 1.0]
//     },
//
//     Vertex {
//         pos: [-0.5, -0.5, -0.5],
//         color: [1.0, 0.0, 0.0],
//         tex_coord: [1.0, 0.0]
//     },
//     Vertex {
//         pos: [0.5, -0.5, -0.5],
//         color: [0.0, 1.0, 0.0],
//         tex_coord: [0.0, 0.0]
//     },
//     Vertex {
//         pos: [0.5, 0.5, -0.5],
//         color: [0.0, 0.0, 1.0],
//         tex_coord: [0.0, 1.0]
//     },
//     Vertex {
//         pos: [-0.5, 0.5, -0.5],
//         color: [1.0, 1.0, 1.0],
//         tex_coord: [1.0, 1.0]
//     },
// ];
//
// const INDICES: [u32; 12] =  [0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4];

pub struct RasterRenderer {
    core: Core, // Windowing handles and Vk instance
    physical_layer: PhysicalLayer, // Physical device handle and derived properties
    logical_layer: LogicalLayer, // Logical device and logical queue
    image_available_sems: Vec<vk::Semaphore>,
    render_finished_sems: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,
    render_target: RenderTarget,
    raster_pipeline: RasterPipeline,
    render_pass: vk::RenderPass,
    frame_buffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    vertex_buffer: GpuBuffer,
    index_buffer: GpuBuffer,
    uniform_buffer: UniformBuffer,
    descriptor: Descriptor,
    texture: Texture,
    sampler: Sampler,
    depth: Depth,
    color: Color
}

impl RasterRenderer {
    pub fn new(ev_loop: &EventLoop<()>) -> RasterRenderer {
        let required_extensions: Vec<CString> = Vec::from([
            CString::from(vk::KhrSwapchainFn::name()), // Equivalent to the Vulkan VK_KHR_SWAPCHAIN_EXTENSION_NAME
        ]);
        let required_layers: Vec<String> = Vec::from([String::from("VK_LAYER_KHRONOS_validation")]);
        let (core, physical_layer, logical_layer, image_available_sems, 
            render_finished_sems, in_flight_fences) = create_common_vulkan_objs(ev_loop, MAX_FRAMES_IN_FLIGHT,
                                                                                required_extensions, required_layers);
        let render_target = RenderTarget::new(&core, &physical_layer, &logical_layer,
                                              vk::ImageUsageFlags::COLOR_ATTACHMENT, vk::Format::B8G8R8A8_SRGB,
                                              Some(vk::ColorSpaceKHR::SRGB_NONLINEAR));
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
        let vertex_buffer = GpuBuffer::new_initialized(&core, &physical_layer, &logical_layer, command_pool,
                                                       vk::BufferUsageFlags::VERTEX_BUFFER,
                                                       vk::BufferUsageFlags::empty(), vertices.as_slice());
        let index_buffer = GpuBuffer::new_initialized(&core, &physical_layer, &logical_layer, command_pool,
                                                      vk::BufferUsageFlags::INDEX_BUFFER,
                                                      vk::BufferUsageFlags::empty(), indices.as_slice());
        let uniform_buffer = UniformBuffer::new(&core, &physical_layer, &logical_layer, MAX_FRAMES_IN_FLIGHT);
        let texture = Texture::new(&core, &physical_layer, &logical_layer, command_pool, TEXTURE_PATH);
        // let texture = Texture::new(&core, &physical_layer, &logical_layer, command_pool, "textures/texture.jpg");

        let sampler = create_sampler(&core, &physical_layer, &logical_layer, texture.mip_levels);
        let descriptor = Descriptor::new(&logical_layer, &uniform_buffer, sampler, &texture, descriptor_layout,
                                         MAX_FRAMES_IN_FLIGHT);

        RasterRenderer {
            core,
            physical_layer,
            logical_layer,
            image_available_sems,
            render_finished_sems,
            in_flight_fences,
            current_frame: 0,
            render_target,
            raster_pipeline,
            render_pass,
            frame_buffers,
            command_pool,
            command_buffers,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            descriptor,
            texture,
            sampler,
            depth,
            color
        }
    }

    fn destroy_command_pool(&self) {
        unsafe { self.logical_layer.logical_device.destroy_command_pool(self.command_pool, None) };
    }

    fn record_command_buffer(&self, image_index: u32) {
        let render_target = &self.render_target;
        let logical_device = &self.logical_layer.logical_device;

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
            .height(render_target.extent.height)
            .width(render_target.extent.width);
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

        let viewports = [setup_viewport(&render_target.extent)];

        let scissors = [setup_scissor(&render_target.extent)];

        let command_buffer = *self.command_buffers.get(self.current_frame).unwrap();

        let vertex_buffers = [self.vertex_buffer.buf];

        let offsets: [vk::DeviceSize; 1] = [0];

        unsafe {
            logical_device.begin_command_buffer(command_buffer, &begin_info).unwrap();
            logical_device.cmd_begin_render_pass(command_buffer,
                                                      &render_pass_info,
                                                      vk::SubpassContents::INLINE); // Execute commands in primary buffer
            logical_device.cmd_bind_pipeline(command_buffer,
                                                  vk::PipelineBindPoint::GRAPHICS,
                                                  *self.raster_pipeline.pipelines.get(0).unwrap());
            logical_device.cmd_bind_vertex_buffers(command_buffer, 0, &vertex_buffers, &offsets);
            logical_device.cmd_bind_index_buffer(command_buffer, self.index_buffer.buf, 0, vk::IndexType::UINT32);
            logical_device.cmd_set_viewport(command_buffer, 0, &viewports);
            logical_device.cmd_set_scissor(command_buffer, 0, &scissors);
            // self.logical_layer.logical_device.cmd_draw(command_buffer,
            //                              self.vertex_buffer.vertex_count,
            //                              1,
            //                              0, // Vertex buffer offset, lowest value of gl_VertexIndex
            //                              0); // lowest value of gl_InstanceIndex
            logical_device.cmd_bind_descriptor_sets(command_buffer,
                                                                       vk::PipelineBindPoint::GRAPHICS,
                                                                       self.raster_pipeline.pipeline_layout,
                                                                       0,
                                                                       &[*self.descriptor.sets.get(self.current_frame).unwrap()],
                                                                       &[]);
            logical_device.cmd_draw_indexed(command_buffer, self.index_buffer.item_count as u32, 1, 0, 0, 0);
            logical_device.cmd_end_render_pass(command_buffer);
            logical_device.end_command_buffer(command_buffer).unwrap();
        }
    }

    fn cleanup_swap_chain(&self) {
        let logical_layer = &self.logical_layer;
        self.logical_layer.wait_idle();
        self.color.destroy(logical_layer);
        self.depth.destroy(logical_layer);
        destroy_frame_buffers(logical_layer, &self.frame_buffers);
        self.render_target.destroy(&self.logical_layer);
    }

    fn recreate_swap_chain(&mut self) {
        self.cleanup_swap_chain();
        self.render_target = RenderTarget::new(&self.core, &self.physical_layer,
                                               &self.logical_layer, vk::ImageUsageFlags::COLOR_ATTACHMENT,
                                               vk::Format::B8G8R8A8_SRGB, Some(vk::ColorSpaceKHR::SRGB_NONLINEAR));
        self.color = Color::new(&self.core, &self.physical_layer,
                                &self.logical_layer, &self.render_target);
        self.depth = Depth::new(&self.core, &self.physical_layer,
                                &self.logical_layer, &self.render_target,
                                self.command_pool);
        self.frame_buffers = setup_frame_buffers(&self.logical_layer, self.render_pass,
                                                 &self.render_target,
                                                 self.depth.view, self.color.view);
    }

    fn run_blocking(mut self, event_loop: EventLoop<()>) {
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

    fn window_id(&self) -> WindowId {
        self.core.window.id()
    }

    fn draw_frame(&mut self) {
        let logical_device = &self.logical_layer.logical_device;
        let render_target = &self.render_target;
        let graphics_queue = self.logical_layer.graphics_queue;
        let present_queue = self.logical_layer.present_queue;
        let current_frame = self.current_frame;

        let fences = [*self.in_flight_fences.get(current_frame)
            .unwrap()];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let wait_sems = [*self.image_available_sems.get(current_frame).unwrap()];
        let command_buffers = [*self.command_buffers.get(current_frame).unwrap()];
        let sig_sems = [*self.render_finished_sems.get(current_frame).unwrap()];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_sems)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&sig_sems);
        let submit_array = [submit_info];
        let swap_chains = [render_target.swap_chain];

        self.uniform_buffer.build_transforms(render_target, current_frame);

        unsafe {
            logical_device.wait_for_fences(&fences, true, u64::MAX).unwrap();

            let (next_image_idx, _) = match render_target.swap_loader
                .acquire_next_image(render_target.swap_chain, u64::MAX,
                                    *self.image_available_sems
                                        .get(current_frame)
                                        .unwrap(), vk::Fence::null()) {
                Ok(img_idx) => img_idx,
                Err(result) => match result {
                    vk::Result::ERROR_OUT_OF_DATE_KHR => { self.recreate_swap_chain(); return },
                    _ => panic!("Unknown error at acquire_next_image")
                }
            };

            logical_device.reset_fences(&fences).unwrap();

            let image_indices = [next_image_idx];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&sig_sems)
                .swapchains(&swap_chains)
                .image_indices(&image_indices);
            logical_device.reset_command_buffer(*self.command_buffers.get(self.current_frame).unwrap(),
                                                                   vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.record_command_buffer(next_image_idx);
            logical_device.queue_submit(graphics_queue, &submit_array,
                                        *self.in_flight_fences
                                            .get(self.current_frame).unwrap()).unwrap();

            match render_target.swap_loader.queue_present(present_queue, &present_info)
            {
                Err(r) => match r {
                    vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR => { self.recreate_swap_chain() },
                    _ => panic!("Unknown error")
                }
                Ok(_) => { }
            }
        }

        self.current_frame = (current_frame + 1) % MAX_FRAMES_IN_FLIGHT;
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
}

impl Drop for RasterRenderer {
    fn drop(&mut self) {
        let logical_layer = &self.logical_layer;
        self.cleanup_swap_chain();
        destroy_sampler(&self.logical_layer, self.sampler);
        self.texture.destroy(logical_layer);
        self.descriptor.destroy(logical_layer);
        self.index_buffer.destroy(logical_layer);
        self.vertex_buffer.destroy(logical_layer);
        self.destroy_sync_objects();
        self.destroy_command_pool();
        self.raster_pipeline.destroy(logical_layer);
        self.uniform_buffer.destroy(logical_layer);
        destroy_render_pass(logical_layer, self.render_pass);
        self.logical_layer.destroy();
        self.core.destroy();
    }
}

fn main() {
    // Generic window setup
    let event_loop = EventLoop::new();

    let renderer = RasterRenderer::new(&event_loop);

    renderer.run_blocking(event_loop);
}
