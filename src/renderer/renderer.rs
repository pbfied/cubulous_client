use std::env;
use std::ffi::{c_char, CStr, CString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::mem;
use std::path::Path;
use std::task::ready;

use ash::{vk, Device, Entry, Instance};
use ash::extensions::khr::{Surface, Swapchain};
use ash::vk::{PhysicalDevice, PresentModeKHR, SurfaceFormatKHR, SurfaceKHR, PhysicalDeviceProperties, PhysicalDeviceFeatures, QueueFamilyProperties, SurfaceCapabilitiesKHR, SwapchainKHR, ImageView, ComponentMapping, Image, Queue, ShaderModuleCreateFlags, ShaderModule};
use num::clamp;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle}; // Entry holds Vulkan functions
// vk holds Vulkan structs with no methods along with Vulkan macros
// Instance wraps Entry functions with a winit surface and some under the hood initialization parameters
// Device is a logical Vulkan device

use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop,
    event_loop::{ControlFlow, EventLoop},
    window::{Icon, Window, WindowBuilder, WindowId},
};

pub struct CubulousRenderer {
    entry: Entry,
    window: Window,
    instance: Instance,
    surface: SurfaceKHR,
    surface_loader: Surface,
    swap_loader: Swapchain,
    swap_chain: SwapchainKHR,
    surface_format: vk::Format,
    extent: vk::Extent2D,
    physical_device: PhysicalDevice,
    logical_device: Device,
    family_index: u32,
    surface_formats: Vec<SurfaceFormatKHR>,
    present_modes: Vec<PresentModeKHR>,
    logical_queue: Queue,
    image_views: Vec<ImageView>,
    shaders: Vec<ShaderModule> // In [Vert, Frag] order
}

