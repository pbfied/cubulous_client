use std::ffi::CString;
use ash::vk;
use ash::extensions::khr;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowId;
use cubulous_client::renderer::core::Core;
use cubulous_client::renderer::logical_layer::LogicalLayer;
use cubulous_client::renderer::physical_layer::PhysicalLayer;
use cubulous_client::renderer::render_target::RenderTarget;
use cubulous_client::renderer::renderer::create_common_vulkan_objs;
use cubulous_client::renderer::rt_accel::{create_acceleration_structures, RtBlas, RtTlas};
use cubulous_client::renderer::rt_canvas::RtCanvas;
use cubulous_client::renderer::rt_descriptor::{create_descriptor_sets, create_per_frame_descriptor_set_layout, create_singleton_descriptor_set_layout, destroy_descriptor_sets};
use cubulous_client::renderer::rt_pipeline::RtPipeline;
use cubulous_client::renderer::rt_ubo::RtUniformBuffer;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct RtRenderer {
    core: Core,
    physical_layer: PhysicalLayer,
    logical_layer: LogicalLayer,
    image_available_sems: Vec<vk::Semaphore>,
    render_finished_sems: Vec<vk::Semaphore>,
    render_target: RenderTarget,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,
    descriptor_layouts: Vec<vk::DescriptorSetLayout>,
    rt_pipeline: RtPipeline,
    descriptor_sets: Vec<vk::DescriptorSet>,
    descriptor_pool: vk::DescriptorPool,
    canvas: RtCanvas,
    accel_instance: khr::AccelerationStructure,
    tlas: Vec<RtTlas>,
    blas: RtBlas,
    per_frame_data: RtUniformBuffer
}

impl RtRenderer {
    pub fn new(ev_loop: &EventLoop<()>) -> RtRenderer {
        let required_extensions: Vec<CString> = Vec::from([
            CString::from(vk::KhrSwapchainFn::name()), // Equivalent to the Vulkan VK_KHR_SWAPCHAIN_EXTENSION_NAME
            CString::from(vk::KhrRayTracingPipelineFn::name()),
            CString::from(vk::KhrAccelerationStructureFn::name()),
            CString::from(vk::KhrDeferredHostOperationsFn::name()), // Required by VK_KHR_acceleration_structure
            CString::from(vk::ExtBufferDeviceAddressFn::name())
        ]);
        let required_layers: Vec<String> = Vec::from([String::from("VK_LAYER_KHRONOS_validation")]);
        let (core, physical_layer, logical_layer, image_available_sems, render_finished_sems,
            in_flight_fences) = create_common_vulkan_objs(ev_loop, MAX_FRAMES_IN_FLIGHT, required_extensions, required_layers);
        let render_target = RenderTarget::new(&core, &physical_layer, &logical_layer,
                                              // Apparently, B8G8R8A8_SRGB is incompatible with ImageUsageFlags::STORAGE
                                              // Another special note: Even though the swap chain images are not used
                                              // as render pass attachments, the COLOR_ATTACHMENT flag is needed for
                                              // some reason.
                                              vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::COLOR_ATTACHMENT,
                                              vk::Format::B8G8R8A8_UNORM,
                                              None);
        let pool_create_info = vk::CommandPoolCreateInfo::default().flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(physical_layer.graphics_family_index);
        let command_pool = unsafe { logical_layer.logical_device.create_command_pool(&pool_create_info, None).unwrap() };
        let buf_create_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32);
        let command_buffers = unsafe { logical_layer.logical_device.allocate_command_buffers(&buf_create_info).unwrap() };
        let current_frame: usize = 0;
        let descriptor_layouts = Vec::from([create_per_frame_descriptor_set_layout(&logical_layer)]);
            //create_singleton_descriptor_set_layout(&logical_layer)]);
        let rt_pipeline = RtPipeline::new(&core, &physical_layer, &logical_layer, command_pool, &descriptor_layouts);
        let canvas = RtCanvas::new(&core, &physical_layer, &logical_layer, &render_target, MAX_FRAMES_IN_FLIGHT);
        let (accel_instance, tlas, blas) = create_acceleration_structures(&core, &physical_layer, &logical_layer,
                                                                         command_pool, MAX_FRAMES_IN_FLIGHT);
        let per_frame_data = RtUniformBuffer::new(&core, &physical_layer, &logical_layer, MAX_FRAMES_IN_FLIGHT);
        let (descriptor_sets, descriptor_pool) = create_descriptor_sets(&logical_layer, &canvas, &tlas,
                                                                   //descriptor_layouts[0],
                                                     &per_frame_data, descriptor_layouts[0], MAX_FRAMES_IN_FLIGHT);

