use std::ffi::CString;
use ash::vk;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowId;
use cubulous_client::renderer::core::Core;
use cubulous_client::renderer::logical_layer::LogicalLayer;
use cubulous_client::renderer::physical_layer::PhysicalLayer;
use cubulous_client::renderer::renderer::{RendererAPI, setup_sync_objects};

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct RtRenderer {
    pub core: Core,
    pub physical_layer: PhysicalLayer,
    pub logical_layer: LogicalLayer,
    pub image_available_sems: Vec<vk::Semaphore>,
    pub render_finished_sems: Vec<vk::Semaphore>,
    pub in_flight_fences: Vec<vk::Fence>,
    pub current_frame: usize,
}

impl RtRenderer {
    pub fn new(ev_loop: &EventLoop<()>) -> RtRenderer {

        let required_extensions: Vec<CString> = Vec::from([
            CString::from(vk::KhrSwapchainFn::name()), // Equivalent to the Vulkan VK_KHR_SWAPCHAIN_EXTENSION_NAME
        ]);
        let required_layers: Vec<String> = Vec::from([String::from("VK_LAYER_KHRONOS_validation")]);

        let core = Core::new(&ev_loop, &required_layers);
        let physical_layer = PhysicalLayer::new(&core, &required_extensions).unwrap();
        let logical_layer = LogicalLayer::new(&core, &physical_layer, &required_extensions);
        let current_frame: usize = 0;
        let (image_available_sems, render_finished_sems, in_flight_fences) =
            setup_sync_objects(&logical_layer, MAX_FRAMES_IN_FLIGHT);

        RtRenderer {
            core,
            physical_layer,
            logical_layer,
            image_available_sems,
            render_finished_sems,
            in_flight_fences,
            current_frame
        }
    }

    fn record_command_buffer(&self, image_index: u32) {
        let render_target = &self.render_target;
        let logical_device = &self.renderer.logical_layer.logical_device;

        let command_buffer = *self.command_buffers.get(self.renderer.current_frame).unwrap();

        unsafe {
            logical_device.begin_command_buffer(command_buffer, &begin_info).unwrap();
            logical_device.end_command_buffer(command_buffer).unwrap();
        }
    }

    fn draw_frame(&mut self) {
        let logical_device = &self.renderer.logical_layer.logical_device;
        let render_target = &self.render_target;
        let graphics_queue = self.renderer.logical_layer.graphics_queue;
        let present_queue = self.renderer.logical_layer.present_queue;
        let current_frame = self.renderer.current_frame;

        let fences = [*self.renderer.in_flight_fences.get(current_frame)
            .unwrap()];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let wait_sems = [*self.renderer.image_available_sems.get(current_frame).unwrap()];
        let command_buffers = [*self.command_buffers.get(current_frame).unwrap()];
        let sig_sems = [*self.renderer.render_finished_sems.get(current_frame).unwrap()];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_sems)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&sig_sems);
        let submit_array = [submit_info];
        let swap_chains = [render_target.swap_chain];

        // self.uniform_buffer.build_transforms(render_target, current_frame);

        unsafe {
            logical_device.wait_for_fences(&fences, true, u64::MAX).unwrap();

            let (next_image_idx, _) = match render_target.swap_loader
                .acquire_next_image(render_target.swap_chain, u64::MAX,
                                    *self.renderer.image_available_sems
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
            logical_device.reset_command_buffer(*self.command_buffers.get(self.renderer.current_frame).unwrap(),
                                                vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.record_command_buffer(next_image_idx);
            logical_device.queue_submit(graphics_queue, &submit_array,
                                        *self.renderer.in_flight_fences
                                            .get(self.renderer.current_frame).unwrap()).unwrap();

            match render_target.swap_loader.queue_present(present_queue, &present_info)
            {
                Err(r) => match r {
                    vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR => { self.recreate_swap_chain() },
                    _ => panic!("Unknown error")
                }
                Ok(_) => { }
            }
        }

        self.renderer.current_frame((current_frame + 1) % MAX_FRAMES_IN_FLIGHT);
    }
}

fn main() {
    // Generic window setup
    let event_loop = EventLoop::new();

    let renderer = RtRenderer::new(&event_loop);

    renderer.run_blocking(event_loop);
}