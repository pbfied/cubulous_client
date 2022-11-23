use ash::{vk, Device};

use std::ffi::{c_char, CStr, CString};

use crate::renderer::core::Core;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::render_target::RenderTarget;

pub(crate) struct LogicalLayer {
    pub(crate) logical_queue: vk::Queue,
    pub(crate) logical_device: Device
}

impl LogicalLayer {
    pub(crate) fn new(core: &Core, physical_layer: &PhysicalLayer, required_extensions: &Vec<CString>) -> LogicalLayer {
        let extensions_cvec: Vec<*const c_char> = required_extensions
            .iter()
            .map(|e| e.as_ptr())
            .collect();

        let queue_priority: [f32; 1] = [1.0];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(physical_layer.family_index)
            .queue_priorities(&queue_priority);
        let enabled_features: vk::PhysicalDeviceFeatures;
        unsafe {
            enabled_features = core.instance.get_physical_device_features(physical_layer.physical_device);
        }

        let qci_slice = [queue_create_info];
        let device_create_info = vk::DeviceCreateInfo::default()
            .enabled_extension_names(&extensions_cvec)
            .enabled_features(&enabled_features)
            .queue_create_infos(&qci_slice);

        let logical_device = unsafe { core.instance.create_device(physical_layer.physical_device, &device_create_info,
                                          None).unwrap() };

        let logical_queue = unsafe {
            logical_device.get_device_queue(physical_layer.family_index, 0) };

        LogicalLayer {
            logical_queue,
            logical_device
        }
    }

    pub(crate) fn wait_idle(&self) {
        unsafe { self.logical_device.device_wait_idle().unwrap() };
    }

    pub(crate) fn destroy(&self) {
        unsafe { self.logical_device.destroy_device(None) };
    }
}