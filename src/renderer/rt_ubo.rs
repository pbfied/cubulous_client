use std::mem;
use ash::vk;
use cgmath::{Deg, Matrix4, perspective, Point3, Transform, Vector3};
use crate::renderer::core::Core;
use crate::renderer::gpu_buffer::create_buffer;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::render_target::RenderTarget;

// Remember to align fields according to the Vulkan specification 15.7.4
#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub(crate) struct RtUniformBufferObject {
    // model: Matrix4<f32>,
    inverse_view: Matrix4<f32>,
    inverse_proj: Matrix4<f32>
}

pub struct  RtUniformBuffer {
    pub(crate) data: Vec<vk::Buffer>,
    mem: Vec<vk::DeviceMemory>,
    mapped: Vec<*mut RtUniformBufferObject>
    // start_time: Instant
}

impl RtUniformBuffer {
    pub fn new(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer, max_frames: usize) ->
                                                                                                             RtUniformBuffer {
        let buffer_size: vk::DeviceSize = mem::size_of::<RtUniformBufferObject>() as vk::DeviceSize;
        // let start_time = Instant::now();
        let mut uniform_buffer: RtUniformBuffer = RtUniformBuffer {
            data: vec![],
            mem: vec![],
            mapped: vec![]
            // start_time
        };

        for _ in 0..max_frames {
            let (buf_mem, buffer) = create_buffer(core, physical_layer, logical_layer, buffer_size,
                                                  vk::BufferUsageFlags::UNIFORM_BUFFER,
                                                  vk::MemoryPropertyFlags::HOST_COHERENT |
                                                      vk::MemoryPropertyFlags::HOST_VISIBLE);
            uniform_buffer.mem.push(buf_mem);
            uniform_buffer.data.push(buffer);

            let dev_memory: *mut RtUniformBufferObject;
            unsafe {
                dev_memory = logical_layer.logical_device
                    .map_memory(buf_mem,
                                0,
                                buffer_size,
                                vk::MemoryMapFlags::empty())
                    .unwrap() as *mut RtUniformBufferObject;
            }
            uniform_buffer.mapped.push(dev_memory);
        }

        uniform_buffer
    }

    pub fn build_transforms(&self, render_target: &RenderTarget, current_frame: usize) {
        // let current_time = Instant::now();
        // let time = current_time.duration_since(self.start_time).as_millis() as f32 / 1000.0;
        let time = 0.0;

        let mut perspective = perspective(Deg(45.0),
                                          (render_target.extent.width as f32) /
                                              (render_target.extent.height as f32),
                                          0.1, 10.0).inverse_transform().unwrap();
        perspective.y.y *= -1.0;
        let transform_matrices = [RtUniformBufferObject {
            inverse_view: Matrix4::look_at_rh(Point3::new(2.0, 2.0, 2.0),
                                              Point3::new(0.0, 0.0, 0.0),
                                              Vector3::new(0.0, 0.0, 1.0)).inverse_transform().unwrap(),
            inverse_proj: perspective
        }];

        unsafe {
            self.mapped[current_frame].copy_from_nonoverlapping(transform_matrices.as_ptr(), transform_matrices.len());
        }
    }

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        for (buf, mem) in self.data.iter().zip(self.mem.iter()) {
            unsafe {
                logical_layer.logical_device.destroy_buffer(*buf, None);
                logical_layer.logical_device.free_memory(*mem, None);
            }
        }
    }
}
