use std::mem;
use ash::extensions::khr::AccelerationStructure;
use ash::vk;
use image::imageops::unsharpen;
use crate::renderer::core::Core;
use crate::renderer::gpu_buffer::GpuBuffer;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::single_time::{begin_single_time_commands, end_single_time_commands};

// pub const TRIANGLE_FACING_CULL_DISABLE: Self = Self(0b1);
// pub const TRIANGLE_FLIP_FACING: Self = Self(0b10);
// pub const FORCE_OPAQUE: Self = Self(0b100);
// pub const FORCE_NO_OPAQUE: Self = Self(0b1000);
// pub const TRIANGLE_FRONT_COUNTERCLOCKWISE: Self = Self::TRIANGLE_FLIP_FACING;
//
// manual conversion of vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE to u8
const MANUAL_CULL_DISABLE: u8 = 0b1;

pub struct RtAccel {
    scratch_size: vk::DeviceSize,
    accel_buf: GpuBuffer,
    scratch_buf: GpuBuffer,
    pub acceleration_structure: vk::AccelerationStructureKHR,
}

pub type RtBlas = RtAccel;
pub type RtTlas = RtAccel;

impl RtAccel {
    pub fn new_blas<T>(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer,
                       acceleration_instance: &AccelerationStructure, command_pool: vk::CommandPool,
                       indices: &[T], vertices: &[f32]) -> RtBlas {
        let index_buffer = GpuBuffer::new_initialized(core, physical_layer, logical_layer, command_pool,
                                                  vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR |
                                                      vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS, &indices);
        let index_dev_addr = vk::DeviceOrHostAddressConstKHR {
            device_address: index_buffer.get_device_address(logical_layer)
        };
        let vertex_buffer = GpuBuffer::new_initialized(core, physical_layer, logical_layer, command_pool,
                                                   vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                                                       | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                                                   &vertices);
        let vertex_dev_addr = vk::DeviceOrHostAddressConstKHR {
            device_address: vertex_buffer.get_device_address(logical_layer)
        };

        assert_eq!(vertices.len() % 3, 0);
        let index_type = match mem::size_of::<T>() {
            1 => { vk::IndexType::UINT8_EXT },
            2 => { vk::IndexType::UINT16 },
            4 => { vk::IndexType::UINT32 },
            _ => { panic!("Invalid index type") }
        };
        let geometry_data_triangles = vk::AccelerationStructureGeometryTrianglesDataKHR::default()
            .index_type(index_type)
            .index_data(index_dev_addr)
            .max_vertex((vertices.len() / 3 - 1) as u32)
            .vertex_format(vk::Format::R32G32B32_SFLOAT)
            .vertex_data(vertex_dev_addr)
            .vertex_stride((mem::size_of::<f32>() * 3) as vk::DeviceSize);
        let geometry_data = vk::AccelerationStructureGeometryDataKHR {
            triangles: geometry_data_triangles
        };

        let box_opaque_geometry = [
            vk::AccelerationStructureGeometryKHR::default()
                .flags(vk::GeometryFlagsKHR::OPAQUE)
                .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
                .geometry(geometry_data)
        ];

        let mut blas_build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .geometries(&box_opaque_geometry)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL);
        // Not documented, but the scratch field seemingly doesn't need to be filled out to get the build size
        let build_size = unsafe {
            acceleration_instance.get_acceleration_structure_build_sizes(vk::AccelerationStructureBuildTypeKHR::DEVICE,
                                                                         &blas_build_info,&[(indices.len() / 3) as u32]) };
        let scratch_size = build_size.build_scratch_size;
        let scratch_buf = GpuBuffer::new(core, physical_layer, logical_layer, scratch_size,
                                                       vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS |
                                                           vk::BufferUsageFlags::STORAGE_BUFFER); // Not sure why
                                                       // STORAGE_BUFFER property is here, but the Nvidia tutorial
                                                       // uses it

        let addr_info = vk::BufferDeviceAddressInfo::default()
            .buffer(scratch_buf.buf);
        let scratch_ptr = unsafe { logical_layer.logical_device.get_buffer_device_address(&addr_info) };

