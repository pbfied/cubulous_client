use std::env;
use std::ffi::{c_char, CStr, CString};
use std::fs::File;
use std::path::Path;
use ash::extensions::khr;
use ash::{Entry, Instance, vk, Device};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Icon, WindowBuilder, Window};

pub struct VkCore {
    _entry: Entry,
    pub window: Window,
    pub instance: Instance,
    pub(crate) surface: vk::SurfaceKHR,
    pub(crate) surface_loader: khr::Surface,
    pub physical_device: vk::PhysicalDevice,
    pub present_family_index: u32,
    pub graphics_family_index: u32,
    pub(crate) supported_surface_formats: Vec<vk::SurfaceFormatKHR>,
    pub(crate) present_modes: Vec<vk::PresentModeKHR>,
    pub max_msaa_samples: vk::SampleCountFlags,
    pub present_queue: vk::Queue,
    pub graphics_queue: vk::Queue,
    pub logical_device: Device
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

impl VkCore {
    pub fn new(ev_loop: &EventLoop<()>, required_layers: &Vec<String>, required_extensions: &Vec<CString>) -> VkCore {
        fn load_entry() -> Entry {
            let vk_lib_env = env::var("VK_LIB_PATH").unwrap();
            let vk_lib_path = Path::new(&vk_lib_env).join("libvulkan.so");

            let entry_local: Entry;
            unsafe {
                entry_local = Entry::load_from(vk_lib_path.to_str().unwrap()).unwrap();
            }

            entry_local
        }

        fn read_window_icon(path: &str) -> Option<Icon> {
            // From https://docs.rs/png/latest/png/
            let decoder = png::Decoder::new(File::open(path).unwrap()); // TODO Worry about proper asset import paths later
            let mut reader = decoder.read_info().unwrap();
            // Allocate the output buffer.
            let mut buf = vec![0; reader.output_buffer_size()];
            // Read the next frame. An APNG might contain multiple frames.
            let info = reader.next_frame(&mut buf).unwrap();
            // Grab the bytes of the image.
            let bytes = &buf[..info.buffer_size()];
            // Inspect more details of the last read frame.
            let _in_animation = reader.info().frame_control.is_some();
            let (width, height) = reader.info().size();

            Icon::from_rgba(bytes.iter().cloned().collect(), width, height).ok()
        }

        fn init_window(event_loop: &EventLoop<()>) -> Window {
            WindowBuilder::new()
                .with_title("Hello Triangle")
                .with_inner_size(LogicalSize::new(800, 600))
                .with_window_icon(read_window_icon("graphics/assets/g1141.png"))
                .build(event_loop)
                .unwrap()
        }

        fn required_layers_present(entry: &Entry, required_layers: &Vec<String>) -> bool {
            // TODO Make contingent on validation layer enable
            let vk_layers: Vec<String>;
            unsafe {
                vk_layers = entry
                    .enumerate_instance_layer_properties()
                    .unwrap()
                    .iter()
                    .map(|l| String::from(CStr::from_ptr(l.layer_name.as_ptr()).to_str().unwrap()))
                    .collect();
            }

            let mut layers_found = 0;
            for layer in required_layers {
                if vk_layers.contains(&layer) {
                    layers_found += 1;
                }
            }

            layers_found == required_layers.len()
        }

        fn required_window_extensions_present(entry: &Entry, available_extensions: &Vec<*const c_char>) -> bool {
            // Load all the vulkan functions wrapped in a struct
            let mut required_extensions: Vec<String> = Vec::new();
            let mut num_extensions_found = 0;
            let mut extensions_found = false;

            unsafe {
                println!("Winit Extensions:");
                for ext in available_extensions {
                    let c_str = CString::from(CStr::from_ptr(*ext));
                    let ext_str = c_str.to_str().unwrap();
                    let s = String::from(ext_str);
                    required_extensions.push(s);
                    println!("{}", ext_str);
                }

                // Ensure that the Vulkan instance will support the required Winit extensions
                let vk_extensions = entry.enumerate_instance_extension_properties(None).unwrap();

                println!("\nVulkan Extensions:");
                for ext in vk_extensions {
                    let ext_name = String::from(
                        CStr::from_ptr(ext.extension_name.as_ptr())
                            .to_str()
                            .unwrap(),
                    );
                    if required_extensions.binary_search(&ext_name).is_ok() {
                        num_extensions_found += 1;
                        if num_extensions_found == required_extensions.len() {
                            extensions_found = true;
                            break;
                        }
                    }
                    println!("{}", ext_name);
                }
            }

            extensions_found
        }

        fn instance_init(entry: &Entry, window: &Window, required_layers: &Vec<String>) -> Result<Instance, String> {
            // Get all the window manager extensions that Vulkan can use
            let mut winit_extensions =
                ash_window::enumerate_required_extensions(window.raw_display_handle())
                    .unwrap()
                    .to_vec();

            if required_window_extensions_present(entry, &winit_extensions) &&
                required_layers_present(entry, required_layers) {
                // TODO Work out a better way to define paths later
                let engine_name: &CStr;
                let application_name: &CStr;
                unsafe {
                    engine_name = CStr::from_bytes_with_nul_unchecked(b"Cubulous\0");
                    application_name = CStr::from_bytes_with_nul_unchecked(b"Hello Triangle\0");
                }

                // Specifies all the versions and names associated with this custom renderer
                let app_info = vk::ApplicationInfo::default()
                    .api_version(vk::make_api_version(0, 1, 3, 0))
                    .application_version(0)
                    .engine_name(engine_name)
                    .engine_version(0)
                    .application_name(application_name);

                // Required for MacOs compatibility
                winit_extensions.push(vk::KhrPortabilityEnumerationFn::name().as_ptr());
                let create_flags = vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;

                // Wrap previous stuff into a higher level struct
                let mut create_info = vk::InstanceCreateInfo::default()
                    .application_info(&app_info)
                    .enabled_extension_names(&winit_extensions)
                    // Note to self, this call fails if the validation layer related dynamic libraries are
                    // not in the same folder as libvulkan.so
                    .flags(create_flags);

                // Get validation layers
                let layer_names_raw: Vec<*const c_char>;
                let layer_names_cstring: Vec<CString>;

                println!("\nValidation support present");
                let layer_names_string: Vec<&str> = required_layers
                    .iter()
                    .map(|s| s.as_str())
                    .collect();
                layer_names_cstring = layer_names_string
                    .iter()
                    .map(|r| CString::new(*r).unwrap())
                    .collect();
                layer_names_raw = layer_names_cstring.iter().map(|s| s.as_ptr()).collect();

                create_info = create_info.enabled_layer_names(&layer_names_raw); // TODO Finish validation layer stuff eventually

                let instance: Instance;
                unsafe {
                    instance = entry.create_instance(&create_info, None).unwrap();
                }

                Ok(instance)
            } else {
                Err(String::from("Required window extensions missing"))
            }
        }

        fn physical_init(instance: &Instance, surface_loader: &khr::Surface, surface: vk::SurfaceKHR,
                         required_extensions: &Vec<CString>)
                         -> Option<(vk::PhysicalDevice, // Physical device handle
                               u32, // Presentation family index
                               u32, // graphics family index
                               Vec<vk::SurfaceFormatKHR>, // Supported surface formats
                               Vec<vk::PresentModeKHR>, // presentation modes
                               vk::SampleCountFlags)> // max msaa samples
        {
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
                physical_devices = instance.enumerate_physical_devices().unwrap();
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
                    dev_properties = instance.get_physical_device_properties(*device);
                    dev_features = instance.get_physical_device_features(*device);
                    instance.get_physical_device_features2(*device, &mut features2);
                }

                // Ensure that at least one kind of surface color/pixel format is supported
                unsafe {
                    surface_formats = surface_loader
                        .get_physical_device_surface_formats(*device, surface).unwrap();
                    // Ensure that the desired FIFO format for pushing images to the screen is available
                    present_modes = surface_loader
                        .get_physical_device_surface_present_modes(*device, surface).unwrap();
                }

                let mut all_queues_found = false;
                if required_physical_extensions_present(instance, *device, required_extensions) &&
                    !present_modes.is_empty() && !surface_formats.is_empty() && dev_features.sampler_anisotropy ==
                    vk::TRUE && rt_features.ray_tracing_pipeline == vk::TRUE &&
                    buf_features.buffer_device_address == vk::TRUE {
                    let queue_families: Vec<vk::QueueFamilyProperties>;
                    unsafe {
                        queue_families = instance
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
                                    surface_loader.get_physical_device_surface_support(*device, idx as u32, surface)
                                        .unwrap()
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
                Some((physical_devices[dev_idx], present_family_index, graphics_family_index, surface_formats,
                     present_modes, max_msaa_samples))
            } else {
                None
            }
        }

        pub fn logical_init(instance: &Instance, physical_device: &vk::PhysicalDevice, graphics_family: u32,
                            presentation_family: u32, required_extensions: &Vec<CString>)
            -> (vk::Queue, // presentation queue
                vk::Queue, // graphics queue
                Device) // logical device
         {
            let extensions_cvec: Vec<*const c_char> = required_extensions
                .iter()
                .map(|e| e.as_ptr())
                .collect();

            let queue_priority: [f32; 1] = [1.0];
            let graphics_queue_create_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(graphics_family)
                .queue_priorities(&queue_priority);

            let mut qci: Vec<vk::DeviceQueueCreateInfo> = Vec::new();
            qci.push(graphics_queue_create_info);
            if presentation_family != graphics_family {
                qci.push(vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(presentation_family)
                    .queue_priorities(&queue_priority));
            }

            let mut rt_features = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
            let mut accel_features = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
            let mut buf_features = vk::PhysicalDeviceBufferDeviceAddressFeaturesEXT::default();
            let mut features2 = vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut rt_features)
                .push_next(&mut buf_features)
                .push_next(&mut accel_features);
            unsafe {
                instance.get_physical_device_features2(*physical_device, &mut features2)
            }

            let device_create_info = vk::DeviceCreateInfo::default()
                .enabled_extension_names(&extensions_cvec)
                .queue_create_infos(qci.as_slice())
                .push_next(&mut features2);

            let logical_device = unsafe { instance.create_device(*physical_device, &device_create_info,
                                                                      None).unwrap() };

            let present_queue = unsafe {
                logical_device
                    .get_device_queue(presentation_family, 0)
            };
            let graphics_queue = unsafe {
                logical_device
                    .get_device_queue(graphics_family, 0)
            };

            (present_queue, graphics_queue, logical_device)
        }

        let entry = load_entry();
        let window = init_window(&ev_loop);
        let instance = instance_init(&entry, &window, &required_layers).unwrap();
        let surface: vk::SurfaceKHR;
        unsafe {
            surface = ash_window::create_surface(
                &entry,
                &instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            ).unwrap();
        }
        let surface_loader = khr::Surface::new(&entry, &instance);
        let (physical_device, present_family_index, graphics_family_index, supported_surface_formats, present_modes,
             max_msaa_samples) = physical_init(&instance, &surface_loader, surface, required_extensions).unwrap();
        let (present_queue, graphics_queue, logical_device) = logical_init(&instance, &physical_device,
                                                                           graphics_family_index,
                                                                           present_family_index, required_extensions);

        VkCore {
            _entry: entry,
            window,
            instance,
            surface,
            surface_loader,
            physical_device,
            present_family_index,
            graphics_family_index,
            supported_surface_formats,
            present_modes,
            max_msaa_samples,
            present_queue,
            graphics_queue,
            logical_device
        }
    }

    pub fn destroy(&self) {
        unsafe {
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        };
    }
}