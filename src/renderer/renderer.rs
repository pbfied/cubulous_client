use std::ffi::CString;
use ash::vk;
use winit::event_loop::EventLoop;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;


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

pub fn create_common_vulkan_objs(ev_loop: &EventLoop<()>, max_frames: usize, required_extensions: Vec<CString>, required_layers: Vec<String>)
                                 -> (Core, PhysicalLayer, LogicalLayer, Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
    let core = Core::new(&ev_loop, &required_layers);
    let physical_layer = PhysicalLayer::new(&core, &required_extensions).unwrap();
    let logical_layer = LogicalLayer::new(&core, &physical_layer, &required_extensions);
    let (image_available_sems, render_finished_sems, in_flight_fences) =
        setup_sync_objects(&logical_layer, max_frames);

    (core, physical_layer, logical_layer, image_available_sems, render_finished_sems, in_flight_fences)
}
