use ash::vk;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;

pub(crate) fn create_buffer(core: &Core,
                 physical_layer: &PhysicalLayer,
                 logical_layer: &LogicalLayer,
                 size: vk::DeviceSize,
                 usage: vk::BufferUsageFlags,
                 mem_props: vk::MemoryPropertyFlags) -> Result<(vk::DeviceMemory, vk::Buffer), ()> {
    let buffer_create_info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = unsafe { logical_layer.logical_device.create_buffer(&buffer_create_info, None).unwrap() };

    let mem_reqs = unsafe { logical_layer.logical_device.get_buffer_memory_requirements(buffer)};

    let idx = find_buf_index(core, physical_layer, mem_props, mem_reqs).unwrap();

    // Explicit flushes are required otherwise
    let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(idx);
    let buffer_mem = unsafe { logical_layer.logical_device.allocate_memory(&alloc_info, None).unwrap()};
    unsafe { logical_layer.logical_device.bind_buffer_memory(buffer, buffer_mem, 0).unwrap() };

    Ok((buffer_mem, buffer))
}

pub(crate) fn find_buf_index(core: &Core,
                             physical_layer: &PhysicalLayer,
                             mem_props: vk::MemoryPropertyFlags,
                             mem_reqs: vk::MemoryRequirements) -> Result<u32, ()> {
    let phys_mem_props = unsafe { core.instance.get_physical_device_memory_properties(physical_layer.physical_device)};

    let mut idx = -1;
    let mut retval = Err(());
    for i in 0..phys_mem_props.memory_type_count {
        if ((1 << i) & mem_reqs.memory_type_bits) > 0 && // If this physical memory type is valid for the requirement
            phys_mem_props.memory_types.get(i as usize).unwrap()
                .property_flags
                .contains(mem_props) {
            idx = i as i64;
            break;
        }
    }

    if idx >= -1 {
        retval = Ok(idx as u32);
    }

    retval
}

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

pub(crate) fn copy_buffer(logical_layer: &LogicalLayer, cmd_pool: vk::CommandPool,
               src_buf: vk::Buffer, dest_buf: vk::Buffer, data_size: vk::DeviceSize) {
    let command_buffer = begin_single_time_commands(logical_layer, cmd_pool);

    let copy_region = vk::BufferCopy::default()
        .size(data_size)
        .dst_offset(0)
        .src_offset(0);

    let copy_regions = [copy_region];

    unsafe {
        logical_layer.logical_device.cmd_copy_buffer(command_buffer, src_buf, dest_buf, &copy_regions);
    }

    end_single_time_commands(logical_layer, cmd_pool, command_buffer);
}