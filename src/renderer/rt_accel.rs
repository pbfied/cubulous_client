use std::mem;
use ash::vk;
use ash::vk::AccelerationStructureBuildGeometryInfoKHR;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::rt_index::IndexBuffer;
use crate::renderer::staging_buf::{copy_buffer, create_buffer};

pub struct AccelBuffer {
    pub buf: vk::Buffer,
    dev_mem: vk::DeviceMemory,
    pub item_count: u32
}

impl AccelBuffer {
    pub fn new<T>(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer, cmd_pool: vk::CommandPool,
                  indices: &[T]) -> AccelBuffer {
        let data_size: vk::DeviceSize = (mem::<T>::size_of() * indices.len()) as vk::DeviceSize;
        let index_count = indices.len();

        let (transfer_mem, transfer_buffer) = create_buffer(core,
                                                            physical_layer,
                                                            logical_layer,
                                                            data_size,
                                                            vk::BufferUsageFlags::TRANSFER_SRC,
                                                            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)
            .unwrap();

        unsafe {
            let dev_memory = logical_layer.logical_device
                .map_memory(transfer_mem,
                            0,
                            data_size,
                            vk::MemoryMapFlags::empty())
                .unwrap() as *mut T;
            dev_memory.copy_from_nonoverlapping(indices.as_ptr(), index_count);
            logical_layer.logical_device.unmap_memory(transfer_mem);
        }

        let (dev_mem, buf) = create_buffer(core,
                                           physical_layer,
                                           logical_layer,
                                           data_size,
                                           vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR | // Used by the vertex shader stage
                                               vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS |
                                               vk::BufferUsageFlags::TRANSFER_DST, // Can be a destination for transfer commands
                                           vk::MemoryPropertyFlags::DEVICE_LOCAL) // Local to GPU
            .expect("Failed to locate suitable device memory");

        copy_buffer(logical_layer, cmd_pool, transfer_buffer, buf, data_size);

        let ibuf = AccelBuffer {
            buf,
            dev_mem,
            item_count: index_count as u32
        };

        unsafe {
            logical_layer.logical_device.destroy_buffer(transfer_buffer, None);
            logical_layer.logical_device.free_memory(transfer_mem, None);
        }

        ibuf
    }

    pub fn get_device_address(&self, logical_layer: &LogicalLayer) -> vk::DeviceAddress {
        let addr_info = vk::BufferDeviceAddressInfo::default()
            .buffer(self.buf);
        unsafe {
            logical_layer.logical_device.get_buffer_device_address(&addr_info)
        }
    }

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            logical_layer.logical_device.destroy_buffer(self.buf, None);
            logical_layer.logical_device.free_memory(self.dev_mem, None);
        }
    }
}

pub fn temp(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer, command_pool: vk::CommandPool) {
    // Clockwise, top to bottom, back to front
    // 0    1 - back    4   5
    // 2    3           6   7
    let acceleration_instance = ash::extensions::khr::AccelerationStructure::new(&core.instance, &logical_layer
        .logical_device);

    let indices: [u8; 36] = [
        0, 1, 2, // back
        1, 3, 2,
        0, 1, 5, // top
        0, 5, 4,
        1, 3, 7, // right
        1, 7, 5,
        2, 3, 7, // bottom
        2, 7, 6,
        0, 4, 6, // left
        0, 6, 2,
        4, 5, 7, // front
        4, 7, 6
    ];
    let vertices: [f32; 24] = [
        -0.5, 0.5, -0.5,
        0.5, 0.5, -0.5,
        -0.5, -0.5, -0.5,
        0.5, -0.5, -0.5,
        -0.5, 0.5, 0.5,
        0.5, 0.5, 0.5,
        -0.5, -0.5, 0.5,
        0.5, -0.5, 0.5,
    ];

    let index_buffer = AccelBuffer::new(core, physical_layer, logical_layer, command_pool, &indices);
    let index_dev_addr = vk::DeviceOrHostAddressConstKHR {
        device_address: index_buffer.get_device_address(logical_layer)
    };
    let vertex_buffer = AccelBuffer::new(core,physical_layer, logical_layer, command_pool, &vertices);
    let vertex_dev_addr = vk::DeviceOrHostAddressConstKHR {
        device_address: vertex_buffer.get_device_address(logical_layer)
    };

    let geometry_data_triangles = vk::AccelerationStructureGeometryTrianglesDataKHR::default()
        .index_type(vk::IndexType::UINT8_EXT)
        .index_data(index_dev_addr)
        .max_vertex(7)
        .vertex_format(vk::Format::R32G32B32_SFLOAT)
        .vertex_data(vertex_dev_addr)
        .vertex_stride((mem::<f32>::size_of() * 3) as vk::DeviceSize);
    let geometry_data = vk::AccelerationStructureGeometryDataKHR {
        triangles: geometry_data_triangles
    };

    let box_opaque_geometry = [
        vk::AccelerationStructureGeometryKHR::default()
            .flags(vk::GeometryFlagsKHR::OPAQUE)
            .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
            .geometry(geometry_data)
            .flags(vk::GeometryFlagsKHR::OPAQUE)
        ];

    // let build_info = AccelerationStructureBuildGeometryInfoKHR::default()
    //     .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE_NV)
    //     .geometries(&box_opaque_geometry)
    //     .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
    //     . TODO
    let build_size = unsafe {
        acceleration_instance.get_acceleration_structure_build_sizes(vk::AccelerationStructureBuildTypeKHR::DEVICE, ) };
}