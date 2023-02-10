use ash::{vk, Device};

use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;
use crate::renderer::render_target::RenderTarget;

pub fn setup_frame_buffers(logical_layer: &LogicalLayer, render_pass: vk::RenderPass,
                           render_target: &RenderTarget, depth_view: vk::ImageView,
                           color_view: vk::ImageView) -> Vec<vk::Framebuffer> {
    let mut frame_buffers: Vec<vk::Framebuffer> = Vec::with_capacity(render_target.image_views.len());
    for v in render_target.image_views.iter() {
        let image_slice = [color_view, depth_view, *v];
        let create_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&image_slice)
            .width(render_target.extent.width)
            .height(render_target.extent.height)
            .layers(1);

        unsafe { frame_buffers.push(logical_layer.logical_device.create_framebuffer(&create_info, None).unwrap()) }
    }

    frame_buffers
}

pub fn destroy_frame_buffers(logical_layer: &LogicalLayer, frame_buffers: &Vec<vk::Framebuffer>) {
    for f in frame_buffers.iter() {
        unsafe { logical_layer.logical_device.destroy_framebuffer(*f, None) };
    }
}