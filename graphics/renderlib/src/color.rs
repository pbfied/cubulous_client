use ash::vk;
use crate::image::{create_image, create_image_view};
use crate::render_target::RenderTarget;
use crate::vkcore::VkCore;

pub struct Color {
    image: vk::Image,
    mem: vk::DeviceMemory,
    pub view: vk::ImageView
}

impl Color {
    pub fn new(core: &VkCore, render_target: &RenderTarget) -> Color {
        let (img, img_mem) = create_image(core, render_target.extent.width,
                                          render_target.extent.height,
                                          1, render_target.surface_format,
                                          vk::ImageTiling::OPTIMAL,
                                          vk::ImageUsageFlags::TRANSIENT_ATTACHMENT |
                                              vk::ImageUsageFlags::COLOR_ATTACHMENT,
                                          vk::MemoryPropertyFlags::DEVICE_LOCAL,
                                          core.max_msaa_samples);
        let view = create_image_view(core, img, render_target.surface_format,
                                     vk::ImageAspectFlags::COLOR, 1);

        Color {
            image: img,
            mem: img_mem,
            view,
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