use ash::vk;
use crate::vkcore::VkCore;

pub fn create_sampler(core: &VkCore, mip_levels: u32) -> vk::Sampler {
    let properties = unsafe { core.instance.get_physical_device_properties(core.physical_device) };

    let sampler_create_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR) // How to interpolate magnified or minified texels
        .min_filter(vk::Filter::LINEAR)
        .address_mode_u(vk::SamplerAddressMode::REPEAT) // How to extend the texture beyond the reference image
        .address_mode_v(vk::SamplerAddressMode::REPEAT)
        .address_mode_w(vk::SamplerAddressMode::REPEAT)
        .anisotropy_enable(true) // Enable texture up/down sampling
        .max_anisotropy(properties.limits.max_sampler_anisotropy)
        .border_color(vk::BorderColor::INT_OPAQUE_BLACK) // What color to paint areas not covered by the texture
        .unnormalized_coordinates(false) // true - coordinates are [0, texture extent], false - coordinates are [0, 1]
        .compare_enable(false)
        .compare_op(vk::CompareOp::ALWAYS)
        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
        .mip_lod_bias(0.0)
        .min_lod(0.0)
        .max_lod(mip_levels as f32);

    unsafe { core.logical_device.create_sampler(&sampler_create_info, None)
        .unwrap() }
}

pub fn destroy_sampler(core: &VkCore, sampler: vk::Sampler) {
    unsafe { core.logical_device.destroy_sampler(sampler, None); }
}