impl CubulousRenderer {
    pub fn new(ev_loop: &EventLoop<()>) -> CubulousRenderer {
        struct PhysicalDependencies {
            physical_device: PhysicalDevice,
            family_index: u32,
            surface_formats: Vec<SurfaceFormatKHR>,
            present_modes: Vec<PresentModeKHR>
        }

        struct SwapDependencies {
            swap_loader: Swapchain,
            swap_chain: SwapchainKHR,
            surface_format: vk::Format,
            extent: vk::Extent2D
        }

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
                .with_window_icon(read_window_icon("assets/g1141.png"))
                .build(event_loop)
                .unwrap()
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
            }
            else {
                Err(String::from("Required window extensions missing"))
            }
        }

        fn required_physical_extensions_present(instance: &Instance,
                                                physical_device: PhysicalDevice,
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

        fn vulkan_physical_setup(
            instance: &Instance,
            surface_loader: &Surface,
            surface: SurfaceKHR,
            required_extensions: &Vec<CString>,
        ) -> Result<PhysicalDependencies, String> {
            let physical_devices: Vec<PhysicalDevice>;
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
            let mut queue_family_idx = 0;
            let mut dev_found = false;
            let mut dev_idx: usize = 0;
            let mut present_modes: Vec<PresentModeKHR> = vec![];
            let mut surface_formats: Vec<SurfaceFormatKHR> = vec![];

            // For each physical device
            for (p_idx, device) in physical_devices.iter().enumerate() {
                let dev_properties: PhysicalDeviceProperties;
                let dev_features: PhysicalDeviceFeatures;
                unsafe {
                    dev_properties = instance.get_physical_device_properties(*device);
                    dev_features = instance.get_physical_device_features(*device);
                }

                // Ensure that at least one kind of surface color/pixel format is supported
                unsafe {
                    surface_formats = surface_loader
                        .get_physical_device_surface_formats(*device, surface).unwrap();
                    // Ensure that the desired FIFO format for pushing images to the screen is available
                    present_modes = surface_loader
                        .get_physical_device_surface_present_modes(*device, surface).unwrap();
                }

                let mut queue_found = false;
                if required_physical_extensions_present(instance,
                                                        *device,
                                                        required_extensions) &&
                    !present_modes.is_empty() &&
                    !surface_formats.is_empty() &&
                    !present_modes.is_empty() {
                    let queue_families: Vec<QueueFamilyProperties>;
                    unsafe {
                        queue_families = instance
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
                                surface_support = surface_loader
                                    .get_physical_device_surface_support(*device, idx as u32, surface)
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
                let physical_dependencies = PhysicalDependencies {
                    physical_device: physical_devices[dev_idx],
                    family_index: queue_family_idx,
                    present_modes,
                    surface_formats
                };
                Ok(physical_dependencies)
            } else {
                Err(String::from("Failed to locate suitable physical device"))
            }
        }

        fn logical_device_init(instance: &Instance, physical_device: &PhysicalDevice, qf_index: u32,
                               required_extensions: &Vec<CString>) -> Device {
            let extensions_cvec: Vec<*const c_char> = required_extensions
                .iter()
                .map(|e| e.as_ptr())
                .collect();

            let queue_priority: [f32; 1] = [1.0];
            let queue_create_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(qf_index)
                .queue_priorities(&queue_priority);
            let enabled_features: PhysicalDeviceFeatures;
            unsafe {
                enabled_features = instance.get_physical_device_features(*physical_device);
            }

            let qci_slice = [queue_create_info];
            let device_create_info = vk::DeviceCreateInfo::default()
                .enabled_extension_names(&extensions_cvec)
                .enabled_features(&enabled_features)
                .queue_create_infos(&qci_slice);

            unsafe {
                return instance.create_device(*physical_device, &device_create_info,
                                                None).unwrap();
            }
        }

        fn choose_swap_extent(window: &Window, capabilities: &SurfaceCapabilitiesKHR) -> vk::Extent2D {
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

        fn vulkan_swap_chain_setup(
            instance: &Instance,
            logical_device: &Device,
            physical_dependencies: &PhysicalDependencies,
            window: &Window,
            surface_loader: &Surface,
            surface: SurfaceKHR,
        ) -> SwapDependencies {
            let capabilities: SurfaceCapabilitiesKHR;
            unsafe {
                capabilities= surface_loader
                    .get_physical_device_surface_capabilities(physical_dependencies.physical_device,
                                                              surface).unwrap();
            }

            // Choose the first surface format with the specified conditions or choose the first option
            // otherwise
            let surface_format =
                match physical_dependencies
                    .surface_formats
                    .iter()
                    .find(|f|f.format == vk::Format::B8G8R8A8_SRGB &&
                        f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
                {
                    Some(x) => x,
                    None => &physical_dependencies.surface_formats[0]
                };

            let presentation_mode =
                match physical_dependencies
                    .present_modes
                    .iter()
                    .find(|p|**p == PresentModeKHR::MAILBOX)
                {
                    Some(x) => *x,
                    None => PresentModeKHR::FIFO
                };

            let swap_extent = choose_swap_extent(window, &capabilities);

            let mut image_count = capabilities.min_image_count + 1;
            if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count {
                image_count = capabilities.max_image_count
            }

            let mut swap_create_info = vk::SwapchainCreateInfoKHR::default()
                .min_image_count(image_count)
                .image_format(surface_format.format)
                .image_color_space(surface_format.color_space)
                .image_extent(swap_extent)
                .image_array_layers(1) // Always 1 except for stereoscopic 3D, I.E. VR
                .surface(surface)

                // TODO This assumes only one queue family. Consider adding support for separate queue
                // families later on
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)

                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT) // "It is also possible that you'll
                // render images to a separate image first to perform
                // operations like post-processing. In that case you may use a value like
                // VK_IMAGE_USAGE_TRANSFER_DST_BIT instead and use a memory operation to transfer the rendered
                // image to a swap chain image."
                .pre_transform(capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(presentation_mode)
                .clipped(true)
                .old_swapchain(vk::SwapchainKHR::null());

            let swap_chain_loader = Swapchain::new(instance, logical_device);
            let swap_chain: SwapchainKHR;
            unsafe {
                swap_chain = swap_chain_loader
                    .create_swapchain(&swap_create_info, None).unwrap();
            }

            return SwapDependencies {
                swap_chain,
                swap_loader: swap_chain_loader,
                surface_format: surface_format.format,
                extent: swap_extent
            }
        }

        fn vulkan_setup_image_views(device: &Device, swap_deps: &SwapDependencies) -> Vec<ImageView> {
            let swap_chain_images: Vec<Image>;
            unsafe {
                swap_chain_images = swap_deps.swap_loader
                    .get_swapchain_images(swap_deps.swap_chain).unwrap();
            }

            let mut image_views: Vec<ImageView> = Vec::new();
            for i in swap_chain_images {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(i)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(swap_deps.surface_format)
                    .components(ComponentMapping { // Allows remapping of color channels, I.E. turn all blues into shades of red
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY
                    })
                    .subresource_range(vk::ImageSubresourceRange { // Describes image purpose, I.E. a human
                        // viewable image for something like VR is composed of multiple images
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1
                    });

                unsafe {
                    image_views.push(device.create_image_view(&create_info, None).unwrap());
                }
            }

            return image_views;
        }

        fn load_shader(path: &str) -> Result<Vec<u8>, String> {
            let mut buf = Vec::new();
            let mut file = File::open(path).unwrap();
            let filesize = file.seek(SeekFrom::End(0)).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            let size = file.read_to_end(&mut buf).unwrap();

            match filesize == size as u64 && (filesize % mem::size_of::<u32>() as u64) == 0 {
                true => Ok(buf),
                false => Err(String::from("Failed to read ") + path)
            }
        }

        fn load_all_shaders(logical_device: &Device) -> Vec<vk::ShaderModule> {
            let shader_paths = ["shaders/spv/vert.spv", "shaders/spv/frag.spv"];

            let mut shader_modules: Vec<ShaderModule> = Vec::with_capacity(shader_paths.len());
            for (idx, sp) in shader_paths.iter().enumerate() {
                let shader_spv = load_shader(sp).unwrap();
                let shader_create_info = vk::ShaderModuleCreateInfo {
                    s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
                    p_next: std::ptr::null(),
                    flags: ShaderModuleCreateFlags::default(),
                    code_size: shader_spv.len(),
                    p_code: shader_spv.as_ptr().cast::<u32>(),
                    _marker: PhantomData
                };
                shader_modules.push(unsafe {
                    logical_device.create_shader_module(&shader_create_info, None).unwrap()
                });
            }

            shader_modules
        }

        let required_extensions: Vec<CString> = Vec::from([
            CString::from(vk::KhrSwapchainFn::name()), // Equivalent to the Vulkan VK_KHR_SWAPCHAIN_EXTENSION_NAME
        ]);
        let required_layers: Vec<String> = Vec::from([String::from("VK_LAYER_KHRONOS_validation")]);

        let entry = load_entry();
        let window = init_window(&ev_loop);
        let instance = instance_init(&entry, &window, &required_layers).unwrap();
        let surface: SurfaceKHR;
        unsafe {
             surface = ash_window::create_surface(
                &entry,
                &instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
             ).unwrap();
        }
        let surface_loader = Surface::new(&entry, &instance);
        let physical_dependencies = vulkan_physical_setup(&instance,
                                                          &surface_loader, surface,
                                                          &required_extensions).unwrap();
        let logical_device = logical_device_init(
            &instance,
            &physical_dependencies.physical_device,
            physical_dependencies.family_index,
            &required_extensions,
        );
        let logical_queue: Queue;
        unsafe {
            logical_queue = logical_device
                .get_device_queue(physical_dependencies.family_index, 0);
        }

        // Contain for images to render to
        let swap_dependencies= vulkan_swap_chain_setup(&instance, &logical_device,
                                                       &physical_dependencies, &window, &surface_loader,
                                                       surface);
        let image_views = vulkan_setup_image_views(&logical_device,
                                                   &swap_dependencies);

        let shaders = load_all_shaders(&logical_device);

        CubulousRenderer {
            entry,
            window,
            instance,
            surface,
            swap_loader: swap_dependencies.swap_loader,
            swap_chain: swap_dependencies.swap_chain,
            surface_format: swap_dependencies.surface_format,
            surface_loader,
            extent: swap_dependencies.extent,
            physical_device: physical_dependencies.physical_device,
            logical_device,
            family_index: physical_dependencies.family_index,
            surface_formats: physical_dependencies.surface_formats,
            present_modes: physical_dependencies.present_modes,
            logical_queue,
            image_views,
            shaders
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn run_blocking(render_id: WindowId, event_loop: EventLoop<()>) {
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    // If event has Event::WindowEvent type and event: WindowEvent::CloseRequested member and if window_id == window.id()
                    event: WindowEvent::CloseRequested,
                    window_id,
                } if window_id == render_id => *control_flow = ControlFlow::Exit,
                _ => (), // Similar to the "default" case of a switch statement: return void which is essentially () in Rust
            }
        });
    }
}

impl Drop for CubulousRenderer {
    fn drop(&mut self) {
        unsafe {
            for &v in self.image_views.iter() {
                self.logical_device.destroy_image_view(v, None);
            }
            self.swap_loader.destroy_swapchain(self.swap_chain, None);
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}