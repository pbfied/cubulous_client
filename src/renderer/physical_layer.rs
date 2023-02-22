use std::arch::x86_64::_bextr_u32;
use std::ffi::{CStr, CString};
use ash::{vk, Instance, extensions::khr};

use crate::renderer::core::Core;

pub struct PhysicalLayer {
    pub(crate)physical_device: vk::PhysicalDevice,
    pub present_family_index: u32,
    pub graphics_family_index: u32,
    pub(crate) supported_surface_formats: Vec<vk::SurfaceFormatKHR>,
    pub(crate) present_modes: Vec<vk::PresentModeKHR>,
    pub max_msaa_samples: vk::SampleCountFlags
}

fn get_max_usable_sample_count(properties: &vk::PhysicalDeviceProperties) -> vk::SampleCountFlags {
    let counts = properties.limits.framebuffer_color_sample_counts &
        properties.limits.framebuffer_depth_sample_counts;

    let retval: vk::SampleCountFlags;
    if (counts & vk::SampleCountFlags::TYPE_64) == vk::SampleCountFlags::TYPE_64 {
        retval = vk::SampleCountFlags::TYPE_64;
    }
    else if (counts & vk::SampleCountFlags::TYPE_32) == vk::SampleCountFlags::TYPE_32 {
        retval = vk::SampleCountFlags::TYPE_32;
    }
    else if (counts & vk::SampleCountFlags::TYPE_16) == vk::SampleCountFlags::TYPE_16 {
        retval = vk::SampleCountFlags::TYPE_16;
    }
    else if (counts & vk::SampleCountFlags::TYPE_8) == vk::SampleCountFlags::TYPE_8 {
        retval = vk::SampleCountFlags::TYPE_8;
    }
    else if (counts & vk::SampleCountFlags::TYPE_4) == vk::SampleCountFlags::TYPE_4 {
        retval = vk::SampleCountFlags::TYPE_4;
    }
    else if (counts & vk::SampleCountFlags::TYPE_2) == vk::SampleCountFlags::TYPE_2 {
        retval = vk::SampleCountFlags::TYPE_2;
    }
    else {
        retval = vk::SampleCountFlags::TYPE_1;
    }

    retval
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
        let mut present_family_index: u32 = 0;
        let mut graphics_family_index: u32 = 0;
        let mut present_family_found = false;
        let mut graphics_family_found = false;
        let mut dev_found = false;
        let mut dev_idx: usize = 0;
        let mut present_modes: Vec<vk::PresentModeKHR> = vec![];
        let mut surface_formats: Vec<vk::SurfaceFormatKHR> = vec![];
        let mut max_msaa_samples: vk::SampleCountFlags = vk::SampleCountFlags::TYPE_1;

        // For each physical device
        for (p_idx, device) in physical_devices.iter().enumerate() {
            let dev_properties: vk::PhysicalDeviceProperties;
            let dev_features: vk::PhysicalDeviceFeatures;
            let mut rt_features: vk::PhysicalDeviceRayTracingPipelineFeaturesKHR =
                vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
            let mut buf_features = vk::PhysicalDeviceBufferDeviceAddressFeaturesEXT::default();
            let mut features2 = vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut rt_features)
                .push_next(&mut buf_features);
            unsafe {
                dev_properties = core.instance.get_physical_device_properties(*device);
                dev_features = core.instance.get_physical_device_features(*device);
                core.instance.get_physical_device_features2(*device, &mut features2);
            }

            // Ensure that at least one kind of surface color/pixel format is supported
            unsafe {
                surface_formats = core.surface_loader
                    .get_physical_device_surface_formats(*device, core.surface).unwrap();
                // Ensure that the desired FIFO format for pushing images to the screen is available
                present_modes = core.surface_loader
                    .get_physical_device_surface_present_modes(*device, core.surface).unwrap();
            }

            let mut all_queues_found = false;
            if required_physical_extensions_present(&core.instance, *device, required_extensions) &&
                !present_modes.is_empty() && !surface_formats.is_empty() && dev_features.sampler_anisotropy ==
                vk::TRUE && rt_features.ray_tracing_pipeline == vk::TRUE &&
                buf_features.buffer_device_address == vk::TRUE {
                let queue_families: Vec<vk::QueueFamilyProperties>;
                unsafe {
                    queue_families = core.instance
                        .get_physical_device_queue_family_properties(*device);
                }

                let queue_fam_enumerator = queue_families.iter().enumerate();

                // For each Queue family associated with a given device
                for (idx, qf) in queue_fam_enumerator {
                    if !graphics_family_found {
                        // Check for graphics support
                        let graphics_support =
                            (qf.queue_flags & vk::QueueFlags::GRAPHICS) == vk::QueueFlags::GRAPHICS;
                        if graphics_support {
                            graphics_family_index = idx as u32;
                            graphics_family_found = true;
                        }
                    }

                    if !present_family_found {
                        let surface_support =
                            unsafe {
                                core.surface_loader
                                    .get_physical_device_surface_support(*device,
                                                                         idx as u32,
                                                                         core.surface).unwrap()
                            };

                        if surface_support {
                            present_family_index = idx as u32;
                            present_family_found = true;
                        }
                    }

                    if present_family_found && graphics_family_found {
                        all_queues_found = true;
                        break;
                    }
                }
            }

            // If the queue family and the device are suitable
            if all_queues_found
                && dev_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU
                && dev_features.geometry_shader != 0
            {
                dev_found = true;
                dev_idx = p_idx;
                max_msaa_samples = get_max_usable_sample_count(&dev_properties);
                break; // Done
            }
        }

        if dev_found {
            let physical_dependencies = PhysicalLayer {
                physical_device: physical_devices[dev_idx],
                present_family_index,
                graphics_family_index,
                present_modes,
                supported_surface_formats: surface_formats,
                max_msaa_samples
            };
            Some(physical_dependencies)
        } else {
            None
        }
    }
}