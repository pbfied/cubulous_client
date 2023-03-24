use ash::vk;
use crate::image::{create_image, create_image_view, transition_image_layout};
use crate::render_target::RenderTarget;
use crate::vkcore::VkCore;

pub struct Depth {
    image: vk::Image,
    mem: vk::DeviceMemory,
    pub view: vk::ImageView
}

fn find_supported_format(core: &VkCore, candidates: Vec<vk::Format>,
                         tiling: vk::ImageTiling, features: vk::FormatFeatureFlags) -> Result<vk::Format, ()> {
    let mut retval = Err(());
    for f in candidates {
        let format_props = unsafe {
            core.instance.get_physical_device_format_properties(core.physical_device, f)
        };

        if tiling == vk::ImageTiling::LINEAR && (format_props.linear_tiling_features & features) == features {
            retval = Ok(f);
            break;
        } else if tiling == vk::ImageTiling::OPTIMAL && (format_props.optimal_tiling_features & features) == features {
            retval = Ok(f);
            break;
        }
    }

    retval
}

pub fn find_depth_format(core: &VkCore) -> vk::Format {
    find_supported_format(core, Vec::from([vk::Format::D32_SFLOAT,
                              vk::Format::D32_SFLOAT_S8_UINT,
                              vk::Format::D24_UNORM_S8_UINT]),
                          vk::ImageTiling::OPTIMAL,
                          vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT).unwrap()
}

impl Depth {
    pub fn new(core: &VkCore, render_target: &RenderTarget,
               command_pool: vk::CommandPool) -> Depth {
        let format = find_depth_format(core);
        let (img, img_mem) = create_image(core,
                                          render_target.extent.width, render_target.extent.height,
                                          1, format, vk::ImageTiling::OPTIMAL,
                                          vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                                          vk::MemoryPropertyFlags::DEVICE_LOCAL,
                                          core.max_msaa_samples);
        let depth_image_view = create_image_view(core, img, format,
                                                 vk::ImageAspectFlags::DEPTH, 1);
        transition_image_layout(core, command_pool, img, format,
                                vk::ImageLayout::UNDEFINED,
                                vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, 1);

        Depth {
            image: img,
            mem: img_mem,
            view: depth_image_view
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