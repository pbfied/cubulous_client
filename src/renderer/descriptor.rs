use ash::vk;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::ubo::{create_descriptor_set_layout, UniformBuffer, UniformBufferObject};

pub(crate) struct Descriptor {
    pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    pub(crate) sets: Vec<vk::DescriptorSet>
}

impl Descriptor {
    pub(crate) fn new(logical_layer: &LogicalLayer, ubo: &UniformBuffer, max_frames: usize) -> Descriptor {
        // Build descriptor pool
        let pool_size = [vk::DescriptorPoolSize::default()
            .descriptor_count(max_frames as u32)
            .ty(vk::DescriptorType::UNIFORM_BUFFER)];
        let pool_create_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(max_frames as u32)
            .pool_sizes(&pool_size);
        let pool = unsafe { logical_layer.logical_device.create_descriptor_pool(&pool_create_info, None).unwrap() };

        // Build descriptor set layout
        let layout = create_descriptor_set_layout(logical_layer);
        let mut layout_vec: Vec<vk::DescriptorSetLayout> = Vec::new();
        for _ in 0..max_frames {
            layout_vec.push(layout);
        }

        // Build descriptor set
        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool)
            .set_layouts(layout_vec.as_slice());
        let mut sets: Vec<vk::DescriptorSet> = unsafe { logical_layer.logical_device.allocate_descriptor_sets(&allocate_info).unwrap() };

        for (set, buffer) in sets.iter().zip(ubo.data.iter()) {
            let buffer_info = [vk::DescriptorBufferInfo::default()
                .offset(0) // The Src buffer index to update from
                .buffer(*buffer) // The Src buffer to update the descriptor set from
                .range(std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize)]; // Can also use VK_WHOLE_SIZE if updating the entire range
            let descriptor_write = [vk::WriteDescriptorSet::default()
                .buffer_info(&buffer_info)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .dst_array_element(0) // The descriptor set can describe an array of elements
                .dst_binding(0) // The location in the target buffer to update
                .dst_set(*set)]; // The target descriptor set to update

            unsafe {
                logical_layer.logical_device.update_descriptor_sets(&descriptor_write, &[]);
            }
        }

        Descriptor {
            pool,
            layout,
            sets
        }
    }

    pub(crate) fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            logical_layer.logical_device.destroy_descriptor_pool(self.pool, None);
            logical_layer.logical_device.destroy_descriptor_set_layout(self.layout, None);

        }
    }
}

