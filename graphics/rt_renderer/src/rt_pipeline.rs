use std::ffi::CStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::mem;
use ash::vk;
use ash::extensions::khr;
use ash::vk::Pipeline;
use cgmath::Vector4;
use renderlib::gpu_buffer::{create_buffer, GpuBuffer};
use renderlib::vkcore::VkCore;

const RAYGEN_IDX: usize = 0;
const RAYHIT_IDX: usize = 1;
const RAYMISS_IDX: usize = 2;

const RAYHIT_COUNT: usize = 1;
const RAYMISS_COUNT: usize = 1;
const RAYCALL_COUNT: usize = 0;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct RtMissConstants {
    pub clear_color: Vector4<f32>
}

pub struct RtPipeline {
    instance: khr::RayTracingPipeline,
    pub pipelines: Vec<Pipeline>,
    pub pipeline_layout: vk::PipelineLayout,
    pub sbt_buf: vk::Buffer,
    pub sbt_mem: vk::DeviceMemory,
    pub raygen_addr_region: vk::StridedDeviceAddressRegionKHR,
    pub raymiss_addr_region: vk::StridedDeviceAddressRegionKHR,
    pub rayhit_addr_region: vk::StridedDeviceAddressRegionKHR,
    pub raycallable_addr_region: vk::StridedDeviceAddressRegionKHR
}

fn align_u32(val: u32, align: u32) -> u32 {
    (val + (align - 1)) & !(align - 1) // Round up operation suggested on
    // https://nvpro-samples.github.io/, since group handle size may not equal the alignment
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

fn load_all_shaders(core: &VkCore) -> Vec<vk::ShaderModule> {
    let shader_paths = ["graphics/shaders/spv/rgen.spv", "graphics/shaders/spv/rchit.spv",
        "graphics/shaders/spv/rmiss.spv"];

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
            core.logical_device.create_shader_module(&shader_create_info, None).unwrap()
        });
    }

    shader_modules
}

