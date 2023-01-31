use std::mem;
use std::time::{Duration, Instant};

use ash::vk;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::staging_buf::{create_buffer, copy_buffer};
use cgmath::{Matrix4, Deg, Point3, Vector3, perspective, SquareMatrix};
use crate::renderer::render_target::RenderTarget;

// Remember to align fields according to the Vulkan specification
#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub(crate) struct UniformBufferObject {
    model: Matrix4<f32>,
    view: Matrix4<f32>,
    proj: Matrix4<f32>
}

pub(crate) struct  UniformBuffer {
    pub(crate) data: Vec<vk::Buffer>,
    mem: Vec<vk::DeviceMemory>,
    mapped: Vec<*mut UniformBufferObject>,
    start_time: Instant
}

impl UniformBuffer {
    pub(crate) fn new(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer, max_frames: usize) -> UniformBuffer {
        let buffer_size: vk::DeviceSize = mem::size_of::<UniformBufferObject>() as vk::DeviceSize;
        let start_time = Instant::now();
        let mut uniform_buffer: UniformBuffer = UniformBuffer {
            data: vec![],
            mem: vec![],
            mapped: vec![],
            start_time
        };

        for _ in 0..max_frames {
            let (buf_mem, buffer) = create_buffer(core, physical_layer, logical_layer, buffer_size,
                                                  vk::BufferUsageFlags::UNIFORM_BUFFER,
                                                  vk::MemoryPropertyFlags::HOST_COHERENT |
                                                      vk::MemoryPropertyFlags::HOST_VISIBLE).unwrap();
            uniform_buffer.mem.push(buf_mem);
            uniform_buffer.data.push(buffer);

            let dev_memory: *mut UniformBufferObject;
            unsafe {
                dev_memory = logical_layer.logical_device
                    .map_memory(buf_mem,
                                0,
                                buffer_size,
                                vk::MemoryMapFlags::empty())
                    .unwrap() as *mut UniformBufferObject;
            }
            uniform_buffer.mapped.push(dev_memory);
        }

        uniform_buffer
    }

    pub(crate) fn build_transforms(&self, render_target: &RenderTarget, current_frame: usize) {
        let current_time = Instant::now();
        let time = current_time.duration_since(self.start_time).as_millis() as f32 / 1000.0;

        let transform_matrices = [UniformBufferObject {
            model: Matrix4::from_angle_z(Deg(90.0 * time)),
            view: Matrix4::look_at_rh(Point3::new(2.0, 2.0, 2.0),
                                      Point3::new(0.0, 0.0, 0.0),
                                      Vector3::new(0.0, 0.0, 1.0)),
            proj: perspective(Deg(45.0), (render_target.extent.width as f32) / (render_target.extent.height as f32), 0.1, 10.0)
        }];

        unsafe {
            self.mapped[current_frame].copy_from_nonoverlapping(transform_matrices.as_ptr(), transform_matrices.len());
        }
    }

    pub(crate) fn destroy(&self, logical_layer: &LogicalLayer) {
        for (buf, mem) in self.data.iter().zip(self.mem.iter()) {
            unsafe {
                logical_layer.logical_device.destroy_buffer(*buf, None);
                logical_layer.logical_device.free_memory(*mem, None);
            }
        }
    }
}

// Use Ash builtin to destroy the descriptor set layout
pub(crate) fn create_descriptor_set_layout(logical_layer: &LogicalLayer) -> vk::DescriptorSetLayout {
    let binding = vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::VERTEX);

    let binding_arr = [binding];

    let layout = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(&binding_arr);

    unsafe {
        logical_layer.logical_device.create_descriptor_set_layout(&layout, None).unwrap()
    }
}