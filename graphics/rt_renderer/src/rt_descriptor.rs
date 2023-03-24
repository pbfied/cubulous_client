use ash::vk;
use ash::vk::AccelerationStructureKHR;
use renderlib::vkcore::VkCore;
use crate::rt_accel::RtTlas;
use crate::rt_canvas::RtCanvas;
use crate::rt_pipeline::RtMissConstants;
use crate::rt_ubo::{RtPerFrameUbo, RtUniformBuffer};

pub fn create_per_frame_descriptor_set_layout(core: &VkCore) -> vk::DescriptorSetLayout {
    let binding_arr = [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .stage_flags(vk::ShaderStageFlags::RAYGEN_KHR),
        vk::DescriptorSetLayoutBinding::default()
            .binding(1)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
            .stage_flags(vk::ShaderStageFlags::RAYGEN_KHR),
        vk::DescriptorSetLayoutBinding::default()
            .binding(2)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::RAYGEN_KHR)
    ];

    let layout = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(&binding_arr)
        .flags(vk::DescriptorSetLayoutCreateFlags::empty());

    unsafe {
        core.logical_device.create_descriptor_set_layout(&layout, None).unwrap()
    }
}

pub fn create_singleton_descriptor_set_layout(core: &VkCore) -> vk::DescriptorSetLayout {
    let binding_arr = [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_count(1)
            .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
            .stage_flags(vk::ShaderStageFlags::RAYGEN_KHR)
    ];

    let layout = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(&binding_arr)
        .flags(vk::DescriptorSetLayoutCreateFlags::empty());

    unsafe {
        core.logical_device.create_descriptor_set_layout(&layout, None).unwrap()
    }
}

pub fn create_per_frame_descriptor_sets(core: &VkCore, canvas: &RtCanvas, tlas: &Vec<RtTlas>, per_frame_data: &RtUniformBuffer<RtPerFrameUbo>, per_frame_layout: vk::DescriptorSetLayout,
                                        max_frames: usize) -> (Vec<vk::DescriptorSet>, vk::DescriptorPool) { // singleton: vk::DescriptorSetLayout,
    let pool_sizes = [
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_IMAGE)
            .descriptor_count(max_frames as u32),
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
            .descriptor_count(max_frames as u32),
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(max_frames as u32)
    ];

    let pool_create_info = vk::DescriptorPoolCreateInfo::default()
        .max_sets((max_frames) as u32)
        .pool_sizes(&pool_sizes);

    let descriptor_pool = unsafe {
        core.logical_device.create_descriptor_pool(&pool_create_info, None).unwrap()
    };

    let mut layout_vec: Vec<vk::DescriptorSetLayout> = Vec::new();
    for _ in 0..max_frames {
        layout_vec.push(per_frame_layout);
    }
   // layout_vec.push(singleton);

    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(layout_vec.as_slice());
    let descriptor_sets = unsafe {
        core.logical_device.allocate_descriptor_sets(&allocate_info).unwrap()
    };

    // Update the per frame descriptors
    let mut image_infos: Vec<[vk::DescriptorImageInfo; 1]> = Vec::new();
    // let mut write_descriptor_vec: Vec<vk::WriteDescriptorSet> = Vec::new();
    for f in 0..max_frames {
        image_infos.push([vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::GENERAL)
            .image_view(*canvas.views.get(f).unwrap())]);
    }

    for f in 0..max_frames {
        let structure_slice = [tlas[f].acceleration_structure];
        let mut accel_write_set = vk::WriteDescriptorSetAccelerationStructureKHR::default()
            .acceleration_structures(&structure_slice);

        let transform_buffer_info = vk::DescriptorBufferInfo::default()
            .offset(0) // The Src buffer index to update from
            .buffer(per_frame_data.data[f]) // The Src buffer to update the descriptor set from
            .range(std::mem::size_of::<RtUniformBuffer<RtPerFrameUbo>>() as vk::DeviceSize);
        let buffer_info = [transform_buffer_info]; // Can also use VK_WHOLE_SIZE if updating the entire range

        let mut write_descriptor_set = [
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_sets[f])
                .dst_array_element(0)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .image_info(&image_infos[f]),
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_sets[f])
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                .push_next(&mut accel_write_set),
            vk::WriteDescriptorSet::default() // The target descriptor set to update
                .dst_set(descriptor_sets[f])
                .dst_binding(2) // The location in the target buffer to update
                .buffer_info(&buffer_info)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .dst_array_element(0) // The descriptor set can describe an array of elements
        ];
        write_descriptor_set[1].descriptor_count = 1; // Not set by push_next;
        unsafe {
            core.logical_device.update_descriptor_sets(&write_descriptor_set, &[]);
        }
    }

    // Singleton setup
    // let structure_slice = [tlas.acceleration_structure];
    // let mut accel_write_set = vk::WriteDescriptorSetAccelerationStructureKHR::default()
    //     .acceleration_structures(&structure_slice);
    // let mut accel_write_descriptor_set = vk::WriteDescriptorSet::default()
    //     .dst_set(descriptor_sets[max_frames])
    //     .dst_binding(0)
    //     .dst_array_element(0)
    //     .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
    //     .push_next(&mut accel_write_set);
    // accel_write_descriptor_set.descriptor_count = 1; // Not set by push_next;
    // write_descriptor_vec.push(accel_write_descriptor_set);
    // unsafe {
    //     logical_layer.logical_device.update_descriptor_sets(write_descriptor_vec.as_slice(), &[]);
    // }

    (descriptor_sets, descriptor_pool)
}

pub fn destroy_descriptor_sets(core: &VkCore, descriptor_set_layouts: &Vec<vk::DescriptorSetLayout>,
                               descriptor_pool: vk::DescriptorPool) {
    for l in descriptor_set_layouts {
        unsafe {
            core.logical_device.destroy_descriptor_set_layout(*l, None);
        }
    }
    unsafe {
         core.logical_device.destroy_descriptor_pool(descriptor_pool, None);
    }
}



