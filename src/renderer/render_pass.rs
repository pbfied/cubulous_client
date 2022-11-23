use ash::vk;

use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::render_target::RenderTarget;

pub(crate) fn setup_render_pass(logical_layer: &LogicalLayer, render_target: &RenderTarget) -> vk::RenderPass {
    let attachment_desc = vk::AttachmentDescription::default() // Color attachment
        .format(render_target.surface_format) // Should match the format of swap chain images
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

    unsafe {logical_layer.logical_device.create_render_pass(&render_pass_create_info, None).unwrap() }
}

pub(crate) fn destroy_render_pass(logical_layer: &LogicalLayer, render_pass: vk::RenderPass) {
    unsafe { logical_layer.logical_device.destroy_render_pass(render_pass, None) };
}