use ash::vk;
use crate::logical_layer::LogicalLayer;
use crate::texture::Texture;
use crate::ubo::{UniformBuffer, UniformBufferObject};

// Use Ash builtin to destroy the descriptor set layout
pub fn create_descriptor_set_layout(logical_layer: &LogicalLayer) -> vk::DescriptorSetLayout {
    let transform_binding = vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::VERTEX);

    let sampler_layout_binding = vk::DescriptorSetLayoutBinding::default()
        .binding(1)
        .descriptor_count(1)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT);

    let binding_arr = [transform_binding, sampler_layout_binding];

    let layout = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(&binding_arr)
        .flags(vk::DescriptorSetLayoutCreateFlags::empty());

    unsafe {
        logical_layer.logical_device.create_descriptor_set_layout(&layout, None).unwrap()
    }
}

pub struct Descriptor {
    pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    pub sets: Vec<vk::DescriptorSet>
}

impl Descriptor {
    pub fn new(logical_layer: &LogicalLayer, ubo: &UniformBuffer, sampler: vk::Sampler,
               texture: &Texture, layout: vk::DescriptorSetLayout, max_frames: usize) -> Descriptor {
        // Build descriptor pool
        let transform_pool_size = vk::DescriptorPoolSize::default()
            .descriptor_count(max_frames as u32)
            .ty(vk::DescriptorType::UNIFORM_BUFFER);
        let texture_sampler_pool_size = vk::DescriptorPoolSize::default()
            .descriptor_count(max_frames as u32)
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER);

        let pool_size = [transform_pool_size, texture_sampler_pool_size];
        let pool_create_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(max_frames as u32)
            .pool_sizes(&pool_size);
        let pool = unsafe { logical_layer.logical_device.create_descriptor_pool(&pool_create_info, None).unwrap() };

        let mut layout_vec: Vec<vk::DescriptorSetLayout> = Vec::new();
        for _ in 0..max_frames {
            layout_vec.push(layout);
        }

        // Build descriptor set
        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool)
            .set_layouts(layout_vec.as_slice());
        let sets: Vec<vk::DescriptorSet> = unsafe { logical_layer.logical_device.allocate_descriptor_sets(&allocate_info).unwrap() };

        for (set, buffer) in sets.iter().zip(ubo.data.iter()) {
            let transform_buffer_info = vk::DescriptorBufferInfo::default()
                .offset(0) // The Src buffer index to update from
                .buffer(*buffer) // The Src buffer to update the descriptor set from
                .range(std::mem::size_of::<UniformBufferObject>() as vk::DeviceSize);
            let buffer_info = [transform_buffer_info]; // Can also use VK_WHOLE_SIZE if updating the entire range
            let transform_desc_write = vk::WriteDescriptorSet::default() // The target descriptor set to update
                .buffer_info(&buffer_info)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .dst_array_element(0) // The descriptor set can describe an array of elements
                .dst_binding(0) // The location in the target buffer to update
                .dst_set(*set);

            let image_info = vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(texture.view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let image_info_array = [image_info];
            let image_info_write = vk::WriteDescriptorSet::default()
                .dst_set(*set)
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_info_array);

            let descriptor_write = [transform_desc_write, image_info_write];

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

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            logical_layer.logical_device.destroy_descriptor_pool(self.pool, None);
            logical_layer.logical_device.destroy_descriptor_set_layout(self.layout, None);

        }
    }
}

