use std::mem;
use ash::vk;
use crate::core::Core;
use crate::logical_layer::LogicalLayer;
use crate::physical_layer::PhysicalLayer;

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Index {
    pub data: [u16; 12]
}

