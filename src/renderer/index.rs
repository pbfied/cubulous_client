use std::mem;
use ash::vk;
use crate::renderer::core::Core;
use crate::renderer::logical_layer::LogicalLayer;
use crate::renderer::physical_layer::PhysicalLayer;

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Index {
    pub data: [u16; 12]
}