        blas_build_info = blas_build_info.scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_ptr });

        let accel_buf = GpuBuffer::new(core, physical_layer, logical_layer, build_size.acceleration_structure_size,
                                                 vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR |
                                                     vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS); // Local to GPU

        let blas_create_info = vk::AccelerationStructureCreateInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .buffer(accel_buf.buf)
            .offset(0)
            .size(build_size.acceleration_structure_size);

        let acceleration_structure = unsafe { acceleration_instance.create_acceleration_structure(&blas_create_info, None).unwrap() };
        blas_build_info = blas_build_info.dst_acceleration_structure(acceleration_structure);
        let build_range_info_l1 = [
            vk::AccelerationStructureBuildRangeInfoKHR::default()
                .first_vertex(0)
                .primitive_count((indices.len() / 3) as u32)
                .primitive_offset(0)
                .transform_offset(0)
        ];
        let build_range_info = [
            build_range_info_l1.as_slice()
        ];

        let command_buffer = begin_single_time_commands(logical_layer, command_pool);
        unsafe {
            acceleration_instance.cmd_build_acceleration_structures(command_buffer, &[blas_build_info],
                                                                    build_range_info.as_slice())
        }
        end_single_time_commands(logical_layer, command_pool, command_buffer);

        RtBlas {
            scratch_size,
            accel_buf,
            scratch_buf,
            acceleration_structure,
        }
    }

    pub fn new_tlas(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer,
                    acceleration_instance: &AccelerationStructure, command_pool: vk::CommandPool,
                    blas: &[&RtBlas]) -> RtTlas {
        let mut geometries: Vec<vk::AccelerationStructureGeometryKHR> = Vec::with_capacity(blas.len());
        // TODO Use a compute shader to construct BLAS instance arrays with different transforms
        for b in blas.iter() {
            let blas_addr_info = vk::AccelerationStructureDeviceAddressInfoKHR::default().acceleration_structure(b.acceleration_structure);
            let blas_addr = unsafe { acceleration_instance.get_acceleration_structure_device_address(&blas_addr_info) };
            let blas_ref = vk::AccelerationStructureReferenceKHR {
                device_handle: blas_addr
            };
            let index_and_mask = vk::Packed24_8::new(0, 0xFF); // No index data for now, assert all cull mask bits
            let offset_and_flags = vk::Packed24_8::new(0, MANUAL_CULL_DISABLE);
            let transform_data = vk::TransformMatrixKHR { // Identity, no translation and no transform
                matrix: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
            };
            let blas_instances = [
                vk::AccelerationStructureInstanceKHR {
                    transform: transform_data,
                    instance_custom_index_and_mask: index_and_mask,
                    instance_shader_binding_table_record_offset_and_flags: offset_and_flags,
                    acceleration_structure_reference: blas_ref
                }
            ];

            let blas_instance_buf = GpuBuffer::new_initialized(core, physical_layer, logical_layer, command_pool,
                                                           vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
                                                               | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS, &blas_instances);
            let blas_instance_addr = vk::DeviceOrHostAddressConstKHR {
                device_address: blas_instance_buf.get_device_address(logical_layer)
            };
            let tlas_geometry_instances = vk::AccelerationStructureGeometryInstancesDataKHR::default()
                .data(blas_instance_addr);
            let tlas_geometry_data = vk::AccelerationStructureGeometryDataKHR {
                instances: tlas_geometry_instances
            };
            let tlas_geometry = vk::AccelerationStructureGeometryKHR::default()
                .geometry_type(vk::GeometryTypeKHR::INSTANCES)
                .geometry(tlas_geometry_data);
            geometries.push(tlas_geometry);
        }

        let mut tlas_build_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .geometries(geometries.as_slice());
        let tlas_build_size = unsafe {
            acceleration_instance.get_acceleration_structure_build_sizes(vk::AccelerationStructureBuildTypeKHR::DEVICE,
                                                                         &tlas_build_info, &[geometries.len() as u32])
        };
        let tlas_scratch_size = tlas_build_size.build_scratch_size;
        let scratch_buf = GpuBuffer::new(core, physical_layer, logical_layer, tlas_scratch_size,
                                                       vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS |
                                                           vk::BufferUsageFlags::STORAGE_BUFFER); // Not sure why
                                                       // STORAGE_BUFFER property is here, but the Nvidia tutorial
                                                       // uses it
        let addr_info = vk::BufferDeviceAddressInfo::default()
            .buffer(scratch_buf.buf);
        let scratch_ptr = unsafe { logical_layer.logical_device.get_buffer_device_address(&addr_info) };
        tlas_build_info = tlas_build_info.scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_ptr });

        let tlas_buf = GpuBuffer::new(core, physical_layer,logical_layer,tlas_build_size.acceleration_structure_size,
                                                 vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR | // Obviously needed
                                                     // to be an AS storage location
                                                     vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS);

        let tlas_create_info = vk::AccelerationStructureCreateInfoKHR::default() // TODO commonize some of the blas/tlas
            // build code
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .size(tlas_build_size.acceleration_structure_size)
            .buffer(tlas_buf.buf)
            .offset(0);
        let tlas = unsafe { acceleration_instance.create_acceleration_structure(&tlas_create_info, None).unwrap() };
        tlas_build_info.dst_acceleration_structure(tlas);

        let build_range_info_l1 = [
            vk::AccelerationStructureBuildRangeInfoKHR::default()
                .primitive_count(blas.len() as u32)
                .primitive_offset(0)
                .transform_offset(0)
        ];
        let build_range_info = [
            build_range_info_l1.as_slice()
        ];
        let command_buffer = begin_single_time_commands(logical_layer, command_pool);
        unsafe {
            acceleration_instance.cmd_build_acceleration_structures(command_buffer, &[tlas_build_info],
                                                                    build_range_info.as_slice());
        }
        end_single_time_commands(logical_layer, command_pool, command_buffer);

        RtTlas {
            scratch_size: tlas_scratch_size,
            accel_buf: tlas_buf,
            scratch_buf,
            acceleration_structure: tlas,
        }
    }

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        self.accel_buf.destroy(logical_layer);
        self.scratch_buf.destroy(logical_layer);
    }
}

pub fn create_acceleration_structures(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer,
                                command_pool: vk::CommandPool) -> (RtTlas, RtBlas) {
    // Clockwise, top to bottom, back to front
    // 0    1 - back    4   5
    // 2    3           6   7
    let acceleration_instance = AccelerationStructure::new(&core.instance, &logical_layer
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

    let blas = RtAccel::new_blas(core, physical_layer, logical_layer, &acceleration_instance, command_pool, &indices,
                                 &vertices);
    let tlas = RtAccel::new_tlas(core, physical_layer, logical_layer, &acceleration_instance, command_pool, &[&blas]);

    (tlas, blas)
}