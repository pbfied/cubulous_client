use ash::vk;
use memoffset::offset_of;

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [f32; 3]
}

impl Vertex {
    pub fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32) // Number of bytes per entry in the binding
            .input_rate(vk::VertexInputRate::VERTEX) // ??
    }

    pub fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        [vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0, // Index of the vertex binding
            format: vk::Format::R32G32_SFLOAT, // Describes a vec2 of 32 bit floating point numbers, not a color
            offset: offset_of!(Vertex, pos) as u32 // Offset of this attribute within this binding entry
        }, vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: offset_of!(Vertex, color) as u32
        }]
    }
}