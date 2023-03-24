use ash::vk;
use crate::vkcore::VkCore;

pub fn begin_single_time_commands(core: &VkCore, command_pool: vk::CommandPool) -> vk::CommandBuffer {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let command_buffer = unsafe {
        *core.logical_device.allocate_command_buffers(&alloc_info).unwrap().get(0).unwrap()
    };
    let begin_info = vk::CommandBufferBeginInfo::default()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    unsafe { core.logical_device.begin_command_buffer(command_buffer, &begin_info).unwrap(); }

    command_buffer
}

pub fn end_single_time_commands(core: &VkCore, command_pool: vk::CommandPool, command_buffer: vk::CommandBuffer) {
    unsafe { core.logical_device.end_command_buffer(command_buffer).unwrap(); }

    let command_buffers = [command_buffer];
    let submit_info = [vk::SubmitInfo::default()
        .command_buffers(&command_buffers)];

    unsafe {
        core.logical_device.queue_submit(core.graphics_queue, &submit_info, vk::Fence::null()).unwrap();
        core.logical_device.queue_wait_idle(core.graphics_queue).unwrap();
        core.logical_device.free_command_buffers(command_pool, &command_buffers);
    }
}