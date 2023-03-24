use ash::vk;
use renderlib::image::{create_image, create_image_view};
use renderlib::render_target::RenderTarget;
use renderlib::vkcore::VkCore;

pub struct RtCanvas {
    pub images: Vec<vk::Image>,
    pub views: Vec<vk::ImageView>,
    mem: Vec<vk::DeviceMemory>
}

impl RtCanvas {
    pub fn new(core: &VkCore, render_target: &RenderTarget,  max_frames: usize) -> RtCanvas {
        let mut images: Vec<vk::Image> = Vec::new();
        let mut mem: Vec<vk::DeviceMemory> = Vec::new();
        let mut views: Vec<vk::ImageView> = Vec::new();
        for _ in 0..max_frames {
            let (i, m) = create_image(core, render_target.extent.width, render_target
                .extent.height, 1, render_target.surface_format, vk::ImageTiling::OPTIMAL,
                                      vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC,
                                      vk::MemoryPropertyFlags::DEVICE_LOCAL, vk::SampleCountFlags::TYPE_1);
            let v = create_image_view(core, i, render_target.surface_format, vk::ImageAspectFlags::COLOR, 1);
            images.push(i);
            mem.push(m);
            views.push(v);
        }

        RtCanvas {
            images,
            views,
            mem
        }
    }

    pub fn destroy(&self, core: &VkCore) {
        for (&i, (&v, &m)) in self.images.iter().zip(self.views.iter().zip(self.mem.iter())) {
            unsafe {
                core.logical_device.destroy_image_view(v, None);
                core.logical_device.destroy_image(i, None);
                core.logical_device.free_memory(m, None);
            }
        }
    }
}