        RtRenderer {
            core,
            physical_layer,
            logical_layer,
            image_available_sems,
            render_finished_sems,
            render_target,
            command_pool,
            command_buffers,
            in_flight_fences,
            current_frame,
            descriptor_layouts,
            rt_pipeline,
            descriptor_sets,
            descriptor_pool,
            canvas,
            accel_instance,
            tlas,
            blas,
            per_frame_data
        }
    }

    fn record_command_buffer(&self, image_index: u32) {
        let logical_device = &self.logical_layer.logical_device;
        let begin_info = vk::CommandBufferBeginInfo::default();
        let command_buffer = *self.command_buffers.get(self.current_frame).unwrap();
        let ray_instances = khr::RayTracingPipeline::new(&self.core.instance, logical_device);
        let present_image = unsafe { *self.render_target.swap_loader.get_swapchain_images(self.render_target
            .swap_chain).unwrap().get(image_index as usize).unwrap() };
        let canvas_image = *self.canvas.images.get(self.current_frame).unwrap();

        let subresource_range = vk::ImageSubresourceRange::default()
            .base_mip_level(0)
            .layer_count(1)
            .level_count(1)
            .base_array_layer(0)
            .aspect_mask(vk::ImageAspectFlags::COLOR);
        let canvas_image_to_dst_barrier = vk::ImageMemoryBarrier::default()
            .image(canvas_image)
            .subresource_range(subresource_range)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::SHADER_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::GENERAL)
            .src_queue_family_index(self.physical_layer.graphics_family_index) // TODO Set up queue family ownership
            // transfers. It's not a problem for now since the graphics and presentation families on my dev platform
            // are the same.
            .dst_queue_family_index(self.physical_layer.graphics_family_index);
        let present_to_dst_barrier = vk::ImageMemoryBarrier::default()
            .image(present_image)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .subresource_range(subresource_range)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .src_queue_family_index(self.physical_layer.graphics_family_index)
            .dst_queue_family_index(self.physical_layer.graphics_family_index);
        let canvas_image_to_src_barrier = vk::ImageMemoryBarrier::default()
            .image(canvas_image)
            .subresource_range(subresource_range)
            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
            .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
            .old_layout(vk::ImageLayout::GENERAL)
            .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
            .src_queue_family_index(self.physical_layer.graphics_family_index)
            .dst_queue_family_index(self.physical_layer.graphics_family_index);
        let present_to_present_barrier = vk::ImageMemoryBarrier::default()
            .image(present_image)
            .subresource_range(subresource_range)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .src_queue_family_index(self.physical_layer.graphics_family_index)
            .dst_queue_family_index(self.physical_layer.graphics_family_index);
        let blit_subresource = vk::ImageSubresourceLayers::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_array_layer(0)
            .mip_level(0)
            .layer_count(1);
        let blit_offsets = [vk::Offset3D::default().x(0).y(0).z(0), vk::Offset3D::default().x(self.render_target
            .extent.width as i32).y(self.render_target.extent.height as i32).z(1)];
        let blit_region = vk::ImageBlit::default()
            .src_subresource(blit_subresource)
            .dst_subresource(blit_subresource)
            .src_offsets(blit_offsets)
            .dst_offsets(blit_offsets);

        unsafe {
            logical_device.begin_command_buffer(command_buffer, &begin_info).unwrap();
            logical_device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::RAY_TRACING_KHR, self.rt_pipeline
                .pipelines[0]);
            logical_device.cmd_bind_descriptor_sets(command_buffer, vk::PipelineBindPoint::RAY_TRACING_KHR, self
                .rt_pipeline.pipeline_layout, 0, &[*self.descriptor_sets.get(self.current_frame).unwrap()], &[]);
            logical_device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::ALL_COMMANDS,
                                                vk::PipelineStageFlags::ALL_COMMANDS, vk::DependencyFlags::empty(),
                                                &[], &[], &[canvas_image_to_dst_barrier]);
            ray_instances.cmd_trace_rays(command_buffer, &self.rt_pipeline.raygen_addr_region,
                                         &self.rt_pipeline.raymiss_addr_region,
                                         &self.rt_pipeline.rayhit_addr_region,
                                         &self.rt_pipeline.raycallable_addr_region,
                                         self.render_target.extent.width, self.render_target.extent.height, 1);
            logical_device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::ALL_COMMANDS,
                                                vk::PipelineStageFlags::ALL_COMMANDS, vk::DependencyFlags::empty(),
                                                &[], &[], &[canvas_image_to_src_barrier]);
            logical_device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::ALL_COMMANDS,
                                                vk::PipelineStageFlags::ALL_COMMANDS, vk::DependencyFlags::empty(),
                                                &[], &[], &[present_to_dst_barrier]);
            logical_device.cmd_blit_image(command_buffer, canvas_image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                                          present_image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[blit_region],
                                          vk::Filter::NEAREST);
            logical_device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::ALL_COMMANDS,
                                                vk::PipelineStageFlags::ALL_COMMANDS, vk::DependencyFlags::empty(),
                                                &[], &[], &[present_to_present_barrier]);
            logical_device.end_command_buffer(command_buffer).unwrap();
        }
    }

    fn recreate_swap_chain(&mut self) {
        self.cleanup_swap_chain();
        self.render_target = RenderTarget::new(&self.core, &self.physical_layer,
                                               &self.logical_layer, vk::ImageUsageFlags::TRANSFER_DST,
                                               vk::Format::B8G8R8A8_UNORM, None);
        self.canvas = RtCanvas::new(&self.core, &self.physical_layer, &self.logical_layer, &self.render_target,
                                    MAX_FRAMES_IN_FLIGHT);
    }

    fn cleanup_swap_chain(&self) {
        self.logical_layer.wait_idle();
        self.render_target.destroy(&self.logical_layer);
        self.canvas.destroy(&self.logical_layer);
    }

    fn draw_frame(&mut self) {
        let logical_device = &self.logical_layer.logical_device;
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
        let swap_chains = [self.render_target.swap_chain];

        self.per_frame_data.build_transforms(&self.render_target, current_frame);

        unsafe {
            logical_device.wait_for_fences(&fences, true, u64::MAX).unwrap();

            let (next_image_idx, _) = match self.render_target.swap_loader.acquire_next_image(self.render_target.swap_chain,
                                                                                              u64::MAX, *self.image_available_sems
                    .get(current_frame).unwrap(), vk::Fence::null()) {
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
            logical_device.reset_command_buffer(*self.command_buffers
                .get(self.current_frame)
                .unwrap(), vk::CommandBufferResetFlags::empty()).unwrap();
            self.record_command_buffer(next_image_idx);
            logical_device.queue_submit(graphics_queue, &submit_array,
                                        *self.in_flight_fences
                                            .get(self.current_frame).unwrap()).unwrap();


            match self.render_target.swap_loader.queue_present(present_queue, &present_info)
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

    fn window_id(&self) -> WindowId {
        self.core.window.id()
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
               Event::MainEventsCleared => self.core.window.request_redraw(), // Emits a RedrawRequested event
                // after input events end
                // Needed when a redraw is needed after the user resizes for example
                Event::RedrawRequested(window_id) if window_id == self.window_id() => self.draw_frame(),
                Event::LoopDestroyed => unsafe { self.logical_layer.logical_device.device_wait_idle().unwrap() },
                _ => (), // Similar to the "default" case of a switch statement: return void which is essentially () in Rust
            }
        });
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

    fn destroy_command_pool(&self) {
        unsafe { self.logical_layer.logical_device.destroy_command_pool(self.command_pool, None) };
    }
}


impl Drop for RtRenderer {
    fn drop(&mut self) {
        let logical_layer = &self.logical_layer;
        self.cleanup_swap_chain();
       // destroy_sampler(&self.logical_layer, self.sampler);
       //  self.texture.destroy(logical_layer);
        destroy_descriptor_sets(&self.logical_layer, &self.descriptor_layouts, self.descriptor_pool);
        for t in &self.tlas {
            t.destroy(logical_layer, &self.accel_instance);
        };
        self.blas.destroy(logical_layer, &self.accel_instance);
       //  self.index_buffer.destroy(logical_layer);
       //  self.vertex_buffer.destroy(logical_layer);
        self.destroy_sync_objects();
        self.destroy_command_pool();
        self.rt_pipeline.destroy(logical_layer);
        self.per_frame_data.destroy(logical_layer);
        // destroy_render_pass(logical_layer, self.render_pass);
        self.logical_layer.destroy();
        self.core.destroy();
    }
}

fn main() {
    // Generic window setup
    let event_loop = EventLoop::new();

    let renderer = RtRenderer::new(&event_loop);

    renderer.run_blocking(event_loop);
}