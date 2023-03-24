use ash::vk;

pub struct RtFrame {
    descriptor_layout: vk::DescriptorSetLayout,

}

impl RtFrame {
    pub fn new(&self) -> RtFrame {


        RtFrame {
            descriptor_layout: Default::default(),
        }
    }
}