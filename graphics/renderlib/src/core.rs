use std::env;
use std::fs::File;
use std::ffi::{c_char, CStr, CString};
use std::path::Path;

use ash::{vk, Entry, Instance};
use ash::extensions::khr::Surface;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle}; // Entry holds Vulkan functions

use winit::{
    dpi::LogicalSize,
    event_loop::EventLoop,
    window::{Icon, Window, WindowBuilder},
};

pub struct Core {
    _entry: Entry,
    pub window: Window,
    pub instance: Instance,
    pub(crate) surface: vk::SurfaceKHR,
    pub(crate) surface_loader: Surface,
}

impl Core {
    pub fn new(ev_loop: &EventLoop<()>, required_layers: &Vec<String>) -> Core {
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
            }
            else {
                Err(String::from("Required window extensions missing"))
            }
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
        let surface_loader = Surface::new(&entry, &instance);

        Core {
            _entry: entry,
            window,
            instance,
            surface,
            surface_loader
        }
    }

    pub fn destroy(&self) {
        unsafe {
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}