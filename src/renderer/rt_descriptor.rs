use ash::vk;
use ash::vk::DescriptorSet;
use image::imageops::unsharpen;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::render_target::RenderTarget;
use crate::renderer::rt_canvas::RtCanvas;

pub fn create_per_frame_descriptor_set_layout(logical_layer: &LogicalLayer) -> vk::DescriptorSetLayout {
    let binding_arr = [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
    ];

    let layout = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(&binding_arr)
        .flags(vk::DescriptorSetLayoutCreateFlags::empty());

    unsafe {
        logical_layer.logical_device.create_descriptor_set_layout(&layout, None).unwrap()
    }
}

pub fn create_singleton_descriptor_set_layout(logical_layer: &LogicalLayer) -> vk::DescriptorSetLayout {
    let binding_arr = [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
    ];

    let layout = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(&binding_arr)
        .flags(vk::DescriptorSetLayoutCreateFlags::empty());

    unsafe {
        logical_layer.logical_device.create_descriptor_set_layout(&layout, None).unwrap()
    }
}

pub fn create_descriptor_sets(logical_layer: &LogicalLayer, canvas: &RtCanvas, per_frame: vk::DescriptorSetLayout,
                              singleton: vk::DescriptorSetLayout, max_frames: usize) -> Vec<DescriptorSet> {
    let pool_sizes = [
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_IMAGE)
            .descriptor_count(max_frames as u32),
        vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
            .descriptor_count(1)
    ];

    let pool_create_info = vk::DescriptorPoolCreateInfo::default()
        .max_sets((max_frames + 1) as u32)
        .pool_sizes(&pool_sizes);

    let descriptor_pool = unsafe {
        logical_layer.logical_device.create_descriptor_pool(&pool_create_info, None).unwrap()
    };

    let mut layout_vec: Vec<vk::DescriptorSetLayout> = Vec::new();
    for _ in 0..max_frames {
        layout_vec.push(per_frame);
    }
    layout_vec.push(singleton);

    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(layout_vec.as_slice());
    let descriptor_sets = unsafe {
        logical_layer.logical_device.allocate_descriptor_sets(&allocate_info).unwrap()
    };

    // Update the per frame descriptors
    for f in 0..max_frames {
        let image_info = [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::GENERAL)
            .image_view(*canvas.views.get(f).unwrap())];

        let write_descriptor_set = vk::WriteDescriptorSet::default()
            .dst_set(descriptor_sets[f])
            .dst_array_element(0)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(&image_info);

        unsafe {
            logical_layer.logical_device.update_descriptor_sets(&[write_descriptor_set], &[]);
        }

    }

    descriptor_sets
}



