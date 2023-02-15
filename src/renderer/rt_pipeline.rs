use std::ffi::CStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::mem;
use ash::vk;
use ash::extensions::khr;
use ash::vk::Pipeline;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;

const RAYGEN_IDX: usize = 0;
const RAYHIT_IDX: usize = 1;
const RAYMISS_IDX: usize = 2;

pub struct RtPipeline {
    instance: khr::RayTracingPipeline,
    pipelines: Vec<Pipeline>,
    pipeline_layout: vk::PipelineLayout
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

fn load_all_shaders(logical_layer: &LogicalLayer) -> Vec<vk::ShaderModule> {
    let shader_paths = ["shaders/spv/rgen.spv", "shaders/spv/rhit.spv", "shaders/spv/rmiss.spv"];

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
            logical_layer.logical_device.create_shader_module(&shader_create_info, None).unwrap()
        });
    }

    shader_modules
}

impl RtPipeline {
    pub fn new(core: &Core, logical_layer: &LogicalLayer, layouts: &Vec<vk::DescriptorSetLayout>) -> RtPipeline {
        let instance = khr::RayTracingPipeline::new(&core.instance, &logical_layer.logical_device);
        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .flags(vk::PipelineLayoutCreateFlags::empty())
            .set_layouts(layouts.as_slice());
        let pipeline_layout = unsafe { logical_layer.logical_device.create_pipeline_layout(&layout_create_info, None)
            .unwrap() };
        let shader_groups = [
            vk::RayTracingShaderGroupCreateInfoKHR::default()
                .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL) // Raygen
                .general_shader(RAYGEN_IDX as u32)
                .closest_hit_shader(vk::SHADER_UNUSED_KHR)
                .intersection_shader(vk::SHADER_UNUSED_KHR)
                .any_hit_shader(vk::SHADER_UNUSED_KHR),
            vk::RayTracingShaderGroupCreateInfoKHR::default()
                .ty(vk::RayTracingShaderGroupTypeKHR::TRIANGLES_HIT_GROUP) // intersection
                .any_hit_shader(RAYHIT_IDX as u32)
                .general_shader(vk::SHADER_UNUSED_KHR)
                .closest_hit_shader(vk::SHADER_UNUSED_KHR)
                .intersection_shader(vk::SHADER_UNUSED_KHR),
            vk::RayTracingShaderGroupCreateInfoKHR::default() // miss
                .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL)
                .general_shader(RAYMISS_IDX as u32)
                .any_hit_shader(vk::SHADER_UNUSED_KHR)
                .intersection_shader(vk::SHADER_UNUSED_KHR)
                .closest_hit_shader(vk::SHADER_UNUSED_KHR)
        ];
        let shader_modules = load_all_shaders(logical_layer);
        let stage_create_info = [
            vk::PipelineShaderStageCreateInfo::default()
                .name(CStr::from_bytes_with_nul(b"rgen_main\0").unwrap())
                .stage(vk::ShaderStageFlags::RAYGEN_KHR)
                .module(shader_modules[RAYGEN_IDX]),
            vk::PipelineShaderStageCreateInfo::default()
                .name(CStr::from_bytes_with_nul(b"rhit_main\0").unwrap())
                .stage(vk::ShaderStageFlags::CLOSEST_HIT_KHR)
                .module(shader_modules[RAYHIT_IDX]),
            vk::PipelineShaderStageCreateInfo::default()
                .name(CStr::from_bytes_with_nul(b"rmiss_main\0").unwrap())
                .stage(vk::ShaderStageFlags::MISS_KHR)
                .module(shader_modules[RAYMISS_IDX])
            ];
        let create_info = [
            vk::RayTracingPipelineCreateInfoKHR::default()
                .layout(pipeline_layout)
                // .base_pipeline_handle(vk::Pipeline::null())
                // .base_pipeline_index(0)
                // .dynamic_state()
                .groups(&shader_groups)
                .max_pipeline_ray_recursion_depth(1)
                .stages(&stage_create_info)
        ];
        let pipelines = unsafe {
            instance.create_ray_tracing_pipelines(vk::DeferredOperationKHR::null(), vk::PipelineCache::null(),
                                                  &create_info, None).unwrap()
        };

        for &s in shader_modules.iter() {
            unsafe { logical_layer.logical_device.destroy_shader_module(s, None) }
        }

        RtPipeline {
            instance,
            pipelines,
            pipeline_layout
        }
    }

    pub fn destroy(&self, logical_layer: &LogicalLayer) {
        unsafe {
            for s in self.pipelines.iter() {
                logical_layer.logical_device.destroy_pipeline(*s, None);
            }
            logical_layer.logical_device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}