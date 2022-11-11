use std::env;
use std::ffi::{c_char, CStr, CString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::mem;
use std::path::Path;

use ash::{vk, Device, Entry, Instance};
use ash::extensions::khr::{Surface, Swapchain};
use num::clamp;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle}; // Entry holds Vulkan functions
// vk holds Vulkan structs with no methods along with Vulkan macros
// Instance wraps Entry functions with a winit surface and some under the hood initialization parameters
// Device is a logical Vulkan device

use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Icon, Window, WindowBuilder, WindowId},
};

const MAX_FRAMES_IN_FLGIHT: usize = 2;

pub struct CubulousRenderer {
    entry: Entry,
    window: Window,
    instance: Instance,
    surface: vk::SurfaceKHR,
    surface_loader: Surface,
    swap_loader: Swapchain,
    swap_chain: vk::SwapchainKHR,
    surface_format: vk::Format,
    extent: vk::Extent2D,
    physical_device: vk::PhysicalDevice,
    logical_device: Device,
    family_index: u32,
    supported_surface_formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
    logical_queue: vk::Queue,
    image_views: Vec<vk::ImageView>,
    shader_modules: Vec<vk::ShaderModule>, // In [Vert, Frag] order
    pipeline_layout: vk::PipelineLayout,
    render_pass: vk::RenderPass,
    pipelines: Vec<vk::Pipeline>,
    frame_buffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available_sems: Vec<vk::Semaphore>,
    render_finished_sems: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize
}

struct PhysicalDependencies {
    physical_device: vk::PhysicalDevice,
    family_index: u32,
    supported_surface_formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>
}

struct SwapDependencies {
    swap_loader: Swapchain,
    swap_chain: vk::SwapchainKHR,
    surface_format: vk::Format,
    extent: vk::Extent2D
}

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

fn setup_swap_chain(
    instance: &Instance,
    logical_device: &Device,
    physical_dependencies: &PhysicalDependencies,
    window: &Window,
    surface_loader: &Surface,
    surface: vk::SurfaceKHR,
) -> SwapDependencies {
    let capabilities: vk::SurfaceCapabilitiesKHR;
    unsafe {
        capabilities= surface_loader
            .get_physical_device_surface_capabilities(physical_dependencies.physical_device,
                                                      surface).unwrap();
    }

    // Choose the first surface format with the specified conditions or choose the first option
    // otherwise
    let surface_format =
        match physical_dependencies
            .supported_surface_formats
            .iter()
            .find(|f|f.format == vk::Format::B8G8R8A8_SRGB &&
                f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
        {
            Some(x) => x,
            None => &physical_dependencies.supported_surface_formats[0]
        };

    let presentation_mode =
        match physical_dependencies
            .present_modes
            .iter()
            .find(|p|**p == vk::PresentModeKHR::MAILBOX)
        {
            Some(x) => *x,
            None => vk::PresentModeKHR::FIFO
        };

    let swap_extent = choose_swap_extent(window, &capabilities);

    let mut image_count = capabilities.min_image_count + 1;
    if capabilities.max_image_count > 0 && image_count > capabilities.max_image_count {
        image_count = capabilities.max_image_count
    }

    let swap_create_info = vk::SwapchainCreateInfoKHR::default()
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
    let swap_chain: vk::SwapchainKHR;
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

fn setup_image_views(device: &Device, swap_deps: &SwapDependencies) -> Vec<vk::ImageView> {
    let swap_chain_images: Vec<vk::Image>;
    unsafe {
        swap_chain_images = swap_deps.swap_loader
            .get_swapchain_images(swap_deps.swap_chain).unwrap();
    }

    let mut image_views: Vec<vk::ImageView> = Vec::new();
    for i in swap_chain_images {
        let create_info = vk::ImageViewCreateInfo::default()
            .image(i)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(swap_deps.surface_format)
            .components(vk::ComponentMapping { // Allows remapping of color channels, I.E. turn all blues into shades of red
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

    let mut shader_modules: Vec<vk::ShaderModule> = Vec::with_capacity(shader_paths.len());
    for sp in shader_paths.iter() {
        let shader_spv = load_shader(sp).unwrap();
        let shader_create_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::ShaderModuleCreateFlags::default(),
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

// Describes the color and depth buffers for each frame, and ???
fn setup_render_pass(logical_device: &Device, surface_format: vk::Format) -> vk::RenderPass {
    let attachment_desc = vk::AttachmentDescription::default() // Color attachment
        .format(surface_format) // Should match the format of swap chain images
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR) // What to do with pre existing data in the attachment before rendering
        .store_op(vk::AttachmentStoreOp::STORE) // What to do with data in attachment after rendering
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE) // Not sure what stencil buffer is
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED) // image layout pre render
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR); // Ready for presentation, not sure how that maps to a layout

    let attachment_desc_array = [attachment_desc];

    let attachment_ref = vk::AttachmentReference::default()
        .attachment(0) // Index of attachment to reference
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL); // Optimal layout for a color attachment

    let attachment_ref_array = [attachment_ref];

    let subpass = vk::SubpassDescription::default() // Each render pass consists of subpasses
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS) // Future Vulkan may have compute subpasses
        .color_attachments(&attachment_ref_array);

    let subpass_array = [subpass];

    let subpass_dependency = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL) // Refers to implicit subpass before the first sub pass
        .dst_subpass(0)  // vk::SUBPASS_EXTERNAL here would refer to the implicit after the last sub pass
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT) // Wait on the color attachment output stage (after color blending)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
        .dependency_flags(vk::DependencyFlags::empty());

    let dependencies = [subpass_dependency];

    let render_pass_create_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachment_desc_array)
        .subpasses(&subpass_array)
        .dependencies(&dependencies);

    unsafe {logical_device.create_render_pass(&render_pass_create_info, None).unwrap() }
}

