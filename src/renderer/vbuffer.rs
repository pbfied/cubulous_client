// use std::mem;
// use ash::{vk, Device, Instance};
// use memoffset::offset_of;
//
// #[repr(C)]
// #[derive(Clone, Debug, Copy)]
// pub(crate) struct Vertex {
//     pub pos: [f32; 2],
//     pub color: [f32; 3]
// }
//
// const INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];
//
// pub struct VertexBuffer {
//     pub vertex_buffer: vk::Buffer,
//     vertex_mem: vk::DeviceMemory,
//     vertices_size: vk::DeviceSize,
//     pub vertex_count: u32
// }
//
// impl VertexBuffer {
//     fn copy_buffer(&self,
//                    logical_queue: vk::Queue,
//                    logical_device: &Device,
//                    cmd_pool: vk::CommandPool,
//                    transfer_buffer: vk::Buffer) {
//         let buf_alloc_info = vk::CommandBufferAllocateInfo::default()
//             .level(vk::CommandBufferLevel::PRIMARY)
//             .command_pool(cmd_pool)
//             .command_buffer_count(1);
//
//         let command_buffer_vec = unsafe { logical_device.allocate_command_buffers(&buf_alloc_info).unwrap() };
//
//         let command_buffer = *command_buffer_vec.get(0).unwrap();
//
//         let command_buffer_array = [command_buffer];
//
//         let begin_info = vk::CommandBufferBeginInfo::default()
//             .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
//
//         unsafe { logical_device.begin_command_buffer(command_buffer, &begin_info).unwrap() };
//
//         let copy_region = vk::BufferCopy::default()
//             .size(self.vertices_size)
//             .dst_offset(0)
//             .src_offset(0);
//
//         let copy_regions = [copy_region];
//
//         let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffer_array);
//         let submit_info_slice = [submit_info];
//
//         unsafe {
//             logical_device.cmd_copy_buffer(command_buffer, transfer_buffer, self.vertex_buffer, &copy_regions);
//             logical_device.end_command_buffer(command_buffer).unwrap();
//             logical_device.queue_submit(logical_queue, &submit_info_slice, vk::Fence::null()).unwrap();
//             logical_device.queue_wait_idle(logical_queue).unwrap();
//             logical_device.free_command_buffers(cmd_pool, &command_buffer_array);
//         }
//     }
//
//     pub fn new(physical_device: vk::PhysicalDevice, instance: &Instance, logical_device: &Device, graphics_queue: vk::Queue, cmd_pool: vk::CommandPool) -> VertexBuffer {
//         fn create_buffer(physical_device: vk::PhysicalDevice,
//                          instance: &Instance,
//                          logical_device: &Device,
//                          size: vk::DeviceSize,
//                          usage: vk::BufferUsageFlags,
//                          mem_props: vk::MemoryPropertyFlags) -> Result<(vk::DeviceMemory, vk::Buffer), ()> {
//             let buffer_create_info = vk::BufferCreateInfo::default()
//                 .size(size)
//                 .usage(usage)
//                 .sharing_mode(vk::SharingMode::EXCLUSIVE);
//
//             let buffer = unsafe { logical_device.create_buffer(&buffer_create_info, None).unwrap() };
//
//             let mem_reqs = unsafe {logical_device.get_buffer_memory_requirements(buffer)};
//
//             let phys_mem_props = unsafe {instance.get_physical_device_memory_properties(physical_device)};
//
//             let mut retval = Err(());
//             for i in 0..phys_mem_props.memory_type_count {
//                 if ((1 << i) & mem_reqs.memory_type_bits) > 0 && // If this physical memory type is valid for the requirement
//                     phys_mem_props.memory_types.get(i as usize).unwrap()
//                         .property_flags
//                         .contains(mem_props) {
//                     // Explicit flushes are required otherwise
//                     let alloc_info = vk::MemoryAllocateInfo::default()
//                         .allocation_size(mem_reqs.size)
//                         .memory_type_index(i);
//                     let buffer_mem = unsafe {logical_device.allocate_memory(&alloc_info, None).unwrap()};
//                     unsafe { logical_device.bind_buffer_memory(buffer, buffer_mem, 0).unwrap() };
//                     retval = Ok((buffer_mem, buffer));
//                     break;
//                 }
//             }
//
//             retval
//         }
//
//         let vertices_size: vk::DeviceSize = mem::size_of_val(&VERTICES) as vk::DeviceSize;
//         let vertex_count = VERTICES.len();
//
//         let (transfer_mem, transfer_buffer) = create_buffer(physical_device,
//                                                             instance,
//                                                             logical_device,
//                                                             vertices_size,
//                                                             vk::BufferUsageFlags::TRANSFER_SRC, // Can be a used as a source for transfer commands
//                                                             vk::MemoryPropertyFlags::HOST_VISIBLE | // Visible for writes on the host
//                                                                 vk::MemoryPropertyFlags::HOST_COHERENT) // COHERENT means that copy operations are atomic with respect to subsequent vkQueueSubmit calls
//             .expect("Failed to locate suitable device memory");
//
//         unsafe {
//             let dev_memory = logical_device
//                 .map_memory(transfer_mem,
//                             0,
//                             vertices_size,
//                             vk::MemoryMapFlags::empty())
//                 .unwrap() as *mut Vertex;
//             dev_memory.copy_from_nonoverlapping(VERTICES.as_ptr(), vertex_count);
//             logical_device.unmap_memory(transfer_mem);
//         }
//
//         let (vertex_mem, vertex_buffer) = create_buffer(physical_device,
//                                                         instance,
//                                                         logical_device,
//                                                         vertices_size,
//                                                         vk::BufferUsageFlags::VERTEX_BUFFER | // Used by the vertex shader stage
//                                                             vk::BufferUsageFlags::TRANSFER_DST, // Can be a destination for transfer commands
//                                                         vk::MemoryPropertyFlags::DEVICE_LOCAL) // Local to GPU
//             .expect("Failed to locate suitable device memory");
//
//         let vbuf = VertexBuffer {
//             vertex_buffer,
//             vertex_mem,
//             vertices_size,
//             vertex_count: vertex_count as u32
//         };
//
//         vbuf.copy_buffer(graphics_queue, logical_device, cmd_pool, transfer_buffer);
//
//         unsafe {
//             logical_device.destroy_buffer(transfer_buffer, None);
//             logical_device.free_memory(transfer_mem, None);
//         }
//
//         vbuf
//     }
//
//     pub fn destroy(&self, logical_device: &Device) {
//         unsafe {
//             logical_device.destroy_buffer(self.vertex_buffer, None);
//             logical_device.free_memory(self.vertex_mem, None);
//         }
//     }
// }