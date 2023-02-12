use std::ffi::CString;
use ash::vk;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowId;
use cubulous_client::renderer::core::Core;
use cubulous_client::renderer::logical_layer::LogicalLayer;
use cubulous_client::renderer::physical_layer::PhysicalLayer;
use cubulous_client::renderer::renderer::RendererAPI;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct RtRenderer {
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
            setup_sync_objects(&logical_layer);

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


}

fn main() {
    // Generic window setup
    let event_loop = EventLoop::new();

    let renderer = RtRenderer::new(&event_loop);

    renderer.run_blocking(event_loop);
}