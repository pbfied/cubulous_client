use std::ffi::{c_char, CStr, CString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::mem;

use ash::vk;
use ash::vk::PipelineLayoutCreateFlags;

use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::render_target::RenderTarget;
use crate::renderer::vertex::Vertex;

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
            logical_layer.logical_device.create_shader_module(&shader_create_info, None).unwrap()
        });
    }

    shader_modules
}

fn setup_pipeline_layout(logical_layer: &LogicalLayer, layout: vk::DescriptorSetLayout) -> vk::PipelineLayout  {
    let ubo_layout_binding_arr = [layout];

    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&ubo_layout_binding_arr)
        .flags(PipelineLayoutCreateFlags::empty());

    unsafe {
        logical_layer.logical_device.create_pipeline_layout(&pipeline_layout_create_info, None).unwrap()
    }
}

pub struct RasterPipeline {
    pub pipeline_layout: vk::PipelineLayout,
    pub pipelines: Vec<vk::Pipeline>
}

impl RasterPipeline {
    pub fn new(logical_layer: &LogicalLayer, render_pass: vk::RenderPass,
               layout: vk::DescriptorSetLayout, msaa_samples: vk::SampleCountFlags) -> RasterPipeline {
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

        let shader_modules = load_all_shaders(logical_layer);

        let pipeline_stages = setup_pipeline_stages(&shader_modules);

        let vertex_binding_descriptions = [Vertex::get_binding_description()];
        let vertex_attribute_descriptions = &Vertex::get_attribute_descriptions();

        let vertex_inputs = vk::PipelineVertexInputStateCreateInfo::default() // Describe the format of each Vertex buffer entry
            .vertex_attribute_descriptions(vertex_attribute_descriptions)
            .vertex_binding_descriptions(&vertex_binding_descriptions);

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
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE) // Rules for determining if a face is front ??
            .depth_bias_enable(false) // Parameters for transforming depth values
            .depth_bias_constant_factor(0.0)
            .depth_bias_clamp(0.0)
            .depth_bias_slope_factor(0.0);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .sample_shading_enable(true) // Disabled for now
            .rasterization_samples(msaa_samples)
            .min_sample_shading(0.2)
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

        let pipeline_layout = setup_pipeline_layout(logical_layer, layout);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .front(vk::StencilOpState::default())
            .back(vk::StencilOpState::default());

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&pipeline_stages)
            .vertex_input_state(&vertex_inputs)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blending_create_info)
            .dynamic_state(&dynamic_state_create_info)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0);

        let pipelines = unsafe { logical_layer.logical_device.create_graphics_pipelines(vk::PipelineCache::null(),
                                                                                   &[pipeline_info],
                                                                                   None).unwrap() };

        for &s in shader_modules.iter() {
            unsafe { logical_layer.logical_device.destroy_shader_module(s, None) }
        }

        RasterPipeline {
            pipeline_layout,
            pipelines
        }
    }

    pub fn destroy(&mut self, logical_layer: &LogicalLayer) {
        unsafe {
            for s in self.pipelines.iter() {
                logical_layer.logical_device.destroy_pipeline(*s, None);
            }
            logical_layer.logical_device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}