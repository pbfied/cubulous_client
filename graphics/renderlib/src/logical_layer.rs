use ash::{vk, Device};

use std::ffi::{c_char, CString};

use crate::core::Core;
use crate::physical_layer::PhysicalLayer;

pub struct LogicalLayer {
    pub present_queue: vk::Queue,
    pub graphics_queue: vk::Queue,
    pub logical_device: Device
}

impl LogicalLayer {
    pub fn new(core: &Core, physical_layer: &PhysicalLayer, required_extensions: &Vec<CString>) -> LogicalLayer {
        let extensions_cvec: Vec<*const c_char> = required_extensions
            .iter()
            .map(|e| e.as_ptr())
            .collect();

        let queue_priority: [f32; 1] = [1.0];
        let graphics_queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(physical_layer.graphics_family_index)
            .queue_priorities(&queue_priority);

        let mut qci: Vec<vk::DeviceQueueCreateInfo> = Vec::new();
        qci.push(graphics_queue_create_info);
        if physical_layer.present_family_index != physical_layer.graphics_family_index {
            qci.push(vk::DeviceQueueCreateInfo::default()
                .queue_family_index(physical_layer.present_family_index)
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
            core.instance.get_physical_device_features2(physical_layer.physical_device, &mut features2)
        }

        let device_create_info = vk::DeviceCreateInfo::default()
            .enabled_extension_names(&extensions_cvec)
            .queue_create_infos(qci.as_slice())
            .push_next(&mut features2);

        let logical_device = unsafe { core.instance.create_device(physical_layer.physical_device, &device_create_info,
                                          None).unwrap() };

        let present_queue = unsafe {
            logical_device
                .get_device_queue(physical_layer.present_family_index, 0)
        };
        let graphics_queue = unsafe {
            logical_device
                .get_device_queue(physical_layer.graphics_family_index, 0)
        };

        LogicalLayer {
            present_queue,
            graphics_queue,
            logical_device
        }
    }

    pub fn wait_idle(&self) {
        unsafe { self.logical_device.device_wait_idle().unwrap() };
    }

    pub fn destroy(&self) {
        unsafe { self.logical_device.destroy_device(None) };
    }
}