impl RtPipeline {
    pub fn new(core: &VkCore, layouts: &Vec<vk::DescriptorSetLayout>) -> RtPipeline {
        let instance = khr::RayTracingPipeline::new(&core.instance, &core.logical_device);
        let push_constant_ranges = [
            vk::PushConstantRange::default()
                .offset(0)
                .size(mem::size_of::<RtMissConstants>() as u32)
                .stage_flags(vk::ShaderStageFlags::MISS_KHR)
        ];
        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .flags(vk::PipelineLayoutCreateFlags::empty())
            .set_layouts(layouts.as_slice())
            .push_constant_ranges(&push_constant_ranges);
        let pipeline_layout = unsafe { core.logical_device.create_pipeline_layout(&layout_create_info, None)
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
                .any_hit_shader(vk::SHADER_UNUSED_KHR)
                .general_shader(vk::SHADER_UNUSED_KHR)
                .closest_hit_shader(RAYHIT_IDX as u32)
                .intersection_shader(vk::SHADER_UNUSED_KHR),
            vk::RayTracingShaderGroupCreateInfoKHR::default() // miss
                .ty(vk::RayTracingShaderGroupTypeKHR::GENERAL)
                .general_shader(RAYMISS_IDX as u32)
                .any_hit_shader(vk::SHADER_UNUSED_KHR)
                .intersection_shader(vk::SHADER_UNUSED_KHR)
                .closest_hit_shader(vk::SHADER_UNUSED_KHR),
        ];
        let shader_modules = load_all_shaders(core);
        let stage_create_info = [
            vk::PipelineShaderStageCreateInfo::default()
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
                .stage(vk::ShaderStageFlags::RAYGEN_KHR)
                .module(shader_modules[RAYGEN_IDX]),
            vk::PipelineShaderStageCreateInfo::default()
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
                .stage(vk::ShaderStageFlags::CLOSEST_HIT_KHR)
                .module(shader_modules[RAYHIT_IDX]),
            vk::PipelineShaderStageCreateInfo::default()
                .name(CStr::from_bytes_with_nul(b"main\0").unwrap())
                .stage(vk::ShaderStageFlags::MISS_KHR)
                .module(shader_modules[RAYMISS_IDX]),
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

        let rt_properties = unsafe { khr::RayTracingPipeline::get_properties(&core.instance, core.physical_device) };

        // Note that each shader table group is made up of one handle for each shader within the group
        // Handles have alignment requirements
        let handle_size = align_u32(rt_properties.shader_group_handle_size, rt_properties
            .shader_group_handle_alignment);
        // Since the group size is used to calculate the offset of the next region, each size must be a multiple of shader_group_base_alignment
        let raygen_group_size = align_u32(handle_size, rt_properties.shader_group_base_alignment) as vk::DeviceSize;
        let rmiss_group_size = align_u32(handle_size * RAYMISS_COUNT as u32, rt_properties
            .shader_group_base_alignment) as vk::DeviceSize;
        let rhit_group_size = align_u32(handle_size * RAYHIT_COUNT as u32, rt_properties.shader_group_base_alignment) as vk::DeviceSize;
        let rcall_group_size = align_u32(handle_size * RAYCALL_COUNT as u32, rt_properties
            .shader_group_base_alignment) as vk::DeviceSize;
        let sbt_size = raygen_group_size + rmiss_group_size + rhit_group_size + rcall_group_size;

        // Should probably replace with a device local buffer later for draw indirect calls
        let (sbt_mem, sbt_buf) = create_buffer(core, sbt_size,
                                               vk::BufferUsageFlags::SHADER_BINDING_TABLE_KHR |
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::TRANSFER_SRC,
                                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT);
        let addr_info = vk::BufferDeviceAddressInfo::default()
            .buffer(sbt_buf);
        let sbt_buf_addr = unsafe {
            core.logical_device.get_buffer_device_address(&addr_info)
        };

        // The Vulkan spec states that the raygen handle stride must be equal to its group size
        let raygen_handle_stride = raygen_group_size;

        let raygen_addr_region = vk::StridedDeviceAddressRegionKHR::default()
            .size(raygen_group_size as vk::DeviceSize)
            .device_address(sbt_buf_addr)
            .stride(raygen_handle_stride as vk::DeviceSize);
        let rayhit_addr_region = vk::StridedDeviceAddressRegionKHR::default()
            .size(rhit_group_size as vk::DeviceSize)
            .device_address(sbt_buf_addr + raygen_group_size)
            .stride(handle_size as vk::DeviceSize);
        let raymiss_addr_region = vk::StridedDeviceAddressRegionKHR::default()
            .size(rmiss_group_size as vk::DeviceSize)
            .device_address(sbt_buf_addr + raygen_group_size + rhit_group_size)
            .stride(handle_size as vk::DeviceSize);
        let raycallable_addr_region = vk::StridedDeviceAddressRegionKHR::default()
            .size(0);

        // Apparently the handles are the raw bytes of the compiled shaders and ready for copying into the SBT?
        let handles = unsafe { instance.get_ray_tracing_shader_group_handles(*pipelines.get(0).unwrap(), 0,
                                                                             shader_groups.len() as u32,
                                                                             (rt_properties.shader_group_handle_size
                                                                                 * stage_create_info.len() as u32
                                                                             ) as usize).unwrap() };

        // Copy shaders to the shader binding table
        unsafe {
            let mut sbt_mapped_memory = core.logical_device
                .map_memory(sbt_mem,
                            0,
                            sbt_size,
                            vk::MemoryMapFlags::empty())
                .unwrap() as *mut u8;
            let mut handles_ptr = handles.as_ptr();
            // Copy the raygen, always the first entry. Note that padding bytes are not copied
            sbt_mapped_memory.copy_from_nonoverlapping(handles_ptr, rt_properties.shader_group_handle_size as
                usize);
            sbt_mapped_memory = sbt_mapped_memory.add(raygen_addr_region.stride as usize);
            handles_ptr = handles_ptr.add(rt_properties.shader_group_handle_size as usize);
            for _ in 0..RAYHIT_COUNT {
                sbt_mapped_memory.copy_from_nonoverlapping(handles_ptr, rt_properties.shader_group_handle_size as usize);
                handles_ptr = handles_ptr.add(rt_properties.shader_group_handle_size as usize);
                sbt_mapped_memory = sbt_mapped_memory.add(rayhit_addr_region.stride as usize);
            }
            for _ in 0..RAYMISS_COUNT {
                sbt_mapped_memory.copy_from_nonoverlapping(handles_ptr, rt_properties.shader_group_handle_size as usize);
                handles_ptr = handles_ptr.add(rt_properties.shader_group_handle_size as usize);
                sbt_mapped_memory = sbt_mapped_memory.add(raymiss_addr_region.stride as usize);
            }
            core.logical_device.unmap_memory(sbt_mem);
        }

        for &s in shader_modules.iter() {
            unsafe { core.logical_device.destroy_shader_module(s, None) }
        }

        RtPipeline {
            instance,
            pipelines,
            pipeline_layout,
            sbt_buf,
            sbt_mem,
            raygen_addr_region,
            raymiss_addr_region,
            rayhit_addr_region,
            raycallable_addr_region,
        }
    }

    pub fn destroy(&self, core: &VkCore) {
        unsafe {
            for s in self.pipelines.iter() {
                core.logical_device.destroy_pipeline(*s, None);
            }
            core.logical_device.destroy_pipeline_layout(self.pipeline_layout, None);
            core.logical_device.destroy_buffer(self.sbt_buf, None);
            core.logical_device.free_memory(self.sbt_mem, None);
        }
    }
}