use std::mem;
use ash::vk;
use cgmath::{Deg, Matrix4, perspective, Point3, Transform, Vector3};
use renderlib::gpu_buffer::create_buffer;
use renderlib::render_target::RenderTarget;
use renderlib::vkcore::VkCore;
use crate::rt_pipeline::RtMissConstants;

// Remember to align fields according to the Vulkan specification 15.7.4
#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct RtPerFrameUbo {
    // model: Matrix4<f32>,
    pub inverse_view: Matrix4<f32>,
    pub inverse_proj: Matrix4<f32>
}

pub struct  RtUniformBuffer<T> {
    pub data: Vec<vk::Buffer>,
    mem: Vec<vk::DeviceMemory>,
    mapped: Vec<*mut T>
    // start_time: Instant
}

impl<T> RtUniformBuffer<T> {
    pub fn new(core: &VkCore, num_entries: usize) ->
                                                                                                             RtUniformBuffer<T> {
        let buffer_size: vk::DeviceSize = mem::size_of::<T>() as vk::DeviceSize;
        // let start_time = Instant::now();
        let mut uniform_buffer: RtUniformBuffer<T> = RtUniformBuffer {
            data: vec![],
            mem: vec![],
            mapped: vec![]
            // start_time
        };

        for _ in 0..num_entries {
            let (buf_mem, buffer) = create_buffer(core, buffer_size, vk::BufferUsageFlags::UNIFORM_BUFFER,
                                                  vk::MemoryPropertyFlags::HOST_COHERENT |
                                                      vk::MemoryPropertyFlags::HOST_VISIBLE);
            uniform_buffer.mem.push(buf_mem);
            uniform_buffer.data.push(buffer);

            let dev_memory: *mut T;
            unsafe {
                dev_memory = core.logical_device
                    .map_memory(buf_mem,
                                0,
                                buffer_size,
                                vk::MemoryMapFlags::empty())
                    .unwrap() as *mut T;
            }
            uniform_buffer.mapped.push(dev_memory);
        }

        uniform_buffer
    }

    pub fn set_mapped(&self, item: &[T], descriptor_idx: usize) {
        unsafe { self.mapped[descriptor_idx].copy_from_nonoverlapping(item.as_ptr(), mem::size_of::<T>()) };
    }

    pub fn destroy(&self, core: &VkCore) {
        for (buf, mem) in self.data.iter().zip(self.mem.iter()) {
            unsafe {
                core.logical_device.destroy_buffer(*buf, None);
                core.logical_device.free_memory(*mem, None);
            }
        }
    }
}
