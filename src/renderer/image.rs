use ash::vk;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::staging_buf::{find_buf_index, begin_single_time_commands, end_single_time_commands};

pub(crate) fn create_image(core: &Core, physical_layer: &PhysicalLayer,logical_layer: &LogicalLayer,
                           width: u32, height: u32, mip_levels: u32,
                           format: vk::Format, tiling: vk::ImageTiling,
                usage: vk::ImageUsageFlags, properties: vk::MemoryPropertyFlags,
                           samples: vk::SampleCountFlags) -> (vk::Image, vk::DeviceMemory) {
    let image_extent = vk::Extent3D::default()
        .height(height)
        .width(width)
        .depth(1);

    let image_info = vk::ImageCreateInfo::default()
        .flags(vk::ImageCreateFlags::empty())
        .extent(image_extent)
        .mip_levels(mip_levels)
        .image_type(vk::ImageType::TYPE_2D)
        .array_layers(1)
        .format(format)
        .tiling(tiling)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .usage(usage) // Sampled allows access from shader
        .samples(samples);

    let mem_reqs: vk::MemoryRequirements;
    let texture_image: vk::Image;
    unsafe {
        texture_image = logical_layer.logical_device.create_image(&image_info,
                                                                  None).unwrap();
        mem_reqs = logical_layer.logical_device.get_image_memory_requirements(texture_image);
    }

    let alloc_info = vk::MemoryAllocateInfo::default()
        .memory_type_index(find_buf_index(core, physical_layer, properties, mem_reqs).unwrap())
        .allocation_size(mem_reqs.size);

    let texture_mem = unsafe { logical_layer.logical_device.allocate_memory(&alloc_info, None).unwrap() };
    unsafe { logical_layer.logical_device.bind_image_memory(texture_image, texture_mem, 0).unwrap() };

    (texture_image, texture_mem)
}

fn has_stencil_component(format: vk::Format) -> bool {
    format == vk::Format::D32_SFLOAT_S8_UINT || format == vk::Format::D24_UNORM_S8_UINT
}

pub(crate) fn transition_image_layout(logical_layer: &LogicalLayer,
                           command_pool: vk::CommandPool,
                           image: vk::Image,
                           format: vk::Format,
                           old_layout: vk::ImageLayout,
                           new_layout: vk::ImageLayout,
                                      mip_levels: u32) {
    let mut aspect_mask = vk::ImageAspectFlags::COLOR;
    if new_layout == vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL {
        aspect_mask = vk::ImageAspectFlags::DEPTH;

        if has_stencil_component(format) {
            aspect_mask |= vk::ImageAspectFlags::STENCIL;
        }
    }

    let subresource_range = vk::ImageSubresourceRange::default()
        .aspect_mask(aspect_mask)
        .base_mip_level(0)
        .level_count(mip_levels)
        .base_array_layer(0)
        .layer_count(1);
    let mut barrier = vk::ImageMemoryBarrier::default()
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(subresource_range)
        .dst_access_mask(vk::AccessFlags::empty());
    let source_stage;
    let dest_stage;
    if old_layout == vk::ImageLayout::UNDEFINED && new_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL {
        barrier = barrier.src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
        source_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
        dest_stage = vk::PipelineStageFlags::TRANSFER;
    }
    else if old_layout == vk::ImageLayout::TRANSFER_DST_OPTIMAL && new_layout == vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL {
        barrier = barrier.src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ);
        source_stage = vk::PipelineStageFlags::TRANSFER;
        dest_stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
    }
        else if old_layout == vk::ImageLayout::UNDEFINED &&
            new_layout == vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL {
            barrier = barrier.src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ |
                    vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE);
            source_stage = vk::PipelineStageFlags::TOP_OF_PIPE;
            dest_stage = vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS;
        }
    else {
        panic!("unsupported layout transition!");
    }

    let barrier_arr = [barrier];

    let commmand_buffer = begin_single_time_commands(logical_layer, command_pool);

    unsafe { logical_layer.logical_device.cmd_pipeline_barrier(commmand_buffer,
                                                               source_stage,
                                                               dest_stage,
                                                               vk::DependencyFlags::empty(),
                                                               &[],
                                                               &[],
                                                               &barrier_arr); }

    end_single_time_commands(logical_layer, command_pool, commmand_buffer);
}

pub(crate) fn copy_buffer_to_image(logical_layer: &LogicalLayer, command_pool: vk::CommandPool,
                        buffer: vk::Buffer, image: vk::Image, width: u32, height: u32) {
    let sub_resource_layers = vk::ImageSubresourceLayers::default()
        .mip_level(0)
        .base_array_layer(0)
        .layer_count(1)
        .aspect_mask(vk::ImageAspectFlags::COLOR);
    let image_offset = vk::Offset3D::default()
        .x(0)
        .y(0)
        .z(0);
    let image_extent = vk::Extent3D::default()
        .height(height)
        .width(width)
        .depth(1);
    let region = [vk::BufferImageCopy::default()
        .buffer_image_height(0)
        .buffer_offset(0)
        .buffer_row_length(0)
        .image_subresource(sub_resource_layers)
        .image_offset(image_offset)
        .image_extent(image_extent)];

    let command_buffer = begin_single_time_commands(logical_layer, command_pool);
    unsafe { logical_layer.logical_device.cmd_copy_buffer_to_image(command_buffer, buffer,
                                                                   image,
                                                                   vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                                                   &region); }
    end_single_time_commands(logical_layer, command_pool, command_buffer);
}

pub(crate) fn create_image_view(logical_layer: &LogicalLayer, image: vk::Image, format: vk::Format,
                                aspect_flags: vk::ImageAspectFlags, mip_levels: u32) -> vk::ImageView {
    let subresource_range = vk::ImageSubresourceRange::default()
        .aspect_mask(aspect_flags)
        .base_mip_level(0)
        .level_count(mip_levels)
        .base_array_layer(0)
        .layer_count(1);
    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(subresource_range);

    unsafe { logical_layer.logical_device
        .create_image_view(&view_info, None)
        .unwrap()
    }
}