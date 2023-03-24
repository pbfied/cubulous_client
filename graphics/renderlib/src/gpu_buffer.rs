use std::mem;
use ash::vk;
use crate::single_time::{begin_single_time_commands, end_single_time_commands};
use crate::vkcore::VkCore;

pub(crate) fn find_buf_index(core: &VkCore, mem_props: vk::MemoryPropertyFlags, mem_reqs: vk::MemoryRequirements)
    -> Result<u32, ()> {
    let phys_mem_props = unsafe { core.instance.get_physical_device_memory_properties(core.physical_device)};

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

pub(crate) fn copy_buffer(core: &VkCore, cmd_pool: vk::CommandPool, src_buf: vk::Buffer, dest_buf: vk::Buffer,
                          data_size: vk::DeviceSize) {
    let command_buffer = begin_single_time_commands(core, cmd_pool);

    let copy_region = vk::BufferCopy::default()
        .size(data_size)
        .dst_offset(0)
        .src_offset(0);

    let copy_regions = [copy_region];

    unsafe {
        core.logical_device.cmd_copy_buffer(command_buffer, src_buf, dest_buf, &copy_regions);
    }

    end_single_time_commands(core, cmd_pool, command_buffer);
}

pub fn create_buffer(core: &VkCore,
                     size: vk::DeviceSize,
                     usage: vk::BufferUsageFlags,
                     mem_props: vk::MemoryPropertyFlags) -> (vk::DeviceMemory, vk::Buffer) {
    let buffer_create_info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let buffer = unsafe { core.logical_device.create_buffer(&buffer_create_info, None).unwrap() };

    let mem_reqs = unsafe { core.logical_device.get_buffer_memory_requirements(buffer)};

    let idx = find_buf_index(core, mem_props, mem_reqs).unwrap();

    // Explicit flushes are required otherwise
    let alloc_info = vk::MemoryAllocateInfo::default()
        .allocation_size(mem_reqs.size)
        .memory_type_index(idx);
    let buffer_mem = unsafe { core.logical_device.allocate_memory(&alloc_info, None).unwrap()};
    unsafe { core.logical_device.bind_buffer_memory(buffer, buffer_mem, 0).unwrap() };

    (buffer_mem, buffer)
}

pub struct GpuBuffer {
    pub buf: vk::Buffer,
    pub mem: vk::DeviceMemory,
    pub item_count: usize
}

impl GpuBuffer {
    pub fn new(core: &VkCore,
               size: vk::DeviceSize,
               usage: vk::BufferUsageFlags) -> GpuBuffer {
        let buffer_create_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { core.logical_device.create_buffer(&buffer_create_info, None).unwrap() };

        let mem_reqs = unsafe { core.logical_device.get_buffer_memory_requirements(buffer) };

        let idx = find_buf_index(core, vk::MemoryPropertyFlags::DEVICE_LOCAL, mem_reqs).unwrap();

        // Explicit flushes are required otherwise
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(idx);
        let buffer_mem = unsafe { core.logical_device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { core.logical_device.bind_buffer_memory(buffer, buffer_mem, 0).unwrap() };

        GpuBuffer {
            buf: buffer,
            mem: buffer_mem,
            item_count: 0
        }
    }

    pub fn new_initialized<T>(core: &VkCore, cmd_pool: vk::CommandPool, dst_flags: vk::BufferUsageFlags,
                              src_flags: vk::BufferUsageFlags, items: &[T]) -> GpuBuffer {
        let data_size: vk::DeviceSize = (mem::size_of::<T>() * items.len()) as vk::DeviceSize;
        let item_count = items.len();

        let (host_mem, host_buf) = create_buffer(core, data_size, vk::BufferUsageFlags::TRANSFER_SRC | src_flags,
                                  vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT);

        unsafe {
            let dev_memory = core.logical_device
                .map_memory(host_mem,
                            0,
                            data_size,
                            vk::MemoryMapFlags::empty())
                .unwrap() as *mut T;
            dev_memory.copy_from_nonoverlapping(items.as_ptr(), item_count);
            core.logical_device.unmap_memory(host_mem);
        }

        let mut device_buf = GpuBuffer::new(core, data_size, dst_flags |
            vk::BufferUsageFlags::TRANSFER_DST);
        copy_buffer(core, cmd_pool, host_buf, device_buf.buf, data_size);
        device_buf.item_count = item_count;
        unsafe {
            core.logical_device.destroy_buffer(host_buf, None);
            core.logical_device.free_memory(host_mem, None);
        }

        device_buf
    }

    pub fn destroy(&self, core: &VkCore) {
        unsafe {
            core.logical_device.destroy_buffer(self.buf, None);
            core.logical_device.free_memory(self.mem, None);
        }
    }

    pub fn get_device_address(&self, core: &VkCore) -> vk::DeviceAddress {
        let addr_info = vk::BufferDeviceAddressInfo::default()
            .buffer(self.buf);
        unsafe {
            core.logical_device.get_buffer_device_address(&addr_info)
        }
    }
}