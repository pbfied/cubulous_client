use ash::vk;
use image::EncodableLayout;
use image::imageops::unsharpen;
use image::io::Reader;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::staging_buf::{create_buffer, find_buf_index, begin_single_time_commands, end_single_time_commands};
use crate::renderer::image::{create_image_view, create_image, copy_buffer_to_image, transition_image_layout};

fn create_texture_image_view(logical_layer: &LogicalLayer, image: vk::Image) -> vk::ImageView {
    create_image_view(logical_layer, image, vk::Format::R8G8B8A8_SRGB, vk::ImageAspectFlags::COLOR)
}

pub(crate) struct Texture {
    image: vk::Image,
    pub(crate) view: vk::ImageView,
    mem: vk::DeviceMemory
}

impl Texture {
    pub(crate) fn new(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer,
                      command_pool: vk::CommandPool, path: &str) -> Texture {
        let img = Reader::open(path).unwrap().decode().unwrap().to_rgba8();
        let img_bytes = img.as_bytes();
        let img_size = img.len();
        assert_eq!(img.len(), (img.width() * img.height() * 4) as usize);

        let (img_mem, img_buf) = create_buffer(core, physical_layer, logical_layer, img_size as vk::DeviceSize,
                                               vk::BufferUsageFlags::TRANSFER_SRC,
                                               vk::MemoryPropertyFlags::HOST_VISIBLE |
                                                   vk::MemoryPropertyFlags::HOST_COHERENT).unwrap();
        unsafe {
            let mapped = logical_layer
                .logical_device
                .map_memory(img_mem, 0, img_size as vk::DeviceSize, vk::MemoryMapFlags::empty())
                .unwrap() as *mut u8;
            mapped.copy_from_nonoverlapping(img_bytes.as_ptr(), img_size);
            logical_layer.logical_device.unmap_memory(img_mem);
        };

        let (texture_image, texture_mem) = create_image(core, physical_layer,
                                                        logical_layer, img.width(),
                                                        img.height(),
                                                        vk::Format::R8G8B8A8_SRGB,
                                                        vk::ImageTiling::OPTIMAL,
                                                        vk::ImageUsageFlags::TRANSFER_DST |
                                                            vk::ImageUsageFlags::SAMPLED,
                                                        vk::MemoryPropertyFlags::DEVICE_LOCAL);
        transition_image_layout(logical_layer, command_pool, texture_image,
                                vk::Format::R8G8B8A8_SRGB, vk::ImageLayout::UNDEFINED,
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL);
        copy_buffer_to_image(logical_layer, command_pool, img_buf, texture_image, img.width(), img.height());
        transition_image_layout(logical_layer, command_pool, texture_image,
                                vk::Format::R8G8B8A8_SRGB,
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

        let texture_image_view = create_texture_image_view(logical_layer, texture_image);

        unsafe {
            logical_layer.logical_device.destroy_buffer(img_buf, None);
            logical_layer.logical_device.free_memory(img_mem, None);
        }

        Texture {
            image: texture_image,
            view: texture_image_view,
            mem: texture_mem
        }
    }

    pub(crate) fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            logical_layer.logical_device.destroy_image_view(self.view, None);
            logical_layer.logical_device.destroy_image(self.image, None);
            logical_layer.logical_device.free_memory(self.mem, None);
        }
    }
}