fn setup_pipeline_layout(logical_device: &Device) -> vk::PipelineLayout {
    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default();

    unsafe {
        logical_device.create_pipeline_layout(&pipeline_layout_create_info, None).unwrap() }
}

fn setup_pipelines(logical_device: &Device,
                   surface_format: vk::Format,
                   shader_modules: &Vec<vk::ShaderModule>,
                   pipeline_layout: vk::PipelineLayout,
                   render_pass: vk::RenderPass
) -> Vec<vk::Pipeline> {
    fn setup_pipeline_stages(shader_modules: &Vec<vk::ShaderModule>) -> Vec<vk::PipelineShaderStageCreateInfo> {
        // Reminder that shader modules are in [vert, frag] order
        let create_bits = [vk::ShaderStageFlags::VERTEX,
            vk::ShaderStageFlags::FRAGMENT];
        let mut create_info: Vec<vk::PipelineShaderStageCreateInfo> = Vec::with_capacity(
            shader_modules.len());
        for (sm, flag) in shader_modules.iter()
            .zip(create_bits) {
            create_info.push(vk::PipelineShaderStageCreateInfo::default()
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
                .stage(flag)
                .module(*sm)
            );
        }

        create_info
    }

    let pipeline_stages = setup_pipeline_stages(shader_modules);

    let vertex_inputs = vk::PipelineVertexInputStateCreateInfo::default();

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST) // Triangle from every three vertices
        .primitive_restart_enable(false); // ??

    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);

    let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
        .depth_clamp_enable(false) // Clamps (?) fragments beyond the far and near planes to said planes
        .rasterizer_discard_enable(false) // Makes geometry not pass through the rasterizer
        .polygon_mode(vk::PolygonMode::FILL) // Determines whether polygons are represented as points, lines or surfaces
        .line_width(1.0) // Line thickness in units of fragment numbers (probably roughly equivalent to pixels?)
        .cull_mode(vk::CullModeFlags::BACK) // Cull the back faces of geometry
        .front_face(vk::FrontFace::CLOCKWISE) // Rules for determining if a face is front ??
        .depth_bias_enable(false) // Parameters for transforming depth values
        .depth_bias_constant_factor(0.0)
        .depth_bias_clamp(0.0)
        .depth_bias_slope_factor(0.0);

    let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
        .sample_shading_enable(false) // Disabled for now
        .rasterization_samples(vk::SampleCountFlags::TYPE_1)
        .min_sample_shading(1.0)
        // .sample_mask() Leave NULL
        .alpha_to_coverage_enable(false)
        .alpha_to_one_enable(false);

    let additive_color_blending_create_infos = [
        vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD) // Blend operation
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
    ];

    let blend_constants: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

    let color_blending_create_info = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false) // Note that enabling this disables all of the attachment states effects
        .logic_op(vk::LogicOp::COPY)
        .attachments(&additive_color_blending_create_infos)
        .blend_constants(blend_constants);

    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

    let dynamic_state_create_info = vk::PipelineDynamicStateCreateInfo::default()
        .dynamic_states(&dynamic_states);

    let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&pipeline_stages)
        .vertex_input_state(&vertex_inputs)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterization_state)
        .multisample_state(&multisample_state)
        // .depth_stencil_state() Currently unused
        .color_blend_state(&color_blending_create_info)
        .dynamic_state(&dynamic_state_create_info)
        .layout(pipeline_layout)
        .render_pass(render_pass)
        .subpass(0);

    let graphics_pipelines = unsafe { logical_device.create_graphics_pipelines(vk::PipelineCache::null(),
                                                                               &[pipeline_info],
                                                                               None).unwrap() };

    for &s in shader_modules.iter() {
        unsafe { logical_device.destroy_shader_module(s, None) }
    }

    graphics_pipelines
}

