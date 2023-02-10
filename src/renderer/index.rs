use std::mem;

use ash::vk;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::staging_buf::*;

pub struct IndexBuffer {
    pub buf: vk::Buffer,
    dev_mem: vk::DeviceMemory,
    data_size: vk::DeviceSize,
    pub index_count: u32
}

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Index {
    pub data: [u16; 12]
}

impl IndexBuffer {
    pub fn new(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer, cmd_pool: vk::CommandPool, indices: &Vec<u32>) -> IndexBuffer {
        let data_size: vk::DeviceSize = (mem::size_of_val(indices.get(0).unwrap()) * indices.len()) as vk::DeviceSize;
        let index_count = indices.len();

        let (transfer_mem, transfer_buffer) = create_buffer(core,
                                                            physical_layer,
                                                            logical_layer,
                                                            data_size,
                                                            vk::BufferUsageFlags::TRANSFER_SRC,
                      vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT).unwrap();

        unsafe {
            let dev_memory = logical_layer.logical_device
                .map_memory(transfer_mem,
                            0,
                            data_size,
                            vk::MemoryMapFlags::empty())
                .unwrap() as *mut u32;
            dev_memory.copy_from_nonoverlapping(indices.as_ptr(), index_count);
            logical_layer.logical_device.unmap_memory(transfer_mem);
        }

        let (dev_mem, buf) = create_buffer(core,
                                           physical_layer,
                                           logical_layer,
                                           data_size,
                                           vk::BufferUsageFlags::INDEX_BUFFER | // Used by the vertex shader stage
                                               vk::BufferUsageFlags::TRANSFER_DST, // Can be a destination for transfer commands
                                           vk::MemoryPropertyFlags::DEVICE_LOCAL) // Local to GPU
            .expect("Failed to locate suitable device memory");

        copy_buffer(logical_layer, cmd_pool, transfer_buffer, buf, data_size);

        let ibuf = IndexBuffer {
            buf,
            dev_mem,
            data_size,
            index_count: index_count as u32
        };

        unsafe {
            logical_layer.logical_device.destroy_buffer(transfer_buffer, None);
            logical_layer.logical_device.free_memory(transfer_mem, None);
        }

        ibuf
    }

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            logical_layer.logical_device.destroy_buffer(self.buf, None);
            logical_layer.logical_device.free_memory(self.dev_mem, None);
        }
    }
}