use memoffset::offset_of;
use std::mem;

use ash::vk;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::staging_buf::{create_buffer, copy_buffer};

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub tex_coord: [f32; 2]
}

pub struct VertexBuffer {
    pub buf: vk::Buffer,
    dev_mem: vk::DeviceMemory
}

impl Vertex {
    pub(crate) fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(mem::size_of::<Vertex>() as u32) // Number of bytes per entry in the binding
            .input_rate(vk::VertexInputRate::VERTEX) // ??
    }

    pub(crate) fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 3] {
        [vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0, // Index of the vertex binding
            format: vk::Format::R32G32B32_SFLOAT, // Describes a vec3 of 32 bit floating point numbers, not a color
            offset: offset_of!(Vertex, pos) as u32 // Offset of this attribute within this binding entry
        }, vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: offset_of!(Vertex, color) as u32
        },
        vk::VertexInputAttributeDescription {
            location: 2,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: offset_of!(Vertex, tex_coord) as u32
        }]
    }
}

impl VertexBuffer {
    pub fn new(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer, cmd_pool: vk::CommandPool, vertices: &[Vertex]) -> VertexBuffer {
        let data_size: vk::DeviceSize = mem::size_of_val(vertices) as vk::DeviceSize;
        let vertex_count = vertices.len();

        let (transfer_mem, transfer_buffer) = create_buffer(core,
                                                            physical_layer,
                                                            logical_layer,
                                                            data_size,
                                                            vk::BufferUsageFlags::TRANSFER_SRC, // Can be a used as a source for transfer commands
                                                            vk::MemoryPropertyFlags::HOST_VISIBLE | // Visible for writes on the host
                                                                vk::MemoryPropertyFlags::HOST_COHERENT) // COHERENT means that copy operations are atomic with respect to subsequent vkQueueSubmit calls
            .expect("Failed to locate suitable device memory");

        unsafe {
            let dev_memory = logical_layer.logical_device
                .map_memory(transfer_mem,
                            0,
                            data_size,
                            vk::MemoryMapFlags::empty())
                .unwrap() as *mut Vertex;
            dev_memory.copy_from_nonoverlapping(vertices.as_ptr(), vertex_count);
            logical_layer.logical_device.unmap_memory(transfer_mem);
        }

        let (dev_mem, buf) = create_buffer(core,
                                           physical_layer,
                                                        logical_layer,
                                                        data_size,
                                                        vk::BufferUsageFlags::VERTEX_BUFFER | // Used by the vertex shader stage
                                                            vk::BufferUsageFlags::TRANSFER_DST, // Can be a destination for transfer commands
                                                        vk::MemoryPropertyFlags::DEVICE_LOCAL) // Local to GPU
            .expect("Failed to locate suitable device memory");

        copy_buffer(logical_layer, cmd_pool, transfer_buffer, buf, data_size);

        let vbuf = VertexBuffer {
            buf,
            dev_mem
        };

        unsafe {
            logical_layer.logical_device.destroy_buffer(transfer_buffer, None);
            logical_layer.logical_device.free_memory(transfer_mem, None);
        }

        vbuf
    }

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            logical_layer.logical_device.destroy_buffer(self.buf, None);
            logical_layer.logical_device.free_memory(self.dev_mem, None);
        }
    }
}