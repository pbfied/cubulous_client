use ash::vk;
use winit::event_loop::EventLoop;
use crate::vkcore::VkCore;

pub fn setup_sync_objects(core: &VkCore, max_frames: usize) -> (Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
    let sem_create_info = vk::SemaphoreCreateInfo::default();
    let fence_create_info = vk::FenceCreateInfo::default()
        .flags(vk::FenceCreateFlags::SIGNALED);

    let mut image_avail_vec: Vec<vk::Semaphore> = Vec::with_capacity(max_frames);
    let mut render_finished_vec: Vec<vk::Semaphore> = Vec::with_capacity(max_frames);
    let mut fences_vec: Vec<vk::Fence> = Vec::with_capacity(max_frames);

    for _ in 0..max_frames {
        unsafe {
            image_avail_vec.push(core.logical_device.create_semaphore(&sem_create_info, None).unwrap());
            render_finished_vec.push(core.logical_device.create_semaphore(&sem_create_info, None).unwrap());
            fences_vec.push(core.logical_device.create_fence(&fence_create_info, None).unwrap());
        }
    }

    (image_avail_vec, render_finished_vec, fences_vec)
}

pub unsafe fn cast_to_u8_slice<'a, T>(obj: &T) -> &'a [u8] {
    core::slice::from_raw_parts((obj as *const T) as *const u8, std::mem::size_of::<T>())
}