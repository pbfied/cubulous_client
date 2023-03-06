use num::clamp;

use ash::{vk};
use ash::extensions::khr::Swapchain;
use ash::vk::ImageView;

use winit::window::Window;

use crate::core::Core;
use crate::image::create_image_view;
use crate::logical_layer::LogicalLayer;
use crate::physical_layer::PhysicalLayer;

pub struct RenderTarget {
    pub swap_loader: Swapchain,
    pub swap_chain: vk::SwapchainKHR,
    pub(crate) surface_format: vk::Format,
    pub extent: vk::Extent2D,
    pub(crate) image_views: Vec<vk::ImageView>,
}

impl RenderTarget {
    pub fn new(core: &Core, physical_layer: &PhysicalLayer, logical_layer: &LogicalLayer, image_usage:
    vk::ImageUsageFlags, color_format: vk::Format, color_space: Option<vk::ColorSpaceKHR>) -> RenderTarget {
        fn choose_swap_extent(window: &Window, capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
            if capabilities.current_extent.width != u32::MAX {
                capabilities.current_extent
            }
            else {
                vk::Extent2D {
                    width: clamp(window.inner_size().width,
                                 capabilities.min_image_extent.width,
                                 capabilities.max_image_extent.width),
                    height: clamp(window.inner_size().height,
                                  capabilities.min_image_extent.height,
                                  capabilities.max_image_extent.height),
                }
            }
        }

        fn setup_image_views(logical_layer: &LogicalLayer, swap_loader: &Swapchain, swap_chain: vk::SwapchainKHR, surface_format: vk::Format) -> Vec<vk::ImageView> {
            let swap_chain_images: Vec<vk::Image>;
            unsafe {
                swap_chain_images = swap_loader
                    .get_swapchain_images(swap_chain).unwrap();
            }

            let mut image_views: Vec<vk::ImageView> = Vec::new();
            for i in swap_chain_images {
                image_views.push(create_image_view(logical_layer, i, surface_format,
                                                       vk::ImageAspectFlags::COLOR,
                                                   1));
            }

            return image_views;
        }

        let capabilities: vk::SurfaceCapabilitiesKHR;
        unsafe {
            capabilities = core.surface_loader
                .get_physical_device_surface_capabilities(physical_layer.physical_device,
                                                          core.surface).unwrap();
        }

        // Choose the first surface format with the specified conditions or choose the first option
        // otherwise
        let surface_format =
            match physical_layer
                .supported_surface_formats
                .iter()
                .find(|f|f.format == color_format &&
                    (if color_space.is_some() { f.color_space == color_space.unwrap() } else { true }) )
            {
                Some(x) => x,
                None => &physical_layer.supported_surface_formats[0]
            };

        let presentation_mode =
            match physical_layer
                .present_modes
                .iter()
                .find(|p|**p == vk::PresentModeKHR::MAILBOX)
            {
                Some(x) => *x,
                None => vk::PresentModeKHR::FIFO
            };

        let extent = choose_swap_extent(&core.window, &capabilities);

        let mut image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count {
            image_count = capabilities.max_image_count
        }

        let mut swap_create_info = vk::SwapchainCreateInfoKHR::default()
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1) // Always 1 except for stereoscopic 3D, I.E. VR
            .surface(core.surface)
            .image_usage(image_usage)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(presentation_mode)
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());

        let family_indices;
        if physical_layer.graphics_family_index != physical_layer.present_family_index {
            family_indices = [physical_layer.graphics_family_index, physical_layer.present_family_index];
            swap_create_info = swap_create_info
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&family_indices);
        }
        else {
            swap_create_info = swap_create_info
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE);
        }

        let swap_loader = Swapchain::new(&core.instance, &logical_layer.logical_device);
        let swap_chain: vk::SwapchainKHR;
        unsafe {
            swap_chain = swap_loader
                .create_swapchain(&swap_create_info, None).unwrap();
        }
        // Image views are only needed by the raster renderer
        let image_views = match image_usage & vk::ImageUsageFlags::COLOR_ATTACHMENT {
            vk::ImageUsageFlags::COLOR_ATTACHMENT => setup_image_views(&logical_layer,
                                                                       &swap_loader,
                                                                       swap_chain,
                                                                       surface_format.format),
            _ => Vec::<ImageView>::new()
        };

        return RenderTarget {
            swap_chain,
            swap_loader,
            surface_format: surface_format.format,
            extent,
            image_views
        }
    }

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            for &v in self.image_views.iter() {
                logical_layer.logical_device.destroy_image_view(v, None);
            }

            self.swap_loader.destroy_swapchain(self.swap_chain, None);
        }
    }
}