fn setup_frame_buffers(logical_device: &Device,
                       image_views: &Vec<vk::ImageView>,
                       render_pass: vk::RenderPass,
                       swap_extent: vk::Extent2D) -> Vec<vk::Framebuffer> {
    let mut frame_buffers: Vec<vk::Framebuffer> = Vec::with_capacity(image_views.len());
    for v in image_views.iter() {
        let image_slice = [*v];
        let create_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&image_slice)
            .width(swap_extent.width)
            .height(swap_extent.height)
            .layers(1);

        unsafe { frame_buffers.push(logical_device.create_framebuffer(&create_info, None).unwrap()) }
    }

    frame_buffers
}

impl CubulousRenderer {
    pub fn new(ev_loop: &EventLoop<()>) -> CubulousRenderer {
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

        fn vulkan_physical_setup(
            instance: &Instance,
            surface_loader: &Surface,
            surface: vk::SurfaceKHR,
            required_extensions: &Vec<CString>,
        ) -> Result<PhysicalDependencies, String> {
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
                    let queue_families: Vec<vk::QueueFamilyProperties>;
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
                    supported_surface_formats: surface_formats
                };
                Ok(physical_dependencies)
            } else {
                Err(String::from("Failed to locate suitable physical device"))
            }
        }

        fn logical_device_init(instance: &Instance, physical_device: &vk::PhysicalDevice, qf_index: u32,
                               required_extensions: &Vec<CString>) -> Device {
            let extensions_cvec: Vec<*const c_char> = required_extensions
                .iter()
                .map(|e| e.as_ptr())
                .collect();

            let queue_priority: [f32; 1] = [1.0];
            let queue_create_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(qf_index)
                .queue_priorities(&queue_priority);
            let enabled_features: vk::PhysicalDeviceFeatures;
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

        fn setup_command_pool(logical_device: &Device, family_idx: u32) -> vk::CommandPool {
            let create_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(family_idx);

            unsafe { logical_device.create_command_pool(&create_info, None).unwrap() }
        }

        fn setup_command_buffers(logical_device: &Device, command_pool: vk::CommandPool) -> Vec<vk::CommandBuffer> {
            let create_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(MAX_FRAMES_IN_FLGIHT as u32);

            unsafe { logical_device.allocate_command_buffers(&create_info).unwrap() }
        }

        fn setup_sync_objects(logical_device: &Device) -> (Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>) {
            let sem_create_info = vk::SemaphoreCreateInfo::default();
            let fence_create_info = vk::FenceCreateInfo::default()
                .flags(vk::FenceCreateFlags::SIGNALED);

            let mut image_avail_vec: Vec<vk::Semaphore> = Vec::with_capacity(MAX_FRAMES_IN_FLGIHT as usize);
            let mut render_finished_vec: Vec<vk::Semaphore> = Vec::with_capacity(MAX_FRAMES_IN_FLGIHT as usize);
            let mut fences_vec: Vec<vk::Fence> = Vec::with_capacity(MAX_FRAMES_IN_FLGIHT as usize);

            for _ in 0..MAX_FRAMES_IN_FLGIHT {
                unsafe {
                    image_avail_vec.push(logical_device.create_semaphore(&sem_create_info, None).unwrap());
                    render_finished_vec.push(logical_device.create_semaphore(&sem_create_info, None).unwrap());
                    fences_vec.push(logical_device.create_fence(&fence_create_info, None).unwrap());
                }
            }

            (image_avail_vec, render_finished_vec, fences_vec)
        }

        let required_extensions: Vec<CString> = Vec::from([
            CString::from(vk::KhrSwapchainFn::name()), // Equivalent to the Vulkan VK_KHR_SWAPCHAIN_EXTENSION_NAME
        ]);
        let required_layers: Vec<String> = Vec::from([String::from("VK_LAYER_KHRONOS_validation")]);

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
        let logical_queue: vk::Queue;
        unsafe {
            logical_queue = logical_device
                .get_device_queue(physical_dependencies.family_index, 0);
        }

        // Contain for images to render to
        let swap_dependencies= setup_swap_chain(&instance, &logical_device,
                                                &physical_dependencies, &window, &surface_loader,
                                                surface);
        let image_views = setup_image_views(&logical_device,
                                            &swap_dependencies);

        let shader_modules = load_all_shaders(&logical_device);

        let pipeline_layout = setup_pipeline_layout(&logical_device);

        let render_pass = setup_render_pass(&logical_device, swap_dependencies.surface_format);

        let pipelines = setup_pipelines(&logical_device,
                                        swap_dependencies.surface_format,
                                        &shader_modules, pipeline_layout,
                                        render_pass);

        let frame_buffers = setup_frame_buffers(&logical_device, &image_views, render_pass, swap_dependencies.extent);

        let command_pool = setup_command_pool(&logical_device, physical_dependencies.family_index);

        let command_buffers = setup_command_buffers(&logical_device, command_pool);

        let (image_available_sems, render_finished_sems, in_flight_fences) =
        setup_sync_objects(&logical_device);

        let current_frame = 0;

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
            supported_surface_formats: physical_dependencies.supported_surface_formats,
            present_modes: physical_dependencies.present_modes,
            logical_queue,
            image_views,
            shader_modules,
            pipeline_layout,
            render_pass,
            pipelines,
            frame_buffers,
            command_pool,
            command_buffers,
            image_available_sems,
            render_finished_sems,
            in_flight_fences,
            current_frame
        }
    }

    fn record_command_buffer(&self, image_index: u32) {
        // Defines a transformation from a VK image to the framebuffer
        fn setup_viewport(swap_extent: &vk::Extent2D) -> vk::Viewport {
            vk::Viewport::default()
                .x(0.0) // Origin
                .y(0.0)
                .width(swap_extent.width as f32) // Max range from origin
                .height(swap_extent.height as f32)
                .min_depth(0.0) // ??
                .max_depth(1.0)
        }

        fn setup_scissor(swap_extent: &vk::Extent2D) -> vk::Rect2D {
            vk::Rect2D::default()
                .offset(vk::Offset2D::default()
                    .x(0)
                    .y(0))
                .extent(*swap_extent)
        }

        let begin_info = vk::CommandBufferBeginInfo::default();

        let render_offset = vk::Offset2D::default()
            .x(0)
            .y(0);
        let render_extent = vk::Extent2D::default()
            .height(self.extent.height)
            .width(self.extent.width);
        let render_area = vk::Rect2D::default() // Area where shader loads and stores occur
            .offset(render_offset)
            .extent(render_extent);

        let clear_colors = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0], // Values to use for the LOAD_OP_CLEAR attachment operation
            }
        }];

        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(self.render_pass)
            .framebuffer(self.frame_buffers[image_index as usize])
            .render_area(render_area)
            .clear_values(&clear_colors);

        let viewports = [setup_viewport(&self.extent)];

        let scissors = [setup_scissor(&self.extent)];

        let command_buffer = *self.command_buffers.get(self.current_frame).unwrap();

        unsafe {
            self.logical_device.begin_command_buffer(command_buffer, &begin_info).unwrap();
            self.logical_device.cmd_begin_render_pass(command_buffer,
                                                      &render_pass_info,
                                                      vk::SubpassContents::INLINE); // Execute commands in primary buffer
            self.logical_device.cmd_bind_pipeline(command_buffer,
                                                  vk::PipelineBindPoint::GRAPHICS,
                                                  *self.pipelines.get(0).unwrap());
            self.logical_device.cmd_set_viewport(command_buffer, 0, &viewports);
            self.logical_device.cmd_set_scissor(command_buffer, 0, &scissors);
            self.logical_device.cmd_draw(command_buffer,
                                         3,
                                         1,
                                         0, // Vertex buffer offset, lowest value of gl_VertexIndex
                                         0); // lowest value of gl_InstanceIndex
            self.logical_device.cmd_end_render_pass(command_buffer);
            self.logical_device.end_command_buffer(command_buffer).unwrap();
        }
    }

    fn draw_frame(&mut self) {
        let fences = [*self.in_flight_fences.get(self.current_frame).unwrap()];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let wait_sems = [*self.image_available_sems.get(self.current_frame).unwrap()];
        let command_buffers = [*self.command_buffers.get(self.current_frame).unwrap()];
        let sig_sems = [*self.render_finished_sems.get(self.current_frame).unwrap()];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_sems)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&sig_sems);
        let submit_array = [submit_info];
        let swap_chains = [self.swap_chain];

        unsafe {
            self.logical_device.wait_for_fences(&fences, true, u64::MAX).unwrap();

            let (next_image_idx, _) = match self.swap_loader.acquire_next_image(self.swap_chain,
                                    u64::MAX,
                                    *self.image_available_sems.get(self.current_frame).unwrap(),
                                    vk::Fence::null()) {
                Ok((img_idx)) => img_idx,
                Err(result) => match result {
                    vk::Result::ERROR_OUT_OF_DATE_KHR => { self.recreate_swap_chain(); return },
                    _ => panic!("Unknown error at acquire_next_image")
                }
            };

            self.logical_device.reset_fences(&fences).unwrap();

            let image_indices = [next_image_idx];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&sig_sems)
                .swapchains(&swap_chains)
                .image_indices(&image_indices);
            self.logical_device.reset_command_buffer(*self.command_buffers.get(self.current_frame).unwrap(),
                                                     vk::CommandBufferResetFlags::empty())
                .unwrap();
            self.record_command_buffer(next_image_idx);
            self.logical_device.queue_submit(self.logical_queue, &submit_array, *self.in_flight_fences.get(self.current_frame).unwrap()).unwrap();

            match self.swap_loader.queue_present(self.logical_queue, &present_info)
            {
                Err(r) => match r {
                    vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR => { self.recreate_swap_chain() },
                    _ => panic!("Unknown error")
                }
                Ok(_) => { }
            }
        }

        self.current_frame = (self.current_frame + 1) % MAX_FRAMES_IN_FLGIHT;
    }

    fn cleanup_swap_chain(&self) {
        unsafe {
            self.logical_device.device_wait_idle().unwrap();

            for f in self.frame_buffers.iter() {
                self.logical_device.destroy_framebuffer(*f, None);
            }
            for &v in self.image_views.iter() {
                self.logical_device.destroy_image_view(v, None);
            }
            self.swap_loader.destroy_swapchain(self.swap_chain, None);
        }
    }

    fn recreate_swap_chain(&mut self) {
        self.cleanup_swap_chain();

        let physical_dependencies = PhysicalDependencies {
            physical_device: self.physical_device,
            family_index: self.family_index,
            supported_surface_formats: self.supported_surface_formats.clone(),
            present_modes: self.present_modes.clone()
        };

        let swap_dependencies = setup_swap_chain(&self.instance,
                                                 &self.logical_device,
                                                 &physical_dependencies,
                                                 &self.window, &self.surface_loader,
                                                 self.surface);

        self.image_views = setup_image_views(&self.logical_device, &swap_dependencies);

        self.swap_chain = swap_dependencies.swap_chain;
        self.swap_loader = swap_dependencies.swap_loader;
        self.surface_format = swap_dependencies.surface_format;
        self.extent = swap_dependencies.extent;

        self.frame_buffers = setup_frame_buffers(&self.logical_device, &self.image_views, self.render_pass, self.extent);
    }

    fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn run_blocking(mut self, event_loop: EventLoop<()>) {
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent {
                    // If event has Event::WindowEvent type and event: WindowEvent::CloseRequested member and if window_id == window.id()
                    event: WindowEvent::CloseRequested,
                    window_id,
                } if window_id == self.window_id() => *control_flow = ControlFlow::Exit,
                Event::MainEventsCleared => self.window.request_redraw(), // Emits a RedrawRequested event after input events end
                                                                        // Needed when a redraw is needed after the user resizes for example
                Event::RedrawRequested(window_id) if window_id == self.window_id() => self.draw_frame(),
                Event::LoopDestroyed => unsafe { self.logical_device.device_wait_idle().unwrap() },
                _ => (), // Similar to the "default" case of a switch statement: return void which is essentially () in Rust
            }
        });
    }
}

impl Drop for CubulousRenderer {
    fn drop(&mut self) {
        self.cleanup_swap_chain();
        unsafe {
            for i in self.image_available_sems.iter() {
                self.logical_device.destroy_semaphore(*i, None);
            }
            for r in self.render_finished_sems.iter() {
                self.logical_device.destroy_semaphore(*r, None);
            }
            for f in self.in_flight_fences.iter() {
                self.logical_device.destroy_fence(*f, None);
            }
            self.logical_device.destroy_command_pool(self.command_pool, None);
            for s in self.pipelines.iter() {
                self.logical_device.destroy_pipeline(*s, None);
            }
            self.logical_device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.logical_device.destroy_render_pass(self.render_pass, None);
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}