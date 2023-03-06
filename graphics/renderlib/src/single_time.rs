use ash::vk;
use crate::logical_layer::LogicalLayer;

pub(crate) fn begin_single_time_commands(logical_layer: &LogicalLayer, command_pool: vk::CommandPool) -> vk::CommandBuffer {
    let alloc_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let command_buffer = unsafe {
        *logical_layer.logical_device.allocate_command_buffers(&alloc_info).unwrap().get(0).unwrap()
    };
    let begin_info = vk::CommandBufferBeginInfo::default()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

    unsafe { logical_layer.logical_device.begin_command_buffer(command_buffer, &begin_info).unwrap(); }

    command_buffer
}

pub(crate) fn end_single_time_commands(logical_layer: &LogicalLayer, command_pool: vk::CommandPool, command_buffer: vk::CommandBuffer) {
    unsafe { logical_layer.logical_device.end_command_buffer(command_buffer).unwrap(); }

    let command_buffers = [command_buffer];
    let submit_info = [vk::SubmitInfo::default()
        .command_buffers(&command_buffers)];

    unsafe {
        logical_layer.logical_device.queue_submit(logical_layer.graphics_queue, &submit_info, vk::Fence::null()).unwrap();
        logical_layer.logical_device.queue_wait_idle(logical_layer.graphics_queue).unwrap();
        logical_layer.logical_device.free_command_buffers(command_pool, &command_buffers);
    }
}