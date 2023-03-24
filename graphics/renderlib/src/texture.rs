use std::cmp::max;
use ash::vk;
use ash::vk::Offset3D;
use image::EncodableLayout;
use image::io::Reader;
use crate::gpu_buffer::{create_buffer};
use crate::image::{create_image_view, create_image, copy_buffer_to_image, transition_image_layout};
use crate::single_time::{begin_single_time_commands, end_single_time_commands};
use crate::vkcore::VkCore;

fn create_texture_image_view(core: &VkCore, image: vk::Image, mip_levels: u32) -> vk::ImageView {
    create_image_view(core, image, vk::Format::R8G8B8A8_SRGB, vk::ImageAspectFlags::COLOR, mip_levels)
}

fn generate_mip_maps(core: &VkCore, command_pool: vk::CommandPool, image: vk::Image, image_format: vk::Format,
                     tex_width: u32, tex_height: u32, mip_levels: u32) {
    let format_properties = unsafe {
        core.instance
            .get_physical_device_format_properties(core.physical_device, image_format)
    };

    assert_ne!(format_properties.optimal_tiling_features &
                   vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR,
               vk::FormatFeatureFlags::empty());

    let cmd_buffer = begin_single_time_commands(core, command_pool);

    let mut sub_resource_range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_array_layer(0)
        .layer_count(1)
        .level_count(1);
    let mut barrier = vk::ImageMemoryBarrier::default()
        .image(image)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .subresource_range(sub_resource_range.clone());

    let mut mip_width = tex_width as i32;
    let mut mip_height = tex_height as i32;

    for i in 1..mip_levels {
        sub_resource_range = sub_resource_range.base_mip_level(i - 1);
        barrier = barrier.subresource_range(sub_resource_range.clone())
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::TRANSFER_READ);

        unsafe {
            core.logical_device
                .cmd_pipeline_barrier(cmd_buffer,
                                      vk::PipelineStageFlags::TRANSFER,
                                      vk::PipelineStageFlags::TRANSFER,
                                      vk::DependencyFlags::empty(), &[],
                                      &[], &[barrier.clone()]);
        }

        let src_sub_resource = vk::ImageSubresourceLayers::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .mip_level(i - 1)
            .base_array_layer(0)
            .layer_count(1);
        let dest_sub_resource = vk::ImageSubresourceLayers::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .mip_level(i)
            .base_array_layer(0)
            .layer_count(1);
        let blit = vk::ImageBlit::default()
            .src_offsets([Offset3D {
                x: 0,
                y: 0,
                z: 0
            }, Offset3D {
                x: mip_width,
                y: mip_height,
                z: 1
            }])
            .src_subresource(src_sub_resource.clone())
            .dst_offsets([Offset3D {
                x: 0,
                y: 0,
                z: 0,
            }, Offset3D {
                x: max(mip_width / 2, 1),
                y:  max(mip_height / 2, 1),
                z: 1,
            }])
            .dst_subresource(dest_sub_resource);

        unsafe { core.logical_device.cmd_blit_image(cmd_buffer, image,
                                                             vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                                                             image,
                                                             vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                                             &[blit], vk::Filter::LINEAR); }

        barrier = barrier.old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_READ)
            .dst_access_mask(vk::AccessFlags::SHADER_READ);

        unsafe {
            core.logical_device.cmd_pipeline_barrier(cmd_buffer,
                                                              vk::PipelineStageFlags::TRANSFER,
                                                              vk::PipelineStageFlags::FRAGMENT_SHADER,
                                                              vk::DependencyFlags::empty(),
                                                              &[], &[],
                                                              &[barrier.clone()]);
        }

        mip_width = max(mip_width / 2, 1);
        mip_height = max(mip_height / 2, 1);
    }

    sub_resource_range = sub_resource_range.base_mip_level(mip_levels - 1);
    barrier = barrier
        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .dst_access_mask(vk::AccessFlags::SHADER_READ)
        .subresource_range(sub_resource_range);

    unsafe {
        core.logical_device.cmd_pipeline_barrier(cmd_buffer,
                                                          vk::PipelineStageFlags::TRANSFER,
                                                          vk::PipelineStageFlags::FRAGMENT_SHADER,
                                                          vk::DependencyFlags::empty(),
                                                          &[], &[],
                                                          &[barrier.clone()]);
    }

    end_single_time_commands(core, command_pool, cmd_buffer);
}

pub struct Texture {
    image: vk::Image,
    pub(crate) view: vk::ImageView,
    mem: vk::DeviceMemory,
    pub mip_levels: u32
}

impl Texture {
    pub fn new(core: &VkCore, command_pool: vk::CommandPool, path: &str) -> Texture {
        let img = Reader::open(path).unwrap().decode().unwrap().to_rgba8();
        let img_bytes = img.as_bytes();
        let img_size = img.len();
        assert_eq!(img.len(), (img.width() * img.height() * 4) as usize);

        let (img_mem, img_buf) = create_buffer(core, img_size as vk::DeviceSize,
                                               vk::BufferUsageFlags::TRANSFER_SRC,
                                               vk::MemoryPropertyFlags::HOST_VISIBLE |
                                                   vk::MemoryPropertyFlags::HOST_COHERENT);
        unsafe {
            let mapped = core.logical_device.map_memory(img_mem, 0, img_size as vk::DeviceSize,
                                                        vk::MemoryMapFlags::empty()).unwrap() as *mut u8;
            mapped.copy_from_nonoverlapping(img_bytes.as_ptr(), img_size);
            core.logical_device.unmap_memory(img_mem);
        };

        let mip_levels = ((img.height().max(img.width()) as f64).log(2.0).floor() as u32) + 1;

        let (texture_image, texture_mem) = create_image(core, img.width(),
                                                        img.height(),
                                                        mip_levels,
                                                        vk::Format::R8G8B8A8_SRGB,
                                                        vk::ImageTiling::OPTIMAL,
                                                        vk::ImageUsageFlags::TRANSFER_DST |
                                                            vk::ImageUsageFlags::TRANSFER_SRC |
                                                            vk::ImageUsageFlags::SAMPLED,
                                                        vk::MemoryPropertyFlags::DEVICE_LOCAL,
                                                        vk::SampleCountFlags::TYPE_1);
        transition_image_layout(core, command_pool, texture_image,
                                vk::Format::R8G8B8A8_SRGB, vk::ImageLayout::UNDEFINED,
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL, mip_levels);
        copy_buffer_to_image(core, command_pool, img_buf, texture_image,
                             img.width(), img.height());
        // transition_image_layout(logical_layer, command_pool, texture_image,
        //                         vk::Format::R8G8B8A8_SRGB,
        //                         vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        //                         vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, mip_levels);
        generate_mip_maps(core, command_pool, texture_image,
                          vk::Format::R8G8B8A8_SRGB, img.width(),
                          img.height(), mip_levels);

        let texture_image_view = create_texture_image_view(core, texture_image, mip_levels);

        unsafe {
            core.logical_device.destroy_buffer(img_buf, None);
            core.logical_device.free_memory(img_mem, None);
        }

        Texture {
            image: texture_image,
            view: texture_image_view,
            mem: texture_mem,
            mip_levels
        }
    }

    pub fn destroy(&self, core: &VkCore) {
        unsafe {
            core.logical_device.destroy_image_view(self.view, None);
            core.logical_device.destroy_image(self.image, None);
            core.logical_device.free_memory(self.mem, None);
        }
    }
}