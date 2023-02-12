use std::ffi::CString;
use ash::vk;
use winit::event_loop::EventLoop;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;

pub struct Renderer {
    pub core: Core, // Windowing handles and Vk instance
    pub physical_layer: PhysicalLayer, // Physical device handle and derived properties
    pub logical_layer: LogicalLayer, // Logical device and logical queue
    pub image_available_sems: Vec<vk::Semaphore>,
    pub render_finished_sems: Vec<vk::Semaphore>,
    pub in_flight_fences: Vec<vk::Fence>,
    pub current_frame: usize
}

pub fn setup_sync_objects(logical_layer: &LogicalLayer, max_frames: usize) -> (Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
    let sem_create_info = vk::SemaphoreCreateInfo::default();
    let fence_create_info = vk::FenceCreateInfo::default()
        .flags(vk::FenceCreateFlags::SIGNALED);

    let mut image_avail_vec: Vec<vk::Semaphore> = Vec::with_capacity(max_frames);
    let mut render_finished_vec: Vec<vk::Semaphore> = Vec::with_capacity(max_frames);
    let mut fences_vec: Vec<vk::Fence> = Vec::with_capacity(max_frames);

    for _ in 0..max_frames {
        unsafe {
            image_avail_vec.push(logical_layer.logical_device.create_semaphore(&sem_create_info, None).unwrap());
            render_finished_vec.push(logical_layer.logical_device.create_semaphore(&sem_create_info, None).unwrap());
            fences_vec.push(logical_layer.logical_device.create_fence(&fence_create_info, None).unwrap());
        }
    }

    (image_avail_vec, render_finished_vec, fences_vec)
}

impl Renderer {
    pub fn new(ev_loop: &EventLoop<()>, max_frames: usize) -> Renderer {
        let required_extensions: Vec<CString> = Vec::from([
            CString::from(vk::KhrSwapchainFn::name()), // Equivalent to the Vulkan VK_KHR_SWAPCHAIN_EXTENSION_NAME
        ]);
        let required_layers: Vec<String> = Vec::from([String::from("VK_LAYER_KHRONOS_validation")]);

        let core = Core::new(&ev_loop, &required_layers);
        let physical_layer = PhysicalLayer::new(&core, &required_extensions).unwrap();
        let logical_layer = LogicalLayer::new(&core, &physical_layer, &required_extensions);
        let (image_available_sems, render_finished_sems, in_flight_fences) =
            setup_sync_objects(&logical_layer, max_frames);
        let current_frame: usize = 0;

        Renderer {
            core,
            physical_layer,
            logical_layer,
            image_available_sems,
            render_finished_sems,
            in_flight_fences,
            current_frame
        }
    }

    pub fn current_frame(&mut self, current_frame: usize) {
        self.current_frame = current_frame;
    }

    pub fn destroy_sync_objects(&self) {
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
