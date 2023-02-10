use ash::vk;
use crate::renderer::core::Core;
use crate::renderer::image::{create_image, create_image_view};
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::render_target::RenderTarget;

pub struct Color {
    image: vk::Image,
    mem: vk::DeviceMemory,
    pub view: vk::ImageView
}

impl Color {
    pub fn new(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer,
               render_target: &RenderTarget) -> Color {
        let (img, img_mem) = create_image(core, physical_layer, logical_layer,
                                          render_target.extent.width, render_target.extent.height,
                                          1, render_target.surface_format,
                                          vk::ImageTiling::OPTIMAL,
                                          vk::ImageUsageFlags::TRANSIENT_ATTACHMENT |
                                              vk::ImageUsageFlags::COLOR_ATTACHMENT,
                                          vk::MemoryPropertyFlags::DEVICE_LOCAL,
                                          physical_layer.max_msaa_samples);
        let view = create_image_view(logical_layer, img, render_target.surface_format,
                                     vk::ImageAspectFlags::COLOR, 1);

        Color {
            image: img,
            mem: img_mem,
            view,
        }
    }

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            logical_layer.logical_device.destroy_image_view(self.view, None);
            logical_layer.logical_device.destroy_image(self.image, None);
            logical_layer.logical_device.free_memory(self.mem, None);
        }
    }
}