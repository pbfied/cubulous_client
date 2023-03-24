use std::mem;

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Index {
    pub data: [u16; 12]
}

