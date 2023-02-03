use std::result;
use ash::vk;
use crate::renderer::core::Core;
use crate::renderer::image::{create_image, create_image_view, transition_image_layout};
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::render_target::RenderTarget;

pub(crate) struct Depth {
    image: vk::Image,
    mem: vk::DeviceMemory,
    pub(crate) view: vk::ImageView
}

fn find_supported_format(core: &Core, physical_layer: &PhysicalLayer, candidates: Vec<vk::Format>,
                         tiling: vk::ImageTiling, features: vk::FormatFeatureFlags) -> Result<vk::Format, ()> {
    let mut retval = Err(());
    for f in candidates {
        let format_props = unsafe {
            core.instance.get_physical_device_format_properties(physical_layer.physical_device, f)
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

pub(crate) fn find_depth_format(core: &Core, physical_layer: &PhysicalLayer) -> vk::Format {
    find_supported_format(core, physical_layer,
                          Vec::from([vk::Format::D32_SFLOAT,
                              vk::Format::D32_SFLOAT_S8_UINT,
                              vk::Format::D24_UNORM_S8_UINT]),
                          vk::ImageTiling::OPTIMAL,
                          vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT).unwrap()
}

impl Depth {
    pub(crate) fn new(core: &Core, physical_layer: &PhysicalLayer,
                                         logical_layer: &LogicalLayer, render_target: &RenderTarget,
                                         command_pool: vk::CommandPool) -> Depth {
        let format = find_depth_format(core, physical_layer);
        let (img, img_mem) = create_image(core, physical_layer, logical_layer,
                                          render_target.extent.width, render_target.extent.height,
                                          format, vk::ImageTiling::OPTIMAL,
                                          vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                                          vk::MemoryPropertyFlags::DEVICE_LOCAL);
        let depth_image_view = create_image_view(logical_layer, img, format,
                                                 vk::ImageAspectFlags::DEPTH);
        transition_image_layout(logical_layer, command_pool, img, format,
                                vk::ImageLayout::UNDEFINED,
                                vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        Depth {
            image: img,
            mem: img_mem,
            view: depth_image_view
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