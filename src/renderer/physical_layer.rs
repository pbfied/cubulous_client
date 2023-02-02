use std::ffi::{c_char, CStr, CString};

use ash::{vk, Instance};

use crate::renderer::core::Core;

pub(crate) struct PhysicalLayer {
    pub(crate)physical_device: vk::PhysicalDevice,
    pub(crate) family_index: u32,
    pub(crate) supported_surface_formats: Vec<vk::SurfaceFormatKHR>,
    pub(crate) present_modes: Vec<vk::PresentModeKHR>
}

impl PhysicalLayer {
    pub fn new(core: &Core, required_extensions: &Vec<CString>) -> Option<PhysicalLayer> {
        fn required_physical_extensions_present(instance: &Instance,
                                                physical_device: vk::PhysicalDevice,
                                                required_extensions: &Vec<CString>) -> bool {
            let dev_extensions: Vec<&str>;
            unsafe {
                dev_extensions = instance
                    .enumerate_device_extension_properties(physical_device)
                    .unwrap()
                    .iter()
                    .map(|i| CStr::from_ptr(i.extension_name.as_ptr()).to_str().unwrap())
                    .collect();
            }

            println!("\nDevice extensions:");
            for e in dev_extensions.clone() {
                println!("{}", e);
            }

            required_extensions.iter()
                .all(|e| dev_extensions.contains(&e.to_str().unwrap()))
        }

        let physical_devices: Vec<vk::PhysicalDevice>;
        unsafe {
            physical_devices = core.instance.enumerate_physical_devices().unwrap();
        }

        // Get the first physical device that satisfies the suitability check
        // Suitability requirements:
        // - Discrete GPU
        // - Geometry shaders
        // - supports these logical requirements:
        //      - Graphics pipelines
        //      - Can present images to the window manager surface
        let mut queue_family_idx = 0;
        let mut dev_found = false;
        let mut dev_idx: usize = 0;
        let mut present_modes: Vec<vk::PresentModeKHR> = vec![];
        let mut surface_formats: Vec<vk::SurfaceFormatKHR> = vec![];

        // For each physical device
        for (p_idx, device) in physical_devices.iter().enumerate() {
            let dev_properties: vk::PhysicalDeviceProperties;
            let dev_features: vk::PhysicalDeviceFeatures;
            unsafe {
                dev_properties = core.instance.get_physical_device_properties(*device);
                dev_features = core.instance.get_physical_device_features(*device);
            }

            // Ensure that at least one kind of surface color/pixel format is supported
            unsafe {
                surface_formats = core.surface_loader
                    .get_physical_device_surface_formats(*device, core.surface).unwrap();
                // Ensure that the desired FIFO format for pushing images to the screen is available
                present_modes = core.surface_loader
                    .get_physical_device_surface_present_modes(*device, core.surface).unwrap();
            }

            let mut queue_found = false;
            if required_physical_extensions_present(&core.instance,
                                                    *device,
                                                    required_extensions) &&
                !present_modes.is_empty() &&
                !surface_formats.is_empty() &&
                dev_features.sampler_anisotropy == vk::TRUE {
                let queue_families: Vec<vk::QueueFamilyProperties>;
                unsafe {
                    queue_families = core.instance
                        .get_physical_device_queue_family_properties(*device);
                }

                let queue_fam_enumerator = queue_families.iter().enumerate();

                // For each Queue family associated with a given device
                for (idx, qf) in queue_fam_enumerator {
                    // Check for graphics support
                    let graphics_support =
                        (qf.queue_flags & vk::QueueFlags::GRAPHICS) == vk::QueueFlags::GRAPHICS;
                    if graphics_support {
                        // Check family suitability
                        let idx_u32 = idx as u32;
                        let surface_support: bool;
                        unsafe {
                            surface_support = core.surface_loader
                                .get_physical_device_surface_support(*device, idx as u32, core.surface)
                                .unwrap();
                        }
                        if surface_support {
                            queue_family_idx = idx_u32;
                            queue_found = true;
                            break;
                        }
                    }
                }
            }

            // If the queue family and the device are suitable
            if queue_found
                && dev_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
                && dev_features.geometry_shader != 0
            {
                dev_found = true;
                dev_idx = p_idx;
                break; // Done
            }
        }

        if dev_found {
            let physical_dependencies = PhysicalLayer {
                physical_device: physical_devices[dev_idx],
                family_index: queue_family_idx,
                present_modes,
                supported_surface_formats: surface_formats
            };
            Some(physical_dependencies)
        } else {
            None
        }
    